//! Bounded JSON-object decoding with Skein's tokenizer. The decoded tree is
//! temporary and never crosses the engine or Smith domain boundary.

use alloc::boxed::Box;
use skein_json::{Token, tokenizer};
use skein_lib::stream::{Down, Up};
use skein_lib::{Env, Intake, List, Queue, Stack, Time, Wall};

use crate::{Limits, Problem, Why};

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Object(Box<[(Box<[u8]>, Value)]>),
    Array(Box<[Value]>),
    String(Box<[u8]>),
    Number(Box<[u8]>),
    Boolean(bool),
    Null,
}

pub(crate) fn parse(input: &[u8], bounds: &Limits) -> Result<Value, Problem> {
    if input.len() > usize::try_from(bounds.input_bytes).expect("u32 fits usize") {
        return Err(Problem { field: b"", why: Why::TooLarge });
    }
    let Ok(length) = u32::try_from(input.len()) else { return Err(Problem { field: b"", why: Why::TooLarge }) };
    let limits = tokenizer::Limits { depth: bounds.depth, string: length, number: 24, chunk: length.max(1), length };
    let mut machine = tokenizer::Tokenizer::new(&limits);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut intake = Intake::with_capacity(length.max(tokenizer::largest_demand(&limits)));
    if intake.append(input).is_err() {
        return Err(Problem { field: b"", why: Why::TooLarge });
    }
    let mut above = Queue::with_capacity(1);
    let mut below = Queue::with_capacity(1);
    let mut tokens = List::with_capacity(4096);
    let mut demanded = None;
    let steps = input.len().checked_mul(4).ok_or(Problem { field: b"", why: Why::TooLarge })?;
    let steps = steps.checked_add(8).ok_or(Problem { field: b"", why: Why::TooLarge })?;
    for _ in 0..steps {
        match demanded.take() {
            Some(read) => match intake.meet(read) {
                Some(bytes) => tokenizer::up(&mut machine, &env, Up::Bytes(bytes), &mut above, &mut below),
                None => tokenizer::up(&mut machine, &env, Up::End, &mut above, &mut below),
            },
            None => tokenizer::down(&mut machine, &env, tokenizer::Request::Next, &mut above, &mut below),
        }
        match below.pop() {
            Some(Down::Demand { read, .. }) => demanded = Some(read),
            Some(Down::Send(_) | Down::Finish) => return Err(Problem { field: b"", why: Why::Malformed }),
            None => {}
        }
        match above.pop() {
            Some(tokenizer::Event::Token(token)) => {
                let Ok(()) = tokens.push(token) else { return Err(Problem { field: b"", why: Why::TooLarge }) };
            }
            Some(tokenizer::Event::Done) => {
                let tokens = tokens.into_boxed();
                let value = value(tokens, bounds)?;
                return match &value.kind {
                    Kind::Object(_) => Ok(value),
                    Kind::Array(_) | Kind::String(_) | Kind::Number(_) | Kind::Boolean(_) | Kind::Null => {
                        Err(Problem { field: b"", why: Why::Type })
                    }
                };
            }
            // Only `Next` is asked, so a capped text's or a skip's answer never comes.
            Some(tokenizer::Event::Failed(reason)) => return Err(tokenizer_problem(reason)),
            Some(tokenizer::Event::Closed | tokenizer::Event::Long(_) | tokenizer::Event::Skipped(_)) => {
                return Err(Problem { field: b"", why: Why::Malformed });
            }
            None => {}
        }
    }
    Err(Problem { field: b"", why: Why::Malformed })
}

enum Frame {
    Object { fields: List<(Box<[u8]>, Value)>, key: Option<Box<[u8]>> },
    Array(List<Value>),
}

fn value(tokens: Box<[Token]>, bounds: &Limits) -> Result<Value, Problem> {
    let mut stack = Stack::with_capacity(bounds.depth);
    let mut result = None;
    for token in tokens {
        let kind = match token {
            Token::ObjectStart => {
                if stack.push(Frame::Object { fields: List::with_capacity(128), key: None }).is_err() {
                    return Err(Problem { field: b"", why: Why::TooLarge });
                }
                continue;
            }
            Token::ArrayStart => {
                if stack.push(Frame::Array(List::with_capacity(128))).is_err() {
                    return Err(Problem { field: b"", why: Why::TooLarge });
                }
                continue;
            }
            Token::Key(next) => {
                let (fields, key) = match stack.top_mut() {
                    Some(Frame::Object { fields, key }) => (fields, key),
                    Some(Frame::Array(_)) | None => return Err(Problem { field: b"", why: Why::Malformed }),
                };
                if key.is_some() {
                    return Err(Problem { field: b"", why: Why::Malformed });
                }
                for (previous, _) in fields.as_slice() {
                    if previous == &next {
                        return Err(Problem { field: b"", why: Why::Malformed });
                    }
                }
                *key = Some(next);
                continue;
            }
            Token::ObjectEnd => match stack.pop() {
                Some(Frame::Object { fields, key: None }) => Kind::Object(fields.into_boxed()),
                Some(Frame::Object { key: Some(_), .. } | Frame::Array(_)) | None => {
                    return Err(Problem { field: b"", why: Why::Malformed });
                }
            },
            Token::ArrayEnd => match stack.pop() {
                Some(Frame::Array(items)) => Kind::Array(items.into_boxed()),
                Some(Frame::Object { .. }) | None => return Err(Problem { field: b"", why: Why::Malformed }),
            },
            Token::String(text) => Kind::String(text),
            Token::Number(text) => Kind::Number(text),
            Token::True => Kind::Boolean(true),
            Token::False => Kind::Boolean(false),
            Token::Null => Kind::Null,
        };
        let mut item = Value { kind, field: b"", words: bounds.words };
        match stack.top_mut() {
            Some(Frame::Object { fields, key }) => {
                let key = key.take().ok_or(Problem { field: b"", why: Why::Malformed })?;
                item.field = field_name(&key);
                match &mut item.kind {
                    Kind::Array(values) => {
                        for value in values {
                            if value.field.is_empty() {
                                value.field = item.field;
                            }
                        }
                    }
                    Kind::Object(_) | Kind::String(_) | Kind::Number(_) | Kind::Boolean(_) | Kind::Null => {}
                }
                if fields.push((key, item)).is_err() {
                    return Err(Problem { field: b"", why: Why::TooLarge });
                }
            }
            Some(Frame::Array(items)) => {
                if items.push(item).is_err() {
                    return Err(Problem { field: b"", why: Why::TooLarge });
                }
            }
            None => {
                if result.is_some() {
                    return Err(Problem { field: b"", why: Why::Malformed });
                }
                result = Some(item);
            }
        }
    }
    if !stack.is_empty() {
        return Err(Problem { field: b"", why: Why::Malformed });
    }
    result.ok_or(Problem { field: b"", why: Why::Malformed })
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Value {
    pub(crate) kind: Kind,
    field: &'static [u8],
    words: u32,
}

impl Value {
    pub(crate) fn field(&self, name: &[u8]) -> Result<Option<&Value>, Problem> {
        match &self.kind {
            Kind::Object(fields) => {
                for (key, value) in fields {
                    if key.as_ref() == name {
                        return Ok(Some(value));
                    }
                }
                Ok(None)
            }
            Kind::Array(_) | Kind::String(_) | Kind::Number(_) | Kind::Boolean(_) | Kind::Null => {
                Err(Problem { field: self.field, why: Why::Type })
            }
        }
    }

    pub(crate) fn required(&self, name: &'static [u8]) -> Result<&Value, Problem> {
        self.field(name)?.ok_or(Problem { field: name, why: Why::Missing })
    }

    pub(crate) fn text(&self) -> Result<Box<[u8]>, Problem> {
        match &self.kind {
            Kind::String(text) => {
                if text.len() > usize::try_from(self.words).expect("u32 fits usize") {
                    return Err(Problem { field: self.field, why: Why::TooLarge });
                }
                Ok(text.clone())
            }
            Kind::Object(_) | Kind::Array(_) | Kind::Number(_) | Kind::Boolean(_) | Kind::Null => {
                Err(Problem { field: self.field, why: Why::Type })
            }
        }
    }

    pub(crate) fn number(&self) -> Result<u64, Problem> {
        match &self.kind {
            Kind::Number(text) => decimal(text).ok_or(Problem { field: self.field, why: Why::Range }),
            Kind::Object(_) | Kind::Array(_) | Kind::String(_) | Kind::Boolean(_) | Kind::Null => {
                Err(Problem { field: self.field, why: Why::Type })
            }
        }
    }

    pub(crate) fn boolean(&self) -> Result<bool, Problem> {
        match &self.kind {
            Kind::Boolean(value) => Ok(*value),
            Kind::Object(_) | Kind::Array(_) | Kind::String(_) | Kind::Number(_) | Kind::Null => {
                Err(Problem { field: self.field, why: Why::Type })
            }
        }
    }

    pub(crate) fn array(&self) -> Result<&[Value], Problem> {
        match &self.kind {
            Kind::Array(items) => Ok(items),
            Kind::Object(_) | Kind::String(_) | Kind::Number(_) | Kind::Boolean(_) | Kind::Null => {
                Err(Problem { field: self.field, why: Why::Type })
            }
        }
    }

    pub(crate) fn small(&self) -> Result<u32, Problem> {
        let Ok(value) = u32::try_from(self.number()?) else {
            return Err(Problem { field: self.field, why: Why::Range });
        };
        Ok(value)
    }

    pub(crate) fn narrow(&self) -> Result<u16, Problem> {
        let Ok(value) = u16::try_from(self.number()?) else {
            return Err(Problem { field: self.field, why: Why::Range });
        };
        Ok(value)
    }
}

fn field_name(key: &[u8]) -> &'static [u8] {
    const NAMES: &[&[u8]] = &[
        b"llm",
        b"main",
        b"model",
        b"effort",
        b"account",
        b"subagents",
        b"action",
        b"age",
        b"amendment",
        b"answers",
        b"at",
        b"authority",
        b"batch",
        b"before",
        b"body",
        b"budget",
        b"change_kind",
        b"charter",
        b"choices",
        b"code",
        b"connector",
        b"contract",
        b"count",
        b"deadline",
        b"decision",
        b"delegation",
        b"dependencies",
        b"depth",
        b"description",
        b"environment",
        b"executor",
        b"fields",
        b"filter",
        b"form",
        b"from",
        b"goal",
        b"grants",
        b"inputs",
        b"instructions",
        b"kind",
        b"kinds",
        b"last",
        b"left",
        b"max",
        b"max_bytes",
        b"name",
        b"news",
        b"note_resources",
        b"notes",
        b"notices",
        b"number",
        b"operation",
        b"page",
        b"parameters",
        b"pattern",
        b"period",
        b"person",
        b"project",
        b"proposal",
        b"proposer",
        b"query",
        b"question",
        b"questions",
        b"reason",
        b"recalled",
        b"references",
        b"resource",
        b"results",
        b"revision",
        b"right",
        b"role",
        b"scope",
        b"scopes",
        b"segments",
        b"service",
        b"setting",
        b"spec",
        b"spend",
        b"subscription",
        b"symbolic_grants",
        b"target",
        b"task",
        b"tasks",
        b"terminal",
        b"through",
        b"timers",
        b"tools",
        b"value",
        b"wake",
        b"words",
    ];
    for name in NAMES {
        if key == *name {
            return name;
        }
    }
    b""
}

pub(crate) fn decimal(bytes: &[u8]) -> Option<u64> {
    let mut value = 0_u64;
    if bytes.is_empty() {
        return None;
    }
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u64::from(byte.checked_sub(b'0')?))?;
    }
    Some(value)
}

fn tokenizer_problem(reason: tokenizer::Error) -> Problem {
    let why = match reason {
        tokenizer::Error::TooLong
        | tokenizer::Error::TooDeep
        | tokenizer::Error::StringTooLong
        | tokenizer::Error::NumberTooLong => Why::TooLarge,
        tokenizer::Error::Unexpected
        | tokenizer::Error::Trailing
        | tokenizer::Error::Number
        | tokenizer::Error::Escape
        | tokenizer::Error::Surrogate
        | tokenizer::Error::Utf8
        | tokenizer::Error::Control
        | tokenizer::Error::Truncated
        | tokenizer::Error::Stream(_) => Why::Malformed,
    };
    Problem { field: b"", why }
}
