//! Refusal cleanup through the service's actual handler and routed store answers.
use super::*;
use crate::tests::fixtures;
use skein_lib::{Duration, bytes};

#[test]
fn a_revoked_record_waits_for_erased_and_coalesces_a_sign_out() {
    assert_eq!(
        Map::<u32, Erasing>::worst_case(2),
        Map::<u32, Option<bool>>::worst_case(2),
        "the generalized pending state fits the existing map bound"
    );
    for (notifications, expected_signed_out, removing) in
        [(&[][..], None, false), (&[true, true][..], Some(true), true), (&[false, false][..], Some(false), true)]
    {
        let mut service = fixtures::started(&fixtures::accounts_document());
        fixtures::finish_start(&mut service);
        let engine = fixtures::engine(&mut service);
        let mut accounts = engine.accounts.take().expect("accounts runtime");
        let mut events = Queue::with_capacity(16);
        let old = 1;
        let fresh = 9001;
        let candidate = accounts.env.limits.client.document;
        let old_record = record(old, &candidate);
        let records::Next::Store(old_keep) = accounts.records.keep(old, old_record, &candidate) else {
            panic!("explicit old source fixture");
        };
        replace(&mut engine.store, old_keep);
        assert!(accounts.records.kept(old).is_some());
        let prior = accounts.required.insert(old, Box::default()).expect("explicit old source");
        assert!(prior.is_none());
        let mut record = record(fresh, &candidate);
        let records::Next::Store(keep) = accounts.records.keep(fresh, record.clone(), &candidate) else {
            panic!("fresh source is kept");
        };
        replace(&mut engine.store, keep);
        assert!(accounts.records.kept(fresh).is_some());
        // Hold unrelated store answers apart from this erase's terminal.
        while engine.store.above.pop().is_some() {}
        engine
            .values
            .accounts
            .granted(
                fresh,
                2,
                table::Value { token: Secret::new(bytes::copy_of(b"fresh-token")), scheme: table::Scheme::Bearer },
                Time::from_nanos(Duration::from_secs(60).as_nanos()),
            )
            .expect("fresh value fits");
        accounts.above.push(driver::Event::Failed {
            account: old,
            ends: driver::Ends::Refresh,
            failure: driver::Failure::Exchange(oauth::Failure::Refused),
        });
        accounts.iterate(Time::from_nanos(1), Wall::from_nanos(1), &mut engine.values, &mut events, &mut engine.store);
        let Some(root::Event::Core(core::Event::Account(domain::Event::Component(domain::FromComponent::Failed {
            account: 1,
            failure: domain::Failure::SignInAgain,
        })))) = events.pop()
        else {
            panic!("revocation fails needs sign-in immediately");
        };
        assert!(engine.values.accounts.value(old, 1, Time::from_nanos(1)).is_none());
        let Some(Erasing::Revoked) = accounts.erasing.get(&old) else {
            panic!("erase awaits durability");
        };
        record.key = old;
        let records::Next::Store(before) = accounts.records.keep(old, record.clone(), &candidate) else {
            panic!("the old slot remains admitted");
        };
        assert!(before.replaces.is_some(), "submission did not clear the held digest");
        assert!(accounts.records.stale(old).is_some());
        for confirmed in notifications {
            accounts.above.push(driver::Event::SignedOut { account: old, revoked: *confirmed });
        }
        accounts.iterate(Time::from_nanos(1), Wall::from_nanos(1), &mut engine.values, &mut events, &mut engine.store);
        assert_eq!(engine.store.above.len(), 1, "one erase for revocation and concurrent sign-out");
        assert!(events.is_empty(), "sign-out awaits erasure");
        let terminal = engine.store.above.pop().expect("held erase acknowledgement");
        let store::Event::Erased { .. } = terminal else {
            panic!("held erase acknowledgement");
        };
        engine.store.sources.push(terminal);
        accounts.answers(&mut engine.store, &mut events);
        // A duplicate acknowledgement and an unknown old-source acknowledgement do nothing.
        engine.store.sources.push(store::Event::Erased { owner: Token::new(PREFIX | u64::from(old)) });
        engine.store.sources.push(store::Event::Erased { owner: Token::new(PREFIX | u64::from(8000_u32)) });
        accounts.answers(&mut engine.store, &mut events);
        assert_eq!(signed_out(&mut events, old), expected_signed_out);
        let records::Next::Store(after) = accounts.records.keep(old, record, &candidate) else {
            panic!("erasure retained the slot");
        };
        assert_eq!(after.replaces, None, "only the acknowledgement cleared the digest");
        assert!(accounts.required.contains_key(&old), "only Removed frees the driver's place");
        assert_eq!(accounts.erasing.contains_key(&old), removing);
        assert_eq!(accounts.above.is_empty(), !removing, "only sign-out asks the driver to remove its account");
        assert!(engine.values.accounts.value(fresh, 2, Time::from_nanos(1)).is_some());
        let Some(store::Event::Secret { value: Some(_), .. }) = engine.store.secrets(
            Token::new(99),
            store::Secrets::Get { owner: store::SecretOwner::Source, number: u64::from(fresh) },
        ) else {
            panic!("the fresh different source keeps its record");
        };
    }
}

fn record(source: u32, _limits: &oauth::Limits) -> oauth::SavedToken {
    let mut record = jig_protocol_accounts_world::fixtures::kept();
    record.key = source;
    record.generation = 2;
    record
}
fn signed_out(events: &mut Queue<root::Event>, old: u32) -> Option<bool> {
    let event = events.pop()?;
    let root::Event::Core(core::Event::Account(domain::Event::SignedOut { source, confirmed })) = event else {
        panic!("only a sign-out terminal follows erasure");
    };
    assert_eq!(source, old);
    assert!(events.is_empty(), "one terminal despite duplicate acknowledgements");
    Some(confirmed)
}

fn carry_llm_key(engine: &mut crate::Engine, value: &[u8]) -> u64 {
    let _later = engine.store.secrets(
        Token::new(123),
        store::Secrets::Carry { kind: jig_protocol_store::CarriedKind::Key, value: Secret::new(value.into()) },
    );
    // The start's mark may still have a terminal here. Preserve unrelated
    // answers and take only this explicit carry's terminal.
    for _ in 0..engine.store.above.len() {
        let event = engine.store.above.pop().expect("store answer");
        if let store::Event::Carried { owner, number } = &event
            && *owner == Token::new(123)
        {
            return *number;
        }
        engine.store.above.push(event);
    }
    panic!("carried LLM key")
}
fn pump_accounts(accounts: &mut Accounts, engine: &mut crate::Engine) -> List<domain::FromComponent> {
    let mut events = Queue::with_capacity(64);
    let mut heard = List::with_capacity(64);
    for _ in 0_u32..32 {
        crate::store::answer(engine);
        accounts.iterate(Time::from_nanos(1), Wall::from_nanos(1), &mut engine.values, &mut events, &mut engine.store);
        while let Some(event) = events.pop() {
            if let root::Event::Core(core::Event::Account(domain::Event::Component(event))) = event {
                heard.push(event).expect("bounded component events");
            }
        }
    }
    heard
}
#[test]
fn llm_key_work_reaches_the_store_and_the_workers_table_through_the_service() {
    let mut service = fixtures::started(&fixtures::accounts_document());
    fixtures::finish_start(&mut service);
    let engine = fixtures::engine(&mut service);
    let mut accounts = engine.accounts.take().expect("accounts component");
    let carried = carry_llm_key(engine, b"first-llm-key");
    accounts.request(domain::Request::Signing(domain::Signing::Keep { source: 7, carried, replaces: false }));
    assert!(has_llm_key_grant(&pump_accounts(&mut accounts, engine), 1));
    assert_eq!(
        engine.values.accounts.value(7, 1, Time::from_nanos(1)).expect("worker value").token.expose(),
        b"first-llm-key"
    );
    let carried = carry_llm_key(engine, b"next-llm-key");
    accounts.request(domain::Request::Signing(domain::Signing::Keep { source: 7, carried, replaces: true }));
    assert!(has_llm_key_grant(&pump_accounts(&mut accounts, engine), 2));
    accounts.request(domain::Request::Signing(domain::Signing::Forget { source: 7 }));
    pump_accounts(&mut accounts, engine);
    assert!(engine.values.accounts.value(7, 1, Time::from_nanos(1)).is_none());
    assert!(engine.values.accounts.value(7, 2, Time::from_nanos(1)).is_none());
    let Some(store::Event::Secret { value: None, .. }) =
        engine.store.secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Source, number: 7 })
    else {
        panic!("removed LLM key value")
    };
}
#[test]
fn service_restart_restores_an_llm_key_before_want_and_sweeps_only_unnamed_sources() {
    let mut service = fixtures::started(&fixtures::accounts_document());
    fixtures::finish_start(&mut service);
    let engine = fixtures::engine(&mut service);
    let mut accounts = engine.accounts.take().expect("accounts component");
    let carried = carry_llm_key(engine, b"kept-llm-key");
    accounts.request(domain::Request::Signing(domain::Signing::Keep { source: 7, carried, replaces: false }));
    pump_accounts(&mut accounts, engine);
    engine.values.accounts.remove(7);
    let carried_orphan = carry_llm_key(engine, b"orphaned-carried-value");
    let _later = engine.store.secrets(
        Token::new(123),
        store::Secrets::Keep {
            owner: store::SecretOwner::Source,
            number: 8,
            value: jig_protocol_store::SecretValue::Record(Secret::new(b"orphaned-source-value".as_slice().into())),
        },
    );
    while engine.store.above.pop().is_some() {}
    let mut fresh_service = fixtures::started(&fixtures::accounts_document());
    let mut restarted = fixtures::engine(&mut fresh_service).accounts.take().expect("new component");
    restarted.request(domain::Request::Signing(domain::Signing::Restore {
        source: 7,
        provider: 1,
        kind: domain::Kind::Key,
    }));
    restarted.request(domain::Request::Component(domain::ToComponent::Want { account: 7 }));
    restarted.request(domain::Request::Signing(domain::Signing::Restored));
    let heard = pump_accounts(&mut restarted, engine);
    // One restored source and one queued want yield exactly one grant.
    let mut grants = 0_u32;
    for event in &heard {
        if let domain::FromComponent::Granted { account: 7, generation: 1, .. } = event {
            grants = grants.checked_add(1).expect("bounded component events");
        }
    }
    assert_eq!(grants, 1);
    assert_eq!(
        engine.values.accounts.value(7, 1, Time::from_nanos(1)).expect("restored value").token.expose(),
        b"kept-llm-key"
    );
    for (owner, number, exists) in [
        (store::SecretOwner::Source, 1, false),
        (store::SecretOwner::Source, 7, true),
        (store::SecretOwner::Source, 8, false),
        (store::SecretOwner::Application, carried_orphan, true),
    ] {
        let Some(store::Event::Secret { value, .. }) =
            engine.store.secrets(Token::new(123), store::Secrets::Get { owner, number })
        else {
            panic!("stored value")
        };
        assert_eq!(value.is_some(), exists, "{owner:?} {number}");
    }
}

fn has_llm_key_grant(heard: &List<domain::FromComponent>, wanted: u64) -> bool {
    for event in heard {
        if let domain::FromComponent::Granted {
            account: 7,
            generation,
            form: domain::Form::Credential(domain::Scheme::Key),
            ..
        } = event
            && *generation == wanted
        {
            return true;
        }
    }
    false
}

#[test]
fn aborting_does_not_submit_an_llm_key_adoption_to_the_closing_store() {
    let mut service = fixtures::started(&fixtures::accounts_document());
    fixtures::finish_start(&mut service);
    let engine = fixtures::engine(&mut service);
    let mut accounts = engine.accounts.take().expect("accounts component");
    let carried = carry_llm_key(engine, b"aborted-llm-key");
    accounts.start(&mut engine.store, &mut engine.values);
    accounts.request(domain::Request::Signing(domain::Signing::Keep { source: 7, carried, replaces: false }));
    accounts.start(&mut engine.store, &mut engine.values);
    assert!(!accounts.key_out.is_empty(), "adoption has not been submitted yet");
    accounts.abort();
    let heard = pump_accounts(&mut accounts, engine);
    assert!(!has_llm_key_grant(&heard, 1));
    assert!(accounts.is_empty(), "abort settles the component");
    let Some(store::Event::Secret { value: None, .. }) =
        engine.store.secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Source, number: 7 })
    else {
        panic!("unadopted LLM key")
    };
    let Some(store::Event::Secret { value: Some(_), .. }) = engine
        .store
        .secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Application, number: carried })
    else {
        panic!("carried value remains an orphan")
    };
}

fn restarted_signed_in_source() -> (crate::Service, Accounts) {
    use jig_protocol_accounts_world::{fixtures as account_fixtures, world::Story};
    let mut service = fixtures::started(&fixtures::accounts_document());
    fixtures::finish_start(&mut service);
    let engine = fixtures::engine(&mut service);
    let limits = engine.accounts.as_ref().expect("accounts component").env.limits;
    let mut saved = account_fixtures::kept();
    saved.key = 9001;
    saved.issued_client_id = Some(b"issued-client".as_slice().into());
    saved.granted_scopes = Some(b"openid read".as_slice().into());
    saved.subject = Some(b"subject".as_slice().into());
    let value = oauth::encode_record(&saved, &limits.client.document).expect("signed-in record");
    let _later = engine.store.secrets(
        Token::new(123),
        store::Secrets::Keep {
            owner: store::SecretOwner::Source,
            number: 9001,
            value: jig_protocol_store::SecretValue::Record(Secret::new(value)),
        },
    );
    while engine.store.above.pop().is_some() {}
    let mut fresh = fixtures::started(&fixtures::accounts_document());
    let mut accounts = fixtures::engine(&mut fresh).accounts.take().expect("restarted component");
    let template = party::party_account(
        &account_fixtures::party_registration(Story::SignIn),
        driver::Endpoint {
            address: ::core::net::SocketAddr::from(([127, 0, 0, 1], 31000)),
            transport: driver::Transport::Plaintext,
        },
        List::with_capacity(0),
        &fixtures::seeds().installation,
    );
    let Ok(previous) = accounts.providers.insert(1, template) else {
        panic!("configured provider fits");
    };
    assert!(previous.is_none());
    accounts.request(domain::Request::Signing(domain::Signing::Restore {
        source: 9001,
        provider: 1,
        kind: domain::Kind::SignIn,
    }));
    accounts.request(domain::Request::Component(domain::ToComponent::Want { account: 9001 }));
    accounts.request(domain::Request::Signing(domain::Signing::Restored));
    (service, accounts)
}

#[test]
fn a_restart_restores_a_party_signed_in_source_under_its_number_and_lends_a_bearer() {
    let (mut service, mut accounts) = restarted_signed_in_source();
    let engine = fixtures::engine(&mut service);
    let mut granted = None;
    for event in &pump_accounts(&mut accounts, engine) {
        if let domain::FromComponent::Granted { account: 9001, generation: 1, form, .. } = event {
            granted = Some(*form);
        }
    }
    assert_eq!(granted, Some(domain::Form::Credential(domain::Scheme::Bearer)));
    let value = engine.values.accounts.value(9001, 1, Time::from_nanos(1)).expect("restored grant");
    assert_eq!(value.token.expose(), jig_protocol_accounts_world::fixtures::TOKEN_FIRST);
    assert_eq!(value.scheme, table::Scheme::Bearer);
}

#[test]
fn a_restart_keeps_the_named_signed_in_record_and_sweeps_an_unnamed_record() {
    let (mut service, mut accounts) = restarted_signed_in_source();
    let engine = fixtures::engine(&mut service);
    let _later = engine.store.secrets(
        Token::new(123),
        store::Secrets::Keep {
            owner: store::SecretOwner::Source,
            number: 9002,
            value: jig_protocol_store::SecretValue::Record(Secret::new(b"unnamed-control".as_slice().into())),
        },
    );
    while engine.store.above.pop().is_some() {}
    pump_accounts(&mut accounts, engine);
    for (number, exists) in [(9001, true), (9002, false)] {
        let Some(store::Event::Secret { value, .. }) =
            engine.store.secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Source, number })
        else {
            panic!("source secret answer");
        };
        assert_eq!(value.is_some(), exists, "source {number}");
    }
}

#[test]
fn a_restored_sources_wrong_kind_stops_the_engine() {
    for (kind, value) in [
        (domain::Kind::Key, store::SecretValue::Record(Secret::new(Box::from(b"record".as_slice())))),
        (domain::Kind::SignIn, store::SecretValue::Key(Secret::new(Box::from(b"key".as_slice())))),
    ] {
        let mut service = fixtures::started(&fixtures::accounts_document());
        fixtures::finish_start(&mut service);
        let engine = fixtures::engine(&mut service);
        let mut accounts = engine.accounts.take().expect("accounts runtime");
        engine
            .store
            .secrets(Token::new(123), store::Secrets::Keep { owner: store::SecretOwner::Source, number: 7, value });
        accounts.dispatch(
            domain::Request::Signing(domain::Signing::Restore { source: 7, provider: 1, kind }),
            &mut engine.store,
            &mut engine.values,
        );
        crate::store::answer(engine);
        let Some(crate::End::Stopped(_)) = &engine.end else { panic!("wrong restored kind must stop: {kind:?}") };
    }
}

#[test]
fn closing_unstarted_accounts_discards_work_and_reports_closed_once() {
    const TERMINALS: u32 = 2; // room to observe an accidental duplicate Closed
    const SOURCE: u32 = 7;
    const PROVIDER: u32 = 1;
    const CARRIED: u64 = 41;
    let mut service = fixtures::started(&fixtures::accounts_document());
    let engine = fixtures::engine(&mut service);
    let mut accounts = engine.accounts.take().expect("unstarted accounts");
    let mut events = Queue::with_capacity(TERMINALS);
    accounts.request(domain::Request::Signing(domain::Signing::Restore {
        source: SOURCE,
        provider: PROVIDER,
        kind: domain::Kind::Key,
    }));
    accounts.request(domain::Request::Signing(domain::Signing::Keep {
        source: SOURCE,
        carried: CARRIED,
        replaces: false,
    }));
    accounts.request(domain::Request::Signing(domain::Signing::Restored));
    accounts.request(domain::Request::Component(domain::ToComponent::Want { account: SOURCE }));
    accounts.request(domain::Request::Component(domain::ToComponent::Close));
    accounts.request(domain::Request::Component(domain::ToComponent::Close));
    accounts.iterate(Time::ZERO, Wall::EPOCH, &mut engine.values, &mut events, &mut engine.store);
    let Some(root::Event::Core(core::Event::Account(domain::Event::Component(domain::FromComponent::Closed)))) =
        events.pop()
    else {
        panic!("an unstarted component closes without making its driver")
    };
    assert!(events.is_empty(), "one close terminal, no failed or granted work");
    assert!(accounts.component.is_none());
    assert!(accounts.is_empty());
    assert!(accounts.io.is_empty());
    assert!(!engine.store.component.mark_durable());
}
