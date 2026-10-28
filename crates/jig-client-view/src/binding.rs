//! Owner tokens carried unchanged by the tree (client/domain/view.md, section 5).

use crate::Owner;
use alloc::boxed::Box;
use skein_lib::Token;
use skein_ui::{Act, Binding, number};

/// A typed meaning belonging to jig's last built tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JigMeaning {
    /// One typed Project settings field, preserving other whole channels.
    ProjectSetting { ask: Box<jig_client::contract::Ask>, field: crate::owner_project::Field },
    /// A new service name, after an explicit supplied role choice.
    ProjectServiceName { project: u32, role: jig_client::contract::HeldRole },
    /// An explicit budget for one supplied party in the configured target.
    ProjectPoolBudget { project: u32, party: u64 },
    /// Pressing, submitting or toggling asks this already typed action.
    Action(crate::action::Act),
    /// Whole text edits a domain field; submit names that same field.
    Field(crate::fields::Purpose),
    /// Digits are kept in the view and submitting goes to that task.
    TaskNumber,
    /// Digits edit the confirmation's priority without a hidden submit.
    Priority,
    /// Source values and selected replacements kept separately for one named field.
    Amendment {
        before: Box<jig_client::contract::Amendment>,
        change: Box<jig_client::contract::Amendment>,
        field: crate::confirmation::amendment::Field,
    },
    /// A typed project goal form field.
    GoalField { form: crate::page::project::goals::GoalForm, field: crate::sections::project::goals::Field },
    /// Description search changes the current note scope only.
    NoteSearch { filter: crate::page::project::NoteFilter },
    /// A connector-owned part referred to by a slot, never an actionable node.
    Part(jig_client::contract::Part),
    /// A whole exact owner reference, with literal path segment boundaries.
    OwnerPart(jig_client::contract::OwnerPart),
}
#[derive(Debug)]
struct Entry {
    token: Token,
    meaning: JigMeaning,
}
/// Bounded meanings for one tree; rebuilding retires every old token.
/// Empty + begin -> building; insert -> issued; decode -> typed action or drop.
/// The next token never resets, so a token from any earlier tree stays stale.
#[derive(Debug)]
pub struct JigBindings {
    entries: skein_lib::List<Entry>,
    next: u64,
    text_limit: u32,
    task: Option<u64>,
}
impl JigBindings {
    /// Allocate the most meanings that a tree may carry.
    #[must_use]
    pub fn new(capacity: u32, text_limit: u32) -> Self {
        Self { entries: skein_lib::List::with_capacity(capacity), next: 0, text_limit, task: None }
    }
    /// The client's bound on one literal segment's bytes.
    #[must_use]
    pub const fn text_limit(&self) -> u32 {
        self.text_limit
    }
    /// Retire the prior tree's meanings before building its replacement.
    pub fn begin(&mut self) {
        self.entries.clear();
    }
    /// Issue one token; exceeding the derived capacity is a view bug.
    pub fn insert(&mut self, meaning: JigMeaning) -> Binding {
        self.next = self.next.checked_add(1).expect("view token space remains");
        let token = Token::new(self.next);
        self.entries.push(Entry { token, meaning }).expect("bindings fit their derived capacity");
        Binding { owner: Owner::Jig.opaque(), token }
    }
    /// Resolve a part slot's token for its connector's view root.
    #[must_use]
    pub fn part(&self, token: Token) -> Option<&jig_client::contract::Part> {
        for entry in &self.entries {
            if entry.token == token {
                match &entry.meaning {
                    JigMeaning::Part(part) => return Some(part),
                    JigMeaning::OwnerPart(_)
                    | JigMeaning::Action(_)
                    | JigMeaning::Field(_)
                    | JigMeaning::TaskNumber
                    | JigMeaning::Priority
                    | JigMeaning::Amendment { .. }
                    | JigMeaning::ProjectSetting { .. }
                    | JigMeaning::ProjectServiceName { .. }
                    | JigMeaning::ProjectPoolBudget { .. }
                    | JigMeaning::GoalField { .. }
                    | JigMeaning::NoteSearch { .. } => return None,
                }
            }
        }
        None
    }
    /// Resolve an exact reference slot's raw key; scope is supplied by its producer.
    #[must_use]
    pub fn owner_part(&self, token: Token) -> Option<&jig_client::contract::OwnerPart> {
        for entry in &self.entries {
            if entry.token == token {
                match &entry.meaning {
                    JigMeaning::OwnerPart(part) => return Some(part),
                    JigMeaning::Part(_)
                    | JigMeaning::Action(_)
                    | JigMeaning::Field(_)
                    | JigMeaning::TaskNumber
                    | JigMeaning::Priority
                    | JigMeaning::Amendment { .. }
                    | JigMeaning::ProjectSetting { .. }
                    | JigMeaning::ProjectServiceName { .. }
                    | JigMeaning::ProjectPoolBudget { .. }
                    | JigMeaning::GoalField { .. }
                    | JigMeaning::NoteSearch { .. } => return None,
                }
            }
        }
        None
    }
    /// A link's typed address for the platform's address translation.
    #[must_use]
    pub fn address(&self, token: Token) -> Option<crate::address::Address> {
        for entry in &self.entries {
            if entry.token == token {
                match &entry.meaning {
                    JigMeaning::Action(action) => match action {
                        crate::action::Act::Go { address } => return Some(*address),
                        crate::action::Act::OpenChat { task } => {
                            return Some(crate::address::Address::Task {
                                task: *task,
                                section: crate::address::TaskSection::Activity,
                            });
                        }
                        crate::action::Act::Edit { .. }
                        | crate::action::Act::Submit { .. }
                        | crate::action::Act::AmendRelease { .. }
                        | crate::action::Act::CloseChat { .. }
                        | crate::action::Act::Intend { .. }
                        | crate::action::Act::Choose { .. }
                        | crate::action::Act::CannotDo
                        | crate::action::Act::Again { .. }
                        | crate::action::Act::Dispose { .. }
                        | crate::action::Act::Change { .. }
                        | crate::action::Act::WriteRelease { .. }
                        | crate::action::Act::Priority { .. }
                        | crate::action::Act::MoveGoal { .. }
                        | crate::action::Act::ProposeInstead { .. }
                        | crate::action::Act::ChatAbout { .. }
                        | crate::action::Act::LookAgain { .. }
                        | crate::action::Act::Confirm
                        | crate::action::Act::Dismiss
                        | crate::action::Act::Fold { .. }
                        | crate::action::Act::FoldCall { .. }
                        | crate::action::Act::FoldCurrentCall { .. }
                        | crate::action::Act::Filter { .. }
                        | crate::action::Act::More { .. }
                        | crate::action::Act::FilterInbox { .. }
                        | crate::action::Act::OpenUpdates
                        | crate::action::Act::BeginConversation { .. }
                        | crate::action::Act::ProjectSettings { .. }
                        | crate::action::Act::ProjectServiceRole { .. }
                        | crate::action::Act::GoalForm { .. }
                        | crate::action::Act::FilterTasks { .. }
                        | crate::action::Act::FilterNotes { .. }
                        | crate::action::Act::FilterPlan { .. }
                        | crate::action::Act::HistoryScope { .. }
                        | crate::action::Act::OpenEnded { .. }
                        | crate::action::Act::SignOut => return None,
                    },
                    JigMeaning::Field(_)
                    | JigMeaning::TaskNumber
                    | JigMeaning::Part(_)
                    | JigMeaning::OwnerPart(_)
                    | JigMeaning::Priority
                    | JigMeaning::Amendment { .. }
                    | JigMeaning::ProjectSetting { .. }
                    | JigMeaning::ProjectServiceName { .. }
                    | JigMeaning::ProjectPoolBudget { .. }
                    | JigMeaning::GoalField { .. }
                    | JigMeaning::NoteSearch { .. } => return None,
                }
            }
        }
        None
    }
}
/// Decode an act only through a token issued for the last tree.
/// Numeric input is retained here until submit; no navigation happens while typing.
#[must_use]
pub fn decode(view: &mut JigBindings, token: Token, act: Act) -> Option<crate::action::Act> {
    for entry in &view.entries {
        if entry.token == token {
            return match &entry.meaning {
                JigMeaning::Part(_) | JigMeaning::OwnerPart(_) => None,
                JigMeaning::ProjectPoolBudget { project, party } => match act {
                    Act::Input { value } => {
                        Some(crate::page::project::owner_form::action(jig_client::contract::Ask::SetPool {
                            project: *project,
                            party: *party,
                            budget: number(&value)?,
                        }))
                    }
                    Act::Press | Act::Submit | Act::Toggle | Act::Drawing { .. } => None,
                },
                JigMeaning::ProjectServiceName { project, role } => match act {
                    Act::Input { value } => {
                        Some(crate::page::project::owner_form::action(jig_client::contract::Ask::MakeService {
                            project: *project,
                            role: *role,
                            name: jig_client::contract::Text::new(value, view.text_limit).ok()?,
                        }))
                    }
                    Act::Press | Act::Submit | Act::Toggle | Act::Drawing { .. } => None,
                },
                JigMeaning::ProjectSetting { ask, field } => match act {
                    Act::Input { value } => crate::owner_project::edit(ask, *field, value, view.text_limit),
                    Act::Press | Act::Submit | Act::Toggle | Act::Drawing { .. } => None,
                },
                JigMeaning::GoalField { form, field } => match act {
                    Act::Input { value } => crate::sections::project::goals::edit(*form, *field, &value),
                    Act::Press | Act::Submit | Act::Toggle | Act::Drawing { .. } => None,
                },
                JigMeaning::NoteSearch { filter } => match act {
                    Act::Input { value } => Some(crate::action::Act::FilterNotes {
                        filter: crate::page::project::NoteFilter {
                            scope: filter.scope.clone(),
                            description: if value.is_empty() {
                                None
                            } else {
                                Some(jig_client::contract::Text::new(value, view.text_limit).ok()?)
                            },
                        },
                    }),
                    Act::Press | Act::Submit | Act::Toggle | Act::Drawing { .. } => None,
                },
                JigMeaning::Priority => match act {
                    Act::Input { value } => {
                        Some(crate::action::Act::Priority { priority: u32::try_from(number(&value)?).ok()? })
                    }
                    Act::Press | Act::Submit | Act::Toggle | Act::Drawing { .. } => None,
                },
                JigMeaning::Amendment { before, change, field } => match act {
                    Act::Input { value } => {
                        crate::confirmation::amendment::edit(before, change, *field, value, view.text_limit)
                    }
                    Act::Press | Act::Submit | Act::Toggle | Act::Drawing { .. } => None,
                },
                JigMeaning::Action(action) => match act {
                    Act::Press | Act::Submit | Act::Toggle => Some(action.clone()),
                    Act::Input { .. } | Act::Drawing { .. } => None,
                },
                JigMeaning::Field(field) => match act {
                    Act::Input { value } => {
                        let text = jig_client::contract::Text::new(value, view.text_limit).ok()?;
                        Some(crate::action::Act::Edit { field: *field, text })
                    }
                    Act::Submit => Some(crate::action::Act::Submit { field: *field }),
                    Act::Press | Act::Toggle | Act::Drawing { .. } => None,
                },
                JigMeaning::TaskNumber => match act {
                    Act::Input { value } => {
                        view.task = number(&value);
                        None
                    }
                    Act::Submit => Some(crate::action::Act::Go { address: crate::address::Address::task(view.task?) }),
                    Act::Press | Act::Toggle | Act::Drawing { .. } => None,
                },
            };
        }
    }
    None
}
