//! A typed host-tool reply shares its mutation's commit barrier.
use super::{call_name, started_chat, take};
use crate::{Domain, Event, Limits, Output, Store, step};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_host as host;
use skein_lib::{Duration, Env, Queue, Token, Wall};

fn subscription_call(domain: &mut Domain, env: &Env<Limits>, owner: Token) -> (skein_lib::ReplyTo, core::CallKey) {
    step(
        domain,
        env,
        Event::Agent(host::FromAgent::Called {
            owner,
            call: call_name(1, 1, 0),
            ask: host::Ask::Relay {
                tool: b"subscribe".as_slice().into(),
                writes: true,
                input: br#"{"kind":"timer","at":10000000000}"#.as_slice().into(),
                deadline: Duration::from_secs(10),
            },
        }),
    );
    for output in take(domain, env) {
        if let Output::Now(core::Now::Call { to, run, attempt, .. }) = output {
            return (to, core::CallKey { task: run.raw(), attempt: attempt.raw(), completion: 1, position: 0 });
        }
    }
    panic!("the authenticated call leaves for decoding");
}

fn subscription_writes(mut writes: Queue<crate::Write>) {
    let mut call = false;
    let mut task = false;
    while let Some(write) = writes.pop() {
        match write {
            crate::Write::Save(crate::Record::Core(core::Record::Core(core::CoreRecord::Call(row)))) => {
                assert_eq!(row.part, core::CallPart::Subscribed { subscription: 1 });
                assert!(row.settled.is_none(), "the root commits the part before protocol rendering");
                call = true;
            }
            crate::Write::Save(crate::Record::Core(core::Record::Tasks(_))) => task = true,
            crate::Write::Wires
            | crate::Write::Save(_)
            | crate::Write::Erase(_)
            | crate::Write::CopySettingsRevision(_) => {}
        }
    }
    assert!(call && task, "the same transaction owns the mutation and reply part");
}

fn subscription_answer(outputs: Box<[Output]>, key: core::CallKey) {
    let mut answered = false;
    for output in outputs {
        match output {
            Output::Core(core::Held::CallAnswer { key: called, part, .. }) => {
                assert_eq!(called, key);
                assert_eq!(part, core::CallPart::Subscribed { subscription: 1 });
                answered = true;
            }
            Output::Commit { .. }
            | Output::Switch
            | Output::Load { .. }
            | Output::ReadResult { .. }
            | Output::CoreLoad(_)
            | Output::Core(_)
            | Output::Now(_)
            | Output::Observability(_)
            | Output::Infrastructure(_)
            | Output::ApplicationDocument { .. }
            | Output::ApplicationRead { .. }
            | Output::Application { .. }
            | Output::IncidentAssociationRefused { .. }
            | Output::Read { .. }
            | Output::Agent(_)
            | Output::ToChild(_)
            | Output::ProjectMade { .. }
            | Output::Stop => {}
        }
    }
    assert!(answered, "the typed part leaves after the transaction commits");
}

#[test]
fn a_tool_mutation_and_its_reply_part_share_one_commit() {
    let (mut domain, env, owner) = started_chat();
    let (to, key) = subscription_call(&mut domain, &env, owner);
    step(
        &mut domain,
        &env,
        Event::Core(core::Event::NamedAction {
            to,
            key,
            action: core::NamedAction::Subscribe {
                kind: jig_domain_tasks::SubscriptionKind::Timer { at: Wall::from_nanos(10_000_000_000), period: None },
            },
        }),
    );
    let mut commit = None;
    for output in take(&mut domain, &env) {
        if let Output::Commit { number, writes } = output {
            assert!(commit.replace(number).is_none(), "one decision makes one commit");
            subscription_writes(writes);
        } else {
            panic!("the reply part waits for its commit");
        }
    }
    step(&mut domain, &env, Event::Store(Store::Committed { number: commit.unwrap(), fill: 0 }));
    subscription_answer(take(&mut domain, &env), key);
}
