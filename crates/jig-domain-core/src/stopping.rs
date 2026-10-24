//! Admission while the engine drains (domain/engine.md, 6.1).
use crate::{Core, Event, Held, Now, Request, Requests, Timer};
use alloc::boxed::Box;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_views as views;
use skein_lib::{Queue, ReplyTo, Token};

impl Core {
    /// Whether new work is refused while the service drains.
    #[must_use]
    pub fn stopping(&self) -> bool {
        self.stopping
    }
}

pub(crate) fn start(core: &mut Core) -> Requests {
    core.stopping = true;
    core.draining = Some(true);
    empty()
}

pub(crate) fn empty() -> Requests {
    answer(None)
}
fn answer(value: Option<Request>) -> Requests {
    let mut out = Queue::with_capacity(3);
    if let Some(value) = value {
        out.push(value);
    }
    out.push(Request::Decided);
    Requests::Out(out)
}
fn party(to: ReplyTo) -> Requests {
    answer(Some(Request::Held(Box::new(Held::PeopleReply {
        to,
        reply: people::Reply::Again(people::Again::NotReady),
    }))))
}
fn watch(watcher: Token) -> Requests {
    answer(Some(Request::Held(Box::new(Held::WatchEnded { watcher, why: views::End::Stopping }))))
}
fn call(to: ReplyTo) -> Requests {
    answer(Some(Request::Now(Box::new(Now::CallBusy { to }))))
}

pub(crate) fn gate(core: &mut Core, event: Event) -> Result<Event, Requests> {
    if !core.stopping {
        return Ok(event);
    }
    match event {
        Event::ChangeSettings { reply_to, .. }
        | Event::ProposeSettings { reply_to, .. }
        | Event::SignIn { reply_to, .. } => Err(party(reply_to)),
        Event::Act { reply_to, carried, .. } => Err(crate::routing::application_routes::first(
            crate::routing::application_routes::number(carried),
            answer(Some(Request::Held(Box::new(Held::Acted {
                to: reply_to,
                outcome: people::Outcome::Again(people::Again::NotReady),
            })))),
        )),
        Event::Key { reply_to, carried, .. } => {
            Err(crate::routing::application_routes::first(Some(carried.number), party(reply_to)))
        }
        Event::Application { reply_to, connector, owner, carried, .. } => {
            let unused = crate::routing::application_routes::number(carried);
            let Requests::Out(mut out) = crate::routing::application_routes::first(unused, party(reply_to));
            if core.connectors.contains(&connector) {
                out.push(Request::Ask { connector, ask: crate::Ask::DropApplication { owner } });
            }
            Err(Requests::Out(out))
        }
        Event::People(event) => people_gate(event),
        Event::Watch { watcher, .. } | Event::WatchRun { watcher, .. } => Err(watch(watcher)),
        Event::NamedAction { to, .. }
        | Event::NamedAnswer { to, .. }
        | Event::DelegateValidated { to, .. }
        | Event::DelegateInputFailed { to, .. }
        | Event::SettledCall { to, .. } => Err(call(to)),
        Event::Fleet(fleet::Event::Hello { worker: 0, channel, .. }) => {
            Err(answer(Some(Request::Held(Box::new(Held::Refuse { channel })))))
        }
        Event::Fleet(event) => Ok(Event::Fleet(event)),
        Event::Stopping
        | Event::Activate { .. }
        | Event::PreparedAgent { .. }
        | Event::StartBrief { .. }
        | Event::BriefNotes { .. }
        | Event::BriefAssembled { .. }
        | Event::WorkspacePrepared { .. }
        | Event::ClaimPrepared { .. }
        | Event::ClaimRefused { .. }
        | Event::PreparationFailed { .. }
        | Event::TranscriptUnread { .. }
        | Event::ProcedureStep { .. }
        | Event::ProcedureHoldings { .. }
        | Event::StartRecurring { .. }
        | Event::Period { .. } => Err(empty()),
        event @ (Event::Fill { .. }
        | Event::Marked { .. }
        | Event::ReadLoaded { .. }
        | Event::Read { .. }
        | Event::ApplicationPart { .. }
        | Event::ApplicationFinished { .. }
        | Event::ProjectionSettled { .. }
        | Event::EffectStart { .. }
        | Event::SubscribeTopic { .. }
        | Event::EffectConnector(_)
        | Event::EffectDeadline
        | Event::Unwatch { .. }
        | Event::Delivered { .. }
        | Event::StreamDelivered { .. }
        | Event::RunCall { .. }
        | Event::RunText { .. }
        | Event::Tasks(_)
        | Event::ConnectorRepair { .. }
        | Event::PersonProposal(_)
        | Event::PersonEscalation(_)
        | Event::TaskEscalation(_)
        | Event::TurnPayload { .. }
        | Event::AnswerPayload { .. }
        | Event::AcceptedTurn { .. }
        | Event::RefusedPayload { .. }
        | Event::Forget { .. }
        | Event::EndedReadLoaded { .. }
        | Event::SeenConversationLoaded { .. }
        | Event::HistoricalProposal { .. }
        | Event::DecidedProposal { .. }
        | Event::HistoricalPersonTask { .. }
        | Event::PartyInputsLoaded { .. }
        | Event::EndedTaskLoaded { .. }
        | Event::HistoricalEscalation { .. }
        | Event::DelegateHoldings { .. }
        | Event::Brief(_)
        | Event::Holdings { .. }
        | Event::Account(_)
        | Event::View(_)
        | Event::Notes(_)
        | Event::SettingsCommit { .. }) => Ok(event),
    }
}
fn people_gate(event: people::Event) -> Result<Event, Requests> {
    match event {
        people::Event::Act { reply_to, .. } => Err(answer(Some(Request::Held(Box::new(Held::Acted {
            to: reply_to,
            outcome: people::Outcome::Again(people::Again::NotReady),
        }))))),
        people::Event::Whose { reply_to, .. }
        | people::Event::Ask { reply_to, .. }
        | people::Event::SignOut { reply_to, .. } => Err(party(reply_to)),
        people::Event::ConversationSeen { .. } => Err(empty()),
        event @ (people::Event::ChatState { .. }
        | people::Event::MoveChat { .. }
        | people::Event::Forget { .. }
        | people::Event::MakeService { .. }
        | people::Event::Seed { .. }
        | people::Event::Waiting { .. }
        | people::Event::ApplyRoles { .. }
        | people::Event::SignedIn { .. }
        | people::Event::Roles { .. }
        | people::Event::Decided { .. }
        | people::Event::Restore { .. }
        | people::Event::Restored) => Ok(Event::People(event)),
    }
}
impl Timer {
    /// Whether the core pauses this child timer while the service stops.
    #[must_use]
    pub fn waits_while_stopping(self) -> bool {
        waits(self)
    }
}

pub(crate) fn waits(timer: Timer) -> bool {
    match timer {
        Timer::People | Timer::Tasks | Timer::Fleet | Timer::Brief => true,
        Timer::Account | Timer::View => false,
    }
}

/// The journal holds this terminal behind every earlier commit.
pub(crate) fn drained(core: &mut Core, out: &mut Queue<Request>) {
    if core.draining == Some(true) && core.view_subjects.is_empty() {
        core.draining = Some(false);
        out.push(Request::Held(Box::new(Held::Drained)));
    }
}
