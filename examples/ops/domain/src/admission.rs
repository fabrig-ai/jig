//! Admission by journal backlog, keeping the existing zero-room traffic class
//! (skein's draft `dynamic-memory.md`, 4).
use crate::{Domain, Event, Limits, step};
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_ops_domain_observability as observability;
use skein_lib::Env;

/// The root's entrance: new work waits at the watermark and is handed
/// back unchanged; completions of admitted work are always taken
/// (skein's draft `dynamic-memory.md`, 4.2 and 4.3).
#[expect(clippy::result_large_err, reason = "the service admission boundary returns the original event by value")]
pub fn try_step(domain: &mut Domain, env: &Env<Limits>, event: Event) -> Result<(), Event> {
    match event {
        event @ (Event::Store(_)
        | Event::Released(_)
        | Event::ConnectorRead { .. }
        | Event::Core(core::Event::Fleet(fleet::Event::Relay {
            call: fleet::Call { writes: false, .. },
            ..
        }))
        | Event::Host(host::Event::Called { ask: host::Ask::Relay { writes: false, .. }, .. })
        | Event::Observability(
            observability::Event::Read { .. }
            | observability::Event::System(observability::SystemEvent::ReadDone { .. }),
        )) => {
            step(domain, env, event);
            Ok(())
        }
        event @ (Event::WatchApplication { .. }
        | Event::ReadApplication { .. }
        | Event::Application { .. }
        | Event::Core(_)
        | Event::Host(_)
        | Event::Observability(_)
        | Event::Infrastructure(_)
        | Event::Agent(_)
        | Event::AgentContent { .. }
        | Event::Restart { .. }
        | Event::Timer(_)
        | Event::ObservabilityTimer
        | Event::InfrastructureTimer
        | Event::TurnTimer
        | Event::ResultRead { .. }
        | Event::CallAnswered { .. }
        | Event::Effect { .. }) => {
            if !admits(domain, &env.limits) {
                return Err(event);
            }
            step(domain, env, event);
            Ok(())
        }
    }
}

/// What the root holds of admitted work, in entries (the draft's 4.1).
pub(crate) fn backlog(domain: &Domain) -> u64 {
    domain.journal.backlog()
}

/// New work enters only while the journal runs and the backlog is under
/// the watermark.
pub(crate) fn admits(domain: &Domain, limits: &Limits) -> bool {
    !domain.journal.stopped() && backlog(domain) < u64::from(limits.watermark)
}
