use super::fixtures as f;
use crate::translate::settings as translate;
use jig_client_protocol_wire::v19 as w;
use jig_domain_core::{documents as d, reads, settings as s};
use skein_lib::{List, Reader, Wall};

fn head() -> s::Head {
    s::Head { number: 6, against: 5, at: Wall::EPOCH, by: s::Decider::Release, proposal: None, moved: false }
}
fn settings() -> d::Settings {
    let mut held = List::with_capacity(1);
    held.push(d::HeldRevision { head: head(), holders: 2 }).unwrap();
    let mut settings = List::with_capacity(1);
    settings
        .push(d::SettingLine {
            setting: s::Setting::Template(1),
            name: Some(d::Text { bytes: Box::from(b"chat".as_slice()), cut: false }),
            follows: d::Follows::Default,
            last: 6,
        })
        .unwrap();
    d::Settings { newest: 6, settings, held, pending: List::with_capacity(0) }
}
fn returned(answer: reads::Answer, request: w::Read) -> w::ReadValue {
    let mut exchange = f::read_exchange();
    let (owner, reply, _) = exchange.read(&request);
    exchange.release_read(reply, answer);
    let w::ReadAnswer::Value(value) = exchange.read_answer(owner) else { panic!("value answer") };
    let value = value.into_parts();
    w::ReadValue::decode(&f::wire(), &mut Reader::new(&value.record)).unwrap()
}
#[test]
fn settings_reads_enter_the_core_in_its_vocabulary() {
    let wire = f::wire();
    for (request, expected) in [
        (w::Read::Settings, reads::Read::Settings),
        (
            w::Read::Setting(
                w::ReadSetting::new(&wire, w::ReadSettingParts { setting: w::Setting::Rules, revision: 6 }).unwrap(),
            ),
            reads::Read::Setting { setting: s::Setting::Rules, revision: 6 },
        ),
        (
            w::Read::Revision(w::ReadRevision::new(&wire, w::ReadRevisionParts { number: 5 }).unwrap()),
            reads::Read::Revision { number: 5 },
        ),
        (
            w::Read::Revisions(w::ReadRevisions::new(&wire, w::ReadRevisionsParts { before: Some(5) }).unwrap()),
            reads::Read::Revisions { before: Some(5) },
        ),
    ] {
        let mut exchange = f::read_exchange();
        let (_, _, actual) = exchange.read(&request);
        assert_eq!(actual, expected);
    }
}
#[test]
fn inventory_reads_and_watch_deliveries_carry_the_same_facts() {
    let document = settings();
    let expected = translate::document(&document, &f::wire()).unwrap();
    assert_eq!(
        returned(
            reads::Answer::Document(Box::new(d::Document::Settings(Box::new(document.clone())))),
            w::Read::Settings
        ),
        w::ReadValue::Settings(expected.clone())
    );
    assert_eq!(
        crate::translate::watches::delivery(
            &d::Document::Settings(Box::new(document)),
            0,
            32,
            &f::models(),
            &f::wire()
        )
        .unwrap(),
        w::DeliveryValue::Settings(expected)
    );
}
#[test]
fn settings_watch_subject_enters_the_core() {
    let event = crate::translate::watches::watch(skein_lib::Token::new(1), 2, w::Subject::Settings, 3).unwrap();
    let jig_domain_core::Event::Watch { subject, .. } = event else { panic!("watch") };
    assert_eq!(subject, d::Watched::Settings);
}
#[test]
fn forgotten_and_unknown_revision_answers_keep_their_distinction() {
    for (answer, expected) in [
        (reads::RevisionAnswer::Unknown, w::RevisionAnswer::Unknown),
        (
            reads::RevisionAnswer::Forgotten { oldest: 4 },
            w::RevisionAnswer::Forgotten(
                w::RevisionForgotten::new(&f::wire(), w::RevisionForgottenParts { oldest: 4 }).unwrap(),
            ),
        ),
    ] {
        assert_eq!(translate::revision(&answer, &f::models(), &f::wire()).unwrap(), expected);
    }
}
#[test]
fn the_inventory_at_profile_bounds_fits_one_wire_document() {
    // The standard profile has 8 charters, 16 templates and 8 policies plus rules.
    // Each map can name a whole set. Names belong to templates, up to 8192 bytes.
    const INVENTORY: u32 = 66;
    const NAMED: u32 = 32;
    const NAME_BYTES: usize = 8192;
    let mut document = settings();
    document.settings = List::with_capacity(INVENTORY);
    for number in 0..INVENTORY {
        document
            .settings
            .push(d::SettingLine {
                setting: s::Setting::Template(number),
                name: if number < NAMED {
                    Some(d::Text { bytes: Box::from([b'x'; NAME_BYTES].as_slice()), cut: false })
                } else {
                    None
                },
                follows: d::Follows::Default,
                last: 6,
            })
            .unwrap();
    }
    let value = translate::document(&document, &w::CEILINGS).unwrap();
    assert!(value.measure() < w::CEILINGS.owned_record);
    assert!(document.settings.len() > w::CEILINGS.revisions_page_rows);
    let mut client = f::client_limits().client;
    client.document_bytes = u64::from(w::CEILINGS.owned_record);
    client.text = u32::try_from(NAME_BYTES).unwrap();
    client.turn_text = client.text;
    client.turn_bytes = client.text;
    let limits = jig_client_protocol_link::Limits::new(client, 4).unwrap();
    let bytes = encoded_value(&w::ReadValue::Settings(value));
    let _value = jig_client_protocol_link::translate::reads::value(&bytes, &limits).unwrap();
}

fn authority() -> jig_domain_authority::Authority {
    use jig_domain_authority as a;
    a::Authority {
        tools: a::Tools(1),
        models: a::Models::ALL,
        grants: Box::new([]),
        delegation: a::Delegation { kinds: Box::new([a::Executor::Charter(1)]), tasks: 2, depth: 1 },
        budget: a::Budget { spend: 100, deadline: None },
        notes: a::Scopes(0),
        note_resources: Box::new([]),
    }
}
fn charter() -> jig_domain_core::Charter {
    use jig_domain_core as core;
    use skein_lib::Duration;
    core::Charter {
        number: 1,
        hosts: jig_domain_fleet::Kinds::Both,
        spend: 100,
        policy: core::RunPolicy {
            instructions: Box::from("instructions λ".as_bytes()),
            waiting: Duration::from_secs(2),
            resume: true,
            turns: 4,
            time: Duration::from_secs(30),
            reserve_turns: 1,
            reserve_time: Duration::from_secs(5),
            model: core::Model {
                dialect: 3,
                endpoint: 4,
                program: Some(Box::from(b"program".as_slice())),
                name: Box::from(b"careful".as_slice()),
                window: 8192,
                output: 1024,
                input_price: 1,
                cached_price: 2,
                cache_write_price: Some(3),
                output_price: 4,
                price_unit: 1000,
                efforts: Box::new([Box::from(b"high".as_slice())]),
                effort: Some(Box::from(b"high".as_slice())),
            },
            alternatives: Box::new([]),
            inspect: true,
            modify: false,
            shell: true,
            agents: false,
            call_timeout: Duration::from_secs(1),
        },
    }
}
fn values() -> [s::Value; 4] {
    use jig_domain_authority as a;
    [
        s::Value::Charter(Box::new(charter())),
        s::Value::Template(Box::new(f::template())),
        s::Value::Policy(Box::new(jig_domain_core::ConfiguredPolicy {
            policy: a::Policy {
                escalation_role: Some(1),
                ceiling: authority(),
                projections: Box::new([]),
                period_spend: 100,
                roles: Box::new([a::Role {
                    number: 1,
                    authority: authority(),
                    period_spend: 100,
                    requests: a::Requests::ALL,
                    decides: a::Proposals::ALL,
                }]),
                requirements: Box::new([]),
            },
            permissions: Box::new([jig_domain_people::PermissionRole { connector: 1, permission: 2, role: 1 }]),
            templates: Box::new([]),
        })),
        s::Value::Rules(Box::new(a::Rules {
            ceiling: authority(),
            period_spend: 100,
            minimum_run_spend: 1,
            maximum_run_spend: 100,
            implies: a::Implies::new(Box::new([]), 0).unwrap(),
            requirements: Box::new([]),
        })),
    ]
}
fn encoded_value(value: &w::ReadValue) -> Box<[u8]> {
    let mut out = skein_lib::Writer::new(usize::try_from(value.measure()).unwrap());
    value.encode(&mut out).unwrap();
    out.finish()
}
#[test]
fn whole_setting_kinds_reach_the_clients_as_their_values() {
    use jig_client_domain::contract as c;
    let names = [s::Setting::Charter(1), s::Setting::Template(1), s::Setting::Policy(1), s::Setting::Rules];
    for (name, value) in names.into_iter().zip(values()) {
        let document = reads::SettingDocument { setting: name, revision: 6, value };
        let wire = translate::setting_document(&document, &f::models(), &w::CEILINGS).unwrap();
        let restored =
            jig_client_protocol_link::translate::settings::restore_setting_document(&wire, &f::client_limits())
                .unwrap();
        assert_eq!(restored.revision, 6);
        match (&document.value, restored.value) {
            (s::Value::Charter(source), c::SettingValue::Charter(value)) => {
                assert_eq!(value.instructions.bytes(), source.policy.instructions.as_ref());
                assert_eq!(value.model.name.bytes(), source.policy.model.name.as_ref());
                assert_eq!(value.hosts, c::SettingHosts::Both);
            }
            (s::Value::Template(source), c::SettingValue::Template(value)) => {
                assert_eq!(value.name.bytes.bytes(), source.name.as_ref());
            }
            (s::Value::Policy(source), c::SettingValue::Policy(value)) => {
                assert_eq!(value.roles.len(), u32::try_from(source.policy.roles.len()).unwrap());
                assert_eq!(value.permissions.get(0).unwrap().permission, 2);
            }
            (s::Value::Rules(source), c::SettingValue::Rules(value)) => {
                assert_eq!(value.maximum_run_spend, source.maximum_run_spend);
            }
            _ => panic!("setting kind survives translation"),
        }
    }
}
#[test]
fn revision_values_past_the_single_read_budget_name_their_encoded_size() {
    use jig_client_domain::contract as c;
    let source = s::Value::Charter(Box::new(charter()));
    let bytes = u64::from(translate::value(&source, &f::models(), &w::CEILINGS).unwrap().measure());
    let revision = s::Revision {
        head: head(),
        items: Box::new([
            s::Item { setting: s::Setting::Charter(1), edit: s::Edit::Set(source) },
            s::Item { setting: s::Setting::Template(2), edit: s::Edit::Removed },
            s::Item { setting: s::Setting::Policy(3), edit: s::Edit::Reset },
        ]),
    };
    for (budget, large) in [(bytes, false), (bytes - 1, true)] {
        let answer = reads::RevisionAnswer::Revision { revision: revision.clone(), value_bytes: budget };
        let wire = translate::revision(&answer, &f::models(), &w::CEILINGS).unwrap();
        let restored =
            jig_client_protocol_link::translate::settings::restore_revision_answer(&wire, &f::client_limits()).unwrap();
        let c::RevisionAnswer::Revision(document) = restored else { panic!("revision") };
        assert_eq!(document.head.number, 6);
        assert_eq!(document.items.len(), 3);
        assert_eq!(document.items.get(1).unwrap().edit, c::RevisionEdit::Removed);
        assert_eq!(document.items.get(2).unwrap().edit, c::RevisionEdit::Reset);
        match &document.items.get(0).unwrap().edit {
            c::RevisionEdit::TooLarge(value) => {
                assert!(large);
                assert_eq!(value.bytes, bytes);
            }
            c::RevisionEdit::Set(_) => assert!(!large),
            c::RevisionEdit::Removed | c::RevisionEdit::Reset => panic!("set or size"),
        }
    }
}
#[test]
fn revision_page_heads_and_cursors_reach_the_client_without_values() {
    use jig_client_domain::contract as c;
    let mut items = List::with_capacity(1);
    items.push(reads::RevisionItemHead { setting: s::Setting::Rules, edit: reads::RevisionEdit::Set }).unwrap();
    let mut rows = List::with_capacity(1);
    rows.push(reads::RevisionRow { head: head(), items, more: 3 }).unwrap();
    let page = reads::RevisionsPage { rows, next: Some(6), forgotten: false };
    let wire = translate::revisions(&page, &w::CEILINGS).unwrap();
    let bytes = encoded_value(&w::ReadValue::Revisions(wire));
    let client = jig_client_protocol_link::translate::reads::value(&bytes, &f::client_limits()).unwrap();
    let c::ReadValue::Revisions(page) = client else { panic!("revision page") };
    assert_eq!(page.next, Some(6));
    assert_eq!(page.rows.get(0).unwrap().more, 3);
    assert_eq!(page.rows.get(0).unwrap().items.get(0).unwrap().edit, c::RevisionItemEdit::Set);
}

#[test]
fn review_values_hold_at_the_document_bound_and_give_the_size_one_past_it() {
    let mut limits = f::wire();
    limits.owned_record = w::CEILINGS.owned_record;
    for value in values() {
        let setting = match &value {
            s::Value::Charter(value) => s::Setting::Charter(value.number),
            s::Value::Template(value) => s::Setting::Template(value.number),
            s::Value::Policy(_) => s::Setting::Policy(1),
            s::Value::Rules(_) => s::Setting::Rules,
        };
        let bytes = u64::from(translate::value(&value, &f::models(), &limits).expect("whole value").measure());
        for (bound, too_large) in [(bytes, false), (bytes - 1, true)] {
            let mut items = List::with_capacity(1);
            items
                .push(d::ReviewItem {
                    setting,
                    edit: reads::RevisionEdit::Set,
                    now: Some(value.clone()),
                    origin: Some((false, 1)),
                    would_be: Some(value.clone()),
                })
                .expect("review item");
            let review = d::SettingsReview { newest: 1, items, value_bytes: bound };
            let wire = translate::review(&review, &f::models(), &limits).expect("review");
            let mut input = Reader::new(wire.items());
            let piece = skein_codec::piece::split(&mut input).expect("frame").expect("review item");
            let row = w::ReviewItem::decode(&limits, &mut Reader::new(&piece)).expect("typed item");
            for shown in [row.now(), row.would_be()] {
                if too_large {
                    let w::ReviewValue::TooLarge(size) = shown else { panic!("size instead of a value") };
                    assert_eq!(size.bytes(), bytes);
                } else {
                    let w::ReviewValue::Value(_) = shown else { panic!("whole review value within its bound") };
                }
            }
        }
    }
}
