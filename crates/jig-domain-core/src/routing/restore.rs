//! A keyed history sweep proposes earlier choices to their owners (engine.md, 17.10).
use crate::{Core, Event, Limits, Request, Requests, reads, settings as s};
use alloc::boxed::Box;
use jig_domain_people as people;
use skein_lib::{Decimal, Env, List, Map, Queue, Token, Writer};

pub(super) struct Input {
    pub request: Token,
    pub person: u64,
    pub revision: u32,
    pub reason: Box<[u8]>,
}
pub(super) fn start(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    input: Input,
) {
    let Input { request, person, revision, reason } = input;
    let newest = core.current.newest();
    let oldest = reads::settings::oldest(core, &env.limits);
    let outcome = if core.restoring.is_some() || core.reads.len() >= env.limits.load_slots {
        Some(people::Outcome::Again(people::Again::Busy))
    } else if revision == 0 || revision > newest {
        Some(people::Outcome::Refused(people::Refusal::Unknown))
    } else if revision < oldest {
        Some(people::Outcome::Refused(people::Refusal::Forgotten { oldest }))
    } else if revision == newest {
        Some(people::Outcome::Refused(people::Refusal::NotWellFormed {
            at: 0,
            why: people::Malformed::SettingsUnchanged,
        }))
    } else {
        None
    };
    if let Some(outcome) = outcome {
        work.push(Event::People(people::Event::Decided { request, outcome }));
        return;
    }
    core.restoring = Some(s::Restoring {
        request,
        person,
        revision,
        newest,
        before: None,
        load: None,
        reason,
        items: Map::with_capacity(
            s::capacity(&env.limits).expect("settings capacity").min(jig_domain_tasks::SETTINGS_ITEMS),
        ),
    });
    load(core, env, work, out);
}

fn finish(core: &mut Core, work: &mut Queue<Event>, outcome: people::Outcome) {
    let flight = core.restoring.take().expect("restore flight");
    work.push(Event::People(people::Event::Decided { request: flight.request, outcome }));
}
fn busy(core: &mut Core, work: &mut Queue<Event>) {
    finish(core, work, people::Outcome::Again(people::Again::Busy));
}
fn load(core: &mut Core, env: &Env<Limits>, work: &mut Queue<Event>, out: &mut Queue<Request>) {
    let flight = core.restoring.as_ref().expect("restore flight");
    let (first, last) = reads::settings::page_range(
        flight.newest,
        flight.revision.checked_add(1).expect("earlier revision"),
        flight.before,
        env.limits.documents.rows,
    )
    .expect("remaining history page");
    let rows = last.checked_sub(first).expect("ordered page").checked_add(1).expect("nonempty page");
    let range = reads::Range {
        party: flight.person,
        scope: reads::Scope::SettingsHistory { first, last, newest: flight.newest, whole: false },
        after: None,
        backwards: false,
        rows,
        bytes: reads::settings::load_bytes(&env.limits, rows).expect("validated page bytes"),
    };
    match reads::load_restore(core, range) {
        Some(request) => out.push(request),
        None => busy(core, work),
    }
}

pub(crate) fn loaded(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    range: reads::Range,
    result: reads::Loaded,
) -> Requests {
    let mut out = Queue::with_capacity(2);
    let Some(flight) = &core.restoring else { return Requests::Out(out) };
    if flight.newest == core.current.newest() {
        let result = page(core, env, work, range, result);
        if let Err(outcome) = result {
            finish(core, work, outcome);
        } else if core.restoring.is_some() && core.settings_offering.is_none() {
            load(core, env, work, &mut out);
        }
    } else {
        let newest = core.current.newest();
        finish(core, work, people::Outcome::Refused(people::Refusal::Stale { newest }));
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

fn page(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    range: reads::Range,
    result: reads::Loaded,
) -> Result<(), people::Outcome> {
    let reads::Loaded::Page { rows, next } = result else {
        return Err(people::Outcome::Again(people::Again::Busy));
    };
    let Ok(mut records) = reads::settings::whole_records(&env.limits, &range, rows, next) else {
        return Err(people::Outcome::Again(people::Again::Busy));
    };
    let reads::Scope::SettingsHistory { first, last, .. } = range.scope else { unreachable!("history destination") };
    let flight = core.restoring.as_mut().expect("restore flight");
    for number in (first..=last).rev() {
        let Some(record) = records.remove(&number) else {
            return Err(people::Outcome::Again(people::Again::Busy));
        };
        // Descending records replace the more recent before-state with the earlier one.
        for item in record.items {
            let edit = match item.replaced {
                s::Replaced::Value(value) => s::Edit::Set(*value),
                s::Replaced::Default | s::Replaced::NotOffered => s::Edit::Reset,
                s::Replaced::Removed => s::Edit::Removed,
            };
            if flight.items.insert(item.setting, edit).is_err() {
                return Err(super::settings::malformed(s::Failure { setting: item.setting, fault: s::Fault::Count }));
            }
        }
        if let Err(failure) = s::restore_count(&flight.items, &env.limits) {
            return Err(super::settings::malformed(failure));
        }
    }
    flight.before = Some(first);
    if first > flight.revision.checked_add(1).expect("earlier revision") {
        return Ok(());
    }
    propose(core, env, work)
}
fn propose(core: &mut Core, env: &Env<Limits>, work: &mut Queue<Event>) -> Result<(), people::Outcome> {
    let flight = core.restoring.as_ref().expect("restore flight");
    let person = flight.person;
    let request = flight.request;
    let against = flight.newest;
    let reason = reason(flight.revision, &flight.reason, env.limits.people.words)
        .ok_or(people::Outcome::Refused(people::Refusal::Limit { which: people::LimitKind::Words }))?;
    // Roles may have changed during the loads; admission remains the people's.
    let mut admitted = core.people.role(person, people::DEPLOYMENT).is_some();
    if let Some(roles) = core.people.project_roles(person) {
        admitted |= !roles.is_empty();
    }
    if !admitted {
        return Err(people::Outcome::Refused(people::Refusal::Role { held: None }));
    }
    let flight = core.restoring.as_mut().expect("restore flight");
    let mut items = List::with_capacity(flight.items.len());
    while let Some((&setting, _)) = flight.items.first() {
        let edit = flight.items.remove(&setting).expect("folded setting");
        items.push(s::Item { setting, edit }).expect("same fold capacity");
    }
    let items = items.into_boxed();
    core.proposed_input = Some(if s::changes_current(&core.current, &items) {
        match s::change_shape(&items, &env.limits) {
            Ok(()) => Ok(Box::new(s::Proposal { against, items })),
            Err(failure) => Err(people::Malformed::Settings { setting: failure.setting, fault: failure.fault }),
        }
    } else {
        Err(people::Malformed::SettingsUnchanged)
    });
    super::settings::propose_admitted(core, env, work, request, person, against, reason);
    if core.settings_offering.is_none() {
        drop(core.restoring.take());
    }
    Ok(())
}

fn reason(revision: u32, words: &[u8], bound: u32) -> Option<Box<[u8]>> {
    const PREFIX: &[u8] = b"restores revision ";
    let revision = Decimal::of(u64::from(revision));
    let prefix = PREFIX.len().checked_add(revision.as_bytes().len())?;
    let bound = usize::try_from(bound).ok()?;
    if prefix > bound {
        return None;
    }
    let mut keep = words.len().min(bound.saturating_sub(prefix.checked_add(2)?));
    while keep > 0 && keep < words.len() && words.get(keep).expect("inside reason") & 0xc0 == 0x80 {
        keep = keep.checked_sub(1).expect("nonzero prefix");
    }
    let separator: &[u8] = if keep == 0 { b"" } else { b": " };
    let length = prefix.checked_add(separator.len())?.checked_add(keep)?;
    let mut writer = Writer::new(length);
    writer.put(PREFIX).expect("measured reason");
    writer.put(revision.as_bytes()).expect("measured revision");
    writer.put(separator).expect("measured separator");
    writer.put(words.get(..keep).expect("bounded UTF-8 prefix")).expect("bounded reason");
    Some(writer.finish())
}
