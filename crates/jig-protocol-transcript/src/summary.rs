//! Bounded tool-outcome summaries; tool names and inputs remain on their calls.
//! Provider text results remain verbatim; opaque payloads never enter a summary.
use alloc::boxed::Box;
use skein_lib::stream::Read;
use skein_lib::{Decimal, Intake, Reader};
use smith_transcript::v6 as smith;

use crate::Undecodable;

// Found's byte piece contains successive generated Hit records. As in Smith's
// transcript reader, validate each record before interpreting the piece.
fn hit_count(limits: &smith::Limits, piece: &[u8]) -> Result<u64, Undecodable> {
    let mut reader = Reader::new(piece);
    let mut count = 0_u64;
    for _ in 0..=piece.len().checked_div(12).expect("nonzero hit framing length") {
        if reader.is_empty() {
            return Ok(count);
        }
        let path = reader.u32().ok_or(Undecodable::Malformed)?;
        if path > limits.hit_path {
            return Err(Undecodable::TooLarge);
        }
        reader.bytes(path).ok_or(Undecodable::Malformed)?;
        reader.u32().ok_or(Undecodable::Malformed)?;
        let text = reader.u32().ok_or(Undecodable::Malformed)?;
        if text > limits.hit_text {
            return Err(Undecodable::TooLarge);
        }
        reader.bytes(text).ok_or(Undecodable::Malformed)?;
        count = count.checked_add(1).expect("piece bounds its hit count");
    }
    Err(Undecodable::Malformed)
}

pub(super) fn validate(limits: &smith::Limits, source: &smith::Turn) -> Result<(), Undecodable> {
    for message in source.messages() {
        for block in message.blocks() {
            match block {
                smith::Block::Result(result) => {
                    validate_returned(limits, result.returned())?;
                }
                smith::Block::Text(_) | smith::Block::Refusal(_) | smith::Block::Call(_) | smith::Block::Opaque(_) => {}
            }
        }
    }
    Ok(())
}

fn validate_returned(limits: &smith::Limits, returned: &smith::Returned) -> Result<(), Undecodable> {
    match returned {
        smith::Returned::Outcome(outcome) => match outcome.value() {
            smith::Outcome::Found(found) => {
                hit_count(limits, found.hits())?;
            }
            smith::Outcome::Read(_)
            | smith::Outcome::Listed(_)
            | smith::Outcome::Written(_)
            | smith::Outcome::Edited(_)
            | smith::Outcome::Exited(_)
            | smith::Outcome::Running(_)
            | smith::Outcome::Job(_)
            | smith::Outcome::NotGranted
            | smith::Outcome::Outside(_)
            | smith::Outcome::ReadOnly(_)
            | smith::Outcome::TooLong
            | smith::Outcome::NotFound(_)
            | smith::Outcome::Removed
            | smith::Outcome::NotEmpty
            | smith::Outcome::NotFile
            | smith::Outcome::Linked
            | smith::Outcome::Protected
            | smith::Outcome::NotDirectory(_)
            | smith::Outcome::TooLarge(_)
            | smith::Outcome::NotRead
            | smith::Outcome::Stale
            | smith::Outcome::EditFailed(_)
            | smith::Outcome::Failed(_)
            | smith::Outcome::TimedOut
            | smith::Outcome::Cancelled
            | smith::Outcome::Busy
            | smith::Outcome::NulByte => {}
        },
        smith::Returned::Invalid(_)
        | smith::Returned::Said(_)
        | smith::Returned::NotRun(_)
        | smith::Returned::Withdrawn => {}
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct Summary {
    pub bytes: Box<[u8]>,
    pub failed: bool,
}

pub(super) fn summarize(returned: &smith::Returned) -> Summary {
    let mut builder = Builder { bytes: Intake::with_capacity(256), failed: true };
    match returned {
        smith::Returned::Outcome(outcome) => builder.outcome(outcome.value()),
        smith::Returned::Said(said) => {
            // This arm is handled directly by the receiving text budget.
            builder.put(b"Tool returned text");
            builder.failed = said.error();
        }
        smith::Returned::Invalid(invalid) => {
            builder.put(b"Invalid call: ");
            match invalid.problem() {
                smith::CallProblem::UnknownTool => builder.put(b"unknown tool"),
                smith::CallProblem::NotAnObject => builder.put(b"input is not an object"),
                smith::CallProblem::Missing(field) => {
                    builder.put(b"missing ");
                    builder.put(field.field());
                }
                smith::CallProblem::WrongType(field) => {
                    builder.put(b"wrong type for ");
                    builder.put(field.field());
                }
                smith::CallProblem::BadValue(field) => {
                    builder.put(b"bad value for ");
                    builder.put(field.field());
                }
                smith::CallProblem::TooLarge => builder.put(b"input too large"),
                smith::CallProblem::Oversize(oversize) => {
                    builder.put(b"input too large, ");
                    builder.number(oversize.bytes());
                    builder.put(b" bytes over ");
                    builder.number(u64::from(oversize.bound()));
                }
                smith::CallProblem::CutOff(cut) => {
                    builder.put(b"input cut off after ");
                    builder.number(cut.bytes());
                    builder.put(b" bytes");
                }
                smith::CallProblem::JobsFull(full) => {
                    builder.put(b"jobs full, at most ");
                    builder.number(u64::from(full.jobs()));
                    builder.put(b" at once");
                }
                smith::CallProblem::UnknownJob(unknown) => {
                    builder.put(b"unknown job ");
                    builder.number(u64::from(unknown.job()));
                }
                smith::CallProblem::Nesting => builder.put(b"nested sub-agent"),
            }
        }
        smith::Returned::NotRun(not_run) => builder.put(match not_run.why() {
            smith::NotRunWhy::Stopped => b"Not run: stopped",
            smith::NotRunWhy::OutsideChoice(_) => b"Not run: outside the tool choice",
            smith::NotRunWhy::AfterFailedWrite(_) => b"Not run: after a failed write",
        }),
        smith::Returned::Withdrawn => builder.put(b"Withdrawn"),
    }
    let length = builder.bytes.len();
    Summary {
        bytes: builder.bytes.meet(Read::Fill(length)).expect("summary bytes are buffered"),
        failed: builder.failed,
    }
}

#[derive(Debug)]
struct Builder {
    bytes: Intake,
    failed: bool,
}
impl Builder {
    fn put(&mut self, bytes: &[u8]) {
        self.bytes.append(bytes).expect("fixed summary words and numeric fields fit 256 bytes");
    }
    fn number(&mut self, value: u64) {
        self.put(Decimal::of(value).as_bytes());
    }
    fn outcome(&mut self, outcome: &smith::Outcome) {
        match outcome {
            smith::Outcome::Read(read) => {
                self.failed = false;
                self.put(b"Read ");
                self.number(u64::from(read.lines()));
                self.put(b" of ");
                self.number(u64::from(read.total()));
                self.put(b" lines; skipped ");
                // Lines are numbered from one; a zero first line skipped none.
                self.number(u64::from(read.first().saturating_sub(1)));
                match read.cut() {
                    smith::ReadCut::Whole => {}
                    smith::ReadCut::Line => self.put(b"; content cut"),
                }
            }
            smith::Outcome::Listed(listed) => {
                self.failed = false;
                self.put(b"Listed ");
                self.number(u64::from(listed.entries().len()));
                self.put(b" entries; more ");
                self.number(listed.more());
            }
            smith::Outcome::Found(found) => {
                self.failed = false;
                self.put(b"Found ");
                self.number(hit_count(&smith::CEILINGS, found.hits()).expect("hit piece validated before translation"));
                self.put(b" hits; more ");
                self.number(found.more());
                if found.timed_out() {
                    self.put(b"; search timed out");
                }
            }
            smith::Outcome::Removed => {
                self.failed = false;
                self.put(b"Removed");
            }
            smith::Outcome::NotEmpty => self.put(b"Tool refused: directory not empty"),
            smith::Outcome::Written(written) => {
                self.failed = false;
                if written.created() {
                    self.put(b"Created file");
                } else {
                    self.put(b"Wrote file");
                }
            }
            smith::Outcome::Edited(edited) => {
                self.failed = false;
                self.put(b"Edited ");
                self.number(u64::from(edited.replaced()));
                self.put(b" matches");
            }
            smith::Outcome::Exited(end) => self.command(end),
            smith::Outcome::Running(running) => {
                self.failed = false;
                self.put(b"Started as job ");
                self.number(u64::from(running.job()));
                self.output(running.head(), running.last(), running.dropped());
            }
            smith::Outcome::Job(job) => self.job(job),
            smith::Outcome::NotGranted => self.put(b"Tool refused: not granted"),
            smith::Outcome::Outside(_) => self.put(b"Tool refused: outside workspace"),
            smith::Outcome::ReadOnly(_) => self.put(b"Tool refused: read only"),
            smith::Outcome::TooLong => self.put(b"Tool refused: path too long"),
            smith::Outcome::NotFound(_) => self.put(b"Tool refused: not found"),
            smith::Outcome::NotFile => self.put(b"Tool refused: not a file"),
            smith::Outcome::Linked => self.put(b"Tool refused: linked path"),
            smith::Outcome::Protected => self.put(b"Tool refused: protected path"),
            smith::Outcome::NotDirectory(_) => self.put(b"Tool refused: not a directory"),
            smith::Outcome::TooLarge(large) => {
                self.put(b"Tool refused: too large, ");
                self.number(large.size());
                self.put(b" bytes");
            }
            smith::Outcome::NotRead => self.put(b"Tool refused: not read"),
            smith::Outcome::Stale => self.put(b"Tool refused: stale read"),
            smith::Outcome::EditFailed(failed) => {
                self.put(b"Tool refused: edit ");
                self.number(u64::from(failed.edit()));
                match failed.why() {
                    smith::Mismatch::NoMatch => self.put(b", no match"),
                    smith::Mismatch::Ambiguous(ambiguous) => {
                        self.put(b", ambiguous, ");
                        self.number(u64::from(ambiguous.count()));
                        self.put(b" matches");
                    }
                    smith::Mismatch::Unchanged => self.put(b", unchanged"),
                }
            }
            smith::Outcome::Failed(failed) => {
                self.put(b"Tool failed: ");
                match failed.fault() {
                    smith::Fault::Denied => self.put(b"denied"),
                    smith::Fault::NoSpace => self.put(b"no space"),
                    smith::Fault::Other => self.put(b"other fault"),
                }
            }
            smith::Outcome::TimedOut => self.put(b"Tool timed out"),
            smith::Outcome::Cancelled => self.put(b"Tool cancelled"),
            smith::Outcome::Busy => self.put(b"Tool busy"),
            smith::Outcome::NulByte => self.put(b"Tool refused: NUL byte"),
        }
    }
    fn job(&mut self, job: &smith::JobRead) {
        self.put(b"Job ");
        self.number(u64::from(job.job()));
        match job.end() {
            smith::JobEnd::Running => {
                self.failed = false;
                self.put(b" answered running");
            }
            smith::JobEnd::Ended(ended) => {
                self.put(b" ended: ");
                self.exit(ended.exit());
            }
            smith::JobEnd::Stopped => {
                self.failed = false;
                self.put(b" stopped");
            }
            smith::JobEnd::Cancelled => self.put(b" cancelled"),
        }
        self.output(job.head(), job.last(), job.dropped());
    }
    fn command(&mut self, end: &smith::CommandEnd) {
        self.put(b"Command ended: ");
        self.exit(end.exit());
        self.output(end.head(), end.last(), end.dropped());
    }
    fn exit(&mut self, exit: &smith::Exit) {
        match exit {
            smith::Exit::Code(code) => {
                self.failed = code.code() != 0;
                self.put(b"exit ");
                self.number(u64::from(code.code()));
            }
            smith::Exit::Signal(signal) => {
                self.put(b"signal ");
                self.number(u64::from(signal.signal()));
            }
            smith::Exit::TimedOut => self.put(b"timed out"),
        }
    }
    fn output(&mut self, head: &[u8], last: &[u8], dropped: u64) {
        self.put(b"; output bytes ");
        let length = u64::try_from(head.len())
            .expect("schema bounded bytes")
            .checked_add(u64::try_from(last.len()).expect("schema bounded bytes"))
            .expect("schema bounded bytes");
        self.number(length);
        self.put(b"; dropped ");
        self.number(dropped);
    }
}
