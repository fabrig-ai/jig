//! Presentation of committed owner results (client/domain/contract.md, 4;
//! client/domain/view.md, 7). Reads supplied facts only: no request, current
//! object, name lookup or explanation is used to fill a missing fact.
use crate::owner_outcome_words;
use crate::owner_outcome_words::OutcomePhrase as P;
use crate::words::Words;
use skein_ui::phrases::Hole;
use skein_ui::{Builder, Role, Run};

use jig_client::contract::{
    OwnerEscalationChoice, OwnerOutcome, OwnerProposalChoice, ProposalKey, Proposer, Revision, SeenThrough,
};
use skein_lib::Wall;

// Region + named status + title + confirmation, then a reason and whole name per host.
pub(crate) const MAX_NODES: u32 = 4 + 128 * 2;

/// A finite fragment envelope reusable by an owner's enclosing view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutcomeFragment {
    pub nodes: u32,
    pub text_bytes: u32,
    pub text_per_node: u32,
    pub depth: u32,
}
/// Maximum fragment over all committed kinds and every loaded language.
/// One region, one named status, and up to 128 configured hosts with whole names.
#[must_use]
pub fn required(words: &Words) -> Option<OutcomeFragment> {
    let mut capacity = 0_u32;
    for phrase in owner_outcome_words::ALL {
        capacity = capacity.max(owner_outcome_words::capacity(words, phrase));
    }
    let per_node = capacity.max(4096);
    Some(OutcomeFragment {
        nodes: MAX_NODES,
        text_bytes: per_node.checked_mul(MAX_NODES)?,
        text_per_node: per_node,
        depth: 3,
    })
}
fn header(value: &OwnerOutcome) -> P {
    match value {
        OwnerOutcome::Created { .. } => P::Created,
        OwnerOutcome::Compound { .. } => P::Compound,
        OwnerOutcome::MarkedSeen { .. } => P::MarkedSeen,
        OwnerOutcome::NoteEdited { .. } => P::NoteEdited,
        OwnerOutcome::ServiceMade { .. } => P::ServiceMade,
        OwnerOutcome::Adopted { .. } => P::Adopted,
        OwnerOutcome::Stopped { .. } => P::Stopped,
        OwnerOutcome::Cancelled { .. } => P::Cancelled,
        OwnerOutcome::Released { .. } | OwnerOutcome::Lifted { .. } => P::Released,
        OwnerOutcome::PersonTaken { .. } => P::PersonTaken,
        OwnerOutcome::PersonHandedBack { .. } => P::PersonHandedBack,
        OwnerOutcome::PersonAnswered { .. } => P::PersonAnswered,
        OwnerOutcome::Moved { .. } => P::Moved,
        OwnerOutcome::ProposalDecided { .. } => P::ProposalDecided,
        OwnerOutcome::Said { .. } => P::Said,
        OwnerOutcome::QuestionAnswered { .. } => P::QuestionAnswered,
        OwnerOutcome::Amended { .. } => P::Amended,
        OwnerOutcome::Proposed { .. } => P::Proposed,
        OwnerOutcome::Withdrawn { .. } => P::Withdrawn,
        OwnerOutcome::RolesSet { .. } => P::RolesSet,
        OwnerOutcome::Renamed { .. } => P::Renamed,
        OwnerOutcome::ProjectMade { .. } => P::ProjectMade,
        OwnerOutcome::PolicyChanged { .. } => P::PolicyChanged,
        OwnerOutcome::AccountProgramDeclared { .. } => P::AccountProgramDeclared,
        OwnerOutcome::AccountKeyKept { .. } => P::AccountKeyKept,
        OwnerOutcome::AccountMovedIn { renewed, .. } => {
            if *renewed {
                P::AccountRenewed
            } else {
                P::AccountMovedIn
            }
        }
        OwnerOutcome::AccountFinishing { .. } => P::AccountFinishing,
        OwnerOutcome::AccountChanged => P::AccountChanged,
        OwnerOutcome::AccountRemoved { .. } => P::AccountRemoved,
        OwnerOutcome::AccountSignInStarted { .. } => P::AccountSignInStarted,
        OwnerOutcome::AccountSignedOut { .. } => P::AccountSignedOut,
        OwnerOutcome::PoolSet { .. } => P::PoolSet,
        OwnerOutcome::EscalationDecided { .. } => P::EscalationDecided,
    }
}
fn text(b: &mut Builder, words: &Words, phrase: P, holes: &[Hole]) {
    let saying = owner_outcome_words::say(words, phrase, holes);
    b.text(Run::Plain, &saying);
}
fn number(b: &mut Builder, words: &Words, phrase: P, value: u64) {
    text(b, words, phrase, &[Hole::Number(value)]);
}
fn seen(b: &mut Builder, value: &SeenThrough, words: &Words) {
    match value {
        SeenThrough::Results { through } => number(b, words, P::ResultsThrough, *through),
        SeenThrough::Conversation { task: chat, through } => {
            number(b, words, P::Chat, *chat);
            number(b, words, P::RepliesThrough, *through);
        }
    }
}
fn proposal(b: &mut Builder, key: &ProposalKey, by: u64, at: Wall, choice: OwnerProposalChoice, words: &Words) {
    match key.proposer {
        Proposer::Task(task) => number(b, words, P::TaskProposer, task),
        Proposer::Party(party) => number(b, words, P::PartyProposer, party),
    }
    number(b, words, P::ProposalNumber, key.number);
    number(b, words, P::By, by);
    text(b, words, P::At, &[Hole::Wall(words.adjusted_wall(at))]);
    let phrase = match choice {
        OwnerProposalChoice::Accepted => P::Accepted,
        OwnerProposalChoice::Rejected => P::Rejected,
        OwnerProposalChoice::Passed => P::Passed,
        OwnerProposalChoice::Withdrawn => P::Withdrawn,
        OwnerProposalChoice::Stale => P::Stale,
    };
    text(b, words, phrase, &[]);
}
fn escalation(
    b: &mut Builder,
    task: u64,
    revision: Revision,
    by: u64,
    at: Wall,
    choice: OwnerEscalationChoice,
    words: &Words,
) {
    number(b, words, P::Task, task);
    number(b, words, P::Revision, revision.value);
    number(b, words, P::By, by);
    text(b, words, P::At, &[Hole::Wall(words.adjusted_wall(at))]);
    let phrase = match choice {
        OwnerEscalationChoice::Released => P::ReleaseChoice,
        OwnerEscalationChoice::Rejected => P::Rejected,
        OwnerEscalationChoice::Passed => P::Passed,
    };
    text(b, words, phrase, &[]);
}
fn details(b: &mut Builder, value: &OwnerOutcome, words: &Words) {
    match value {
        OwnerOutcome::AccountKeyKept { .. }
        | OwnerOutcome::AccountChanged
        | OwnerOutcome::AccountSignInStarted { .. }
        | OwnerOutcome::Lifted { .. } => {}
        OwnerOutcome::AccountFinishing { source } => number(b, words, P::SourceNumber, u64::from(*source)),
        OwnerOutcome::AccountMovedIn { account, .. } => number(b, words, P::AccountNumber, u64::from(*account)),
        OwnerOutcome::AccountProgramDeclared { source, name } => {
            number(b, words, P::SourceNumber, u64::from(*source));
            b.text(Run::Plain, name);
        }
        OwnerOutcome::AccountRemoved { confirmed, failed_hosts }
        | OwnerOutcome::AccountSignedOut { confirmed, failed_hosts } => {
            let status = if *confirmed { P::SignOutConfirmed } else { P::SignOutUnconfirmed };
            text(b, words, status, &[]);
            for host in &**failed_hosts {
                let why = match host.why {
                    jig_client::contract::AccountHostWhy::NotInContact => P::HostNotInContact,
                    jig_client::contract::AccountHostWhy::Lost => P::HostLost,
                    jig_client::contract::AccountHostWhy::Failed => P::HostFailed,
                };
                text(b, words, why, &[]);
                b.text(Run::Plain, &host.name);
            }
        }
        OwnerOutcome::Created { first, last, given } => created(b, *first, *last, *given, words),
        OwnerOutcome::MarkedSeen { seen: position } => seen(b, position, words),
        OwnerOutcome::NoteEdited { name } => number(b, words, P::Note, *name),
        OwnerOutcome::ServiceMade { person } => number(b, words, P::Person, *person),
        OwnerOutcome::Adopted { project }
        | OwnerOutcome::Renamed { project }
        | OwnerOutcome::ProjectMade { project }
        | OwnerOutcome::RolesSet { project }
        | OwnerOutcome::PolicyChanged { project } => number(b, words, P::Project, u64::from(*project)),
        OwnerOutcome::Stopped { task }
        | OwnerOutcome::Cancelled { task }
        | OwnerOutcome::Released { task }
        | OwnerOutcome::PersonTaken { task }
        | OwnerOutcome::PersonHandedBack { task }
        | OwnerOutcome::PersonAnswered { task }
        | OwnerOutcome::Moved { task } => number(b, words, P::Task, *task),
        OwnerOutcome::ProposalDecided { proposal: key, by, at, choice } => proposal(b, key, *by, *at, *choice, words),
        OwnerOutcome::Compound { project, operations, first_message } => {
            number(b, words, P::Project, u64::from(*project));
            number(b, words, P::Operations, u64::from(*operations));
            number(b, words, P::Message, *first_message);
        }
        OwnerOutcome::Said { task, message } => {
            number(b, words, P::Task, *task);
            number(b, words, P::Message, *message);
        }
        OwnerOutcome::QuestionAnswered { task, question, message } => {
            number(b, words, P::Task, *task);
            number(b, words, P::Question, *question);
            number(b, words, P::Message, *message);
        }
        OwnerOutcome::Amended { task } => {
            number(b, words, P::Task, *task);
        }
        OwnerOutcome::Proposed { proposal, .. } | OwnerOutcome::Withdrawn { proposal } => {
            number(b, words, P::ProposalNumber, *proposal);
        }
        OwnerOutcome::PoolSet { project, person } => {
            number(b, words, P::Project, u64::from(*project));
            number(b, words, P::Person, *person);
        }
        OwnerOutcome::EscalationDecided { task, revision, by, at, choice } => {
            escalation(b, *task, *revision, *by, *at, *choice, words);
        }
    }
}
/// Build one bounded fragment using only the committed value and selected Words.
pub fn render(b: &mut Builder, value: &OwnerOutcome, words: &Words) {
    b.open(Role::Region);
    let title = owner_outcome_words::say(words, header(value), &[]);
    b.open(Role::Status);
    b.name(&title);
    b.text(Run::Plain, &title);
    b.close();
    details(b, value, words);
    b.close();
}

fn created(b: &mut Builder, first: u64, last: u64, given: Option<jig_client::contract::CreatedGiven>, words: &Words) {
    number(b, words, P::FirstTask, first);
    number(b, words, P::LastTask, last);
    if let Some(given) = given {
        number(b, words, P::GivenTask, given.task);
        number(b, words, P::GivenAmount, given.amount);
    }
}
