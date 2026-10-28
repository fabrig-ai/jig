#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::wildcard_enum_match_arm,
    reason = "ordinary tests collect text, query trees and select scripted requests"
)]

use crate::phrases::english;
use crate::phrases::jig::{JigPhrase, shapes};
use crate::tests::fixtures::{Fixture, Watched};
use crate::words::Words;
use crate::{
    Event, Request,
    action::Act as Action,
    address::Address,
    intents::Progress,
    objects::{Card, Left, Source},
};
use crate::{JigBindings, JigMeaning, Owner, decode};
use jig_client::contract::*;
use skein_lib::{Duration, List, Time, Token, Wall};
use skein_ui::Act;
use skein_ui::phrases::{Hole, PhraseLimits, Phrasebook};
use skein_ui::{Builder, Role, SlotRef, Tree};

pub(crate) const CLIENT: crate::Limits = crate::Limits {
    watches: 8,
    notices: 2,
    notice_life: Duration::from_secs(5),
    linger: Duration::from_secs(5),
    save_delay: Duration::from_secs(1),
    intents: 2,
    fields: 8,

    objects: 3,
    sources: 8,
    reads: 8,
    parts: 8,
    delegates: 2,
    document_bytes: 65536,
    records: 2,
    kept: 4,
    drafts: 1,
    projects: 2,
    roles: 2,
    text: 128,
    rows: 2,
    segments: 4,
    accounts: jig_client::AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
    conversation_words: 2,
    navigation_entries: 8,
    updates_count: 99,
};
pub(super) fn limits() -> skein_ui::Limits {
    skein_ui::Limits {
        nodes: 256,
        depth: 10,
        text_per_node: 1024,
        text_bytes: 16384,
        drawing_bytes: 0,
        styles_per_node: 2,
        patches: 1536,
        bindings: 64,
        slots: 8,
        markdown_depth: 3,
        scene_marks: 0,
        scene_bytes: 0,
    }
}
pub(crate) fn shared_words() -> &'static Words {
    static WORDS: std::sync::OnceLock<Words> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| Words::new(english::english(CLIENT.text).unwrap(), 0, 60))
}
pub(super) fn words() -> &'static Words {
    shared_words()
}
pub(super) fn text(bytes: &[u8]) -> Text {
    Text::new(bytes.into(), CLIENT.text).unwrap()
}
pub(super) fn drive(d: &mut Fixture, event: impl crate::tests::fixtures::Input) -> Vec<Request> {
    d.drive(Time::ZERO, event).into_vec()
}
pub(super) fn document() -> Me {
    let mut projects = List::with_capacity(2);
    projects
        .push(MeProject {
            models: List::with_capacity(0),
            templates: crate::tests::fixtures::chat_templates(),
            creating: Some(crate::tests::fixtures::creating()),
            project: 4,
            name: DocumentText::complete(text(b"One")),
            role: HeldRole::Policy { role: 1 },
            pool: Some(PoolLedger {
                source: Funder::Pool { project: 4, person: 1, period: 1 },
                parent: None,
                numbers: OwnerNumbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 },
                left: 0,
                closed: false,
                began: None,
                next: None,
            }),
        })
        .unwrap();
    Me {
        providers: List::with_capacity(0),
        providers_cut: false,
        key_bytes: 256,
        redirect_bytes: 4096,
        words_bytes: 65_536,
        amendment_bytes: 65_536,
        party: MeParty {
            number: 1,
            kind: PartyKind::Person,
            login: DocumentText::complete(text(b"fixture-login")),
            name: DocumentText::complete(text(b"Pat")),
        },
        projects,
        needs: Needs { questions: 1, proposals: 2, escalations: 3, person_tasks: 4 },
        needs_total: 10,
        results_seen: 0,
        cut: false,
        updates: Count { value: 120, limited: false },

        retention: Duration::from_secs(1000),
        deployment_role: Some(HeldRole::Policy { role: 1 }),
    }
}
pub(super) fn start() -> (Fixture, jig_client::Handle) {
    start_arranged(crate::page::Arrangement::none())
}
pub(super) fn start_arranged(arrangement: crate::page::Arrangement) -> (Fixture, jig_client::Handle) {
    let mut d = Fixture::new(&CLIENT);
    d.configure_arrangement(arrangement).unwrap();
    let out = drive(
        &mut d,
        Event::Start {
            seed: 8,
            place: crate::address::Place::Jig(Address::Inbox { project: None }),
            records: List::with_capacity(0),
        },
    );
    let token = out
        .iter()
        .find_map(|r| match r {
            Request::Client { asked: jig_client::Event::Watch { handle: watch, .. } } => Some(*watch),
            _ => None,
        })
        .unwrap();
    (d, token)
}
fn flat(tree: &Tree) -> Vec<u8> {
    tree.nodes().iter().flat_map(|n| n.text.as_slice().iter().flat_map(|r| r.text.iter().copied())).collect()
}
pub(crate) fn contains(tree: &Tree, text: &[u8]) -> bool {
    flat(tree).windows(text.len()).any(|w| w == text)
}

pub(crate) fn tree(
    fixture: &Fixture,
    section: Option<crate::page::Section>,
    card: Option<skein_lib::Id<crate::objects::Object>>,
) -> Tree {
    let bounds = crate::limits::of_words(&fixture.ui.limits, words()).unwrap();
    let mut builder = Builder::new(&bounds);
    let mut bindings = JigBindings::new(bounds.bindings, fixture.ui.limits.text);
    builder.open(Role::Page);
    if let Some(card) = card {
        crate::cards::card(
            &mut builder,
            &fixture.view(),
            card,
            &crate::cards::Place::Elsewhere,
            words(),
            &mut bindings,
        );
    } else {
        match section {
            Some(section) => {
                builder.open(Role::Main);
                crate::sections::section(&mut builder, &fixture.view(), words(), &mut bindings, section);
                builder.close();
            }
            None => {
                crate::frame::banner(&mut builder, &fixture.view(), words(), &mut bindings);
                crate::pages::main(&mut builder, &fixture.view(), words(), &mut bindings);
                crate::notices::notices(&mut builder, &fixture.view(), words(), &mut bindings);
            }
        }
    }
    builder.close();
    builder.finish()
}

fn frame_builds_statuses_counts_navigation_and_number_submission_in_every_link_state() {
    let (mut d, watch) = start();
    let words = words();
    let mut bindings = JigBindings::new(64, CLIENT.text);
    for expected in [b"Connecting".as_slice(), b"Live", b"Offline"] {
        if expected == b"Live" {
            drive(&mut d, Watched { watch, event: WatchEvent::Delivered(Delivery::Me(Box::new(document()))) });
        }
        if expected == b"Offline" {
            d.set_link(jig_client::Link::Offline);
        }
        bindings.begin();
        let mut b = Builder::new(&limits());
        b.open(Role::Page);
        crate::frame::banner(&mut b, &d.view(), words, &mut bindings);
        crate::frame::navigation(
            &mut b,
            &d.view(),
            SlotRef { owner: (Owner::Application { number: 1 }).opaque(), token: Token::new(1) },
            words,
            &mut bindings,
        );
        b.close();
        let tree = b.finish();
        super::cards_access::check(&tree);
        assert!(tree.nodes().iter().any(|n| n.role == Role::Status && n.name.as_deref() == Some(expected)));
        if expected != b"Connecting" {
            assert!(contains(&tree, b"10 need you"));
            assert!(contains(&tree, b"More than 99 updates not read"));
            assert!(contains(&tree, b"Signed in as Pat"));
        }
        let field = tree.nodes().iter().find(|n| n.role == Role::TextField).unwrap().binding.unwrap();
        assert_eq!(decode(&mut bindings, field.token, Act::Input { value: b"123".as_slice().into() }), None);
        assert_eq!(decode(&mut bindings, field.token, Act::Submit), Some(Action::Go { address: Address::task(123) }));
        assert_eq!(decode(&mut bindings, field.token, Act::Input { value: b"x".as_slice().into() }), None);
        assert_eq!(decode(&mut bindings, field.token, Act::Submit), None);
    }
}

fn shared_card_frame_says_each_progress_refusal_and_departure_with_context_only_by_place() {
    let (mut d, frame_watch) = start();
    let source = Source::Window { number: 1 };
    // The public object collection is read-only in Fixture; build states through its shared entry point.
    let mut objects = crate::objects::Objects::new(2, 8);
    let id = objects.name(ObjectKey::Task(7), source).unwrap();
    let words = words();
    let mut bindings = JigBindings::new(64, 128);
    let states = [
        Card::Open,
        Card::Deciding { request: None, progress: Progress::Confirming },
        Card::Deciding { request: Some(Token::new(1)), progress: Progress::InFlight },
        Card::Deciding { request: Some(Token::new(1)), progress: Progress::Retrying },
        Card::Deciding { request: Some(Token::new(1)), progress: Progress::Parked },
        Card::Deciding { request: Some(Token::new(1)), progress: Progress::Unknown },
        Card::Refused {
            refusal: Refusal::Authority { lacking: Lacking::Budget { needed: 40, left: 12 }, proposable: None },
        },
        Card::Unknown { act: ActKind::Cancel, look: Read::Task { task: 7 } },
        Card::Leaving {
            how: Left::Answered {
                outcome: Outcome::DecidedFirst {
                    by: Party { number: 2, kind: PartyKind::Person, name: text(b"Lee") },
                    how: Decision::Rejected { reason: text(b"already decided") },
                    at: Wall::EPOCH,
                },
            },
            until: Time::ZERO,
        },
        Card::Leaving {
            how: Left::Ended {
                how: Ending::Cancelled { by: Actor::Party(3), reason: Some(text(b"no longer needed")), at: None },
            },
            until: Time::ZERO,
        },
        Card::Leaving { how: Left::Looking, until: Time::ZERO },
        Card::Leaving { how: Left::Unknown, until: Time::ZERO },
    ];
    for state in states {
        let mut b = Builder::new(&limits());
        b.open(Role::Page);
        crate::cards::state(&mut b, &state, id, words, &mut bindings);
        b.close();
        let tree = b.finish();
        super::cards_access::check(&tree);
        assert!(tree.nodes().iter().any(|n| n.role == Role::Status));
        if let Card::Refused { .. } = state {
            assert!(contains(&tree, b"Your pool has 12 left; this needs 40"));
        }
        if let Card::Unknown { .. } = state {
            assert!(
                tree.nodes()
                    .iter()
                    .any(|n| n.role == Role::Button && n.name.as_deref() == Some(b"Inspect current object".as_slice()))
            );
        }
    }
    card_context(&mut d, frame_watch, words, &mut bindings);
    // Refused by the domain after its handle has left; a retained tree token still decodes the handle.
    let binding = bindings.insert(JigMeaning::Action(Action::LookAgain { object: id.token() }));
    let action = decode(&mut bindings, binding.token, Act::Press).unwrap();
    drive(&mut d, Event::Act { action });
}

fn card_context(d: &mut Fixture, frame_watch: jig_client::Handle, words: &Words, bindings: &mut JigBindings) {
    let out =
        drive(d, Watched { watch: frame_watch, event: WatchEvent::Delivered(Delivery::Me(Box::new(document()))) });
    let inbox_watch =
        out.iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Inbox { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
    let mut entries = List::with_capacity(1);
    entries
        .push(InboxEntry {
            reply: None,
            lineage: List::with_capacity(0),
            goal: None,
            recipient: None,
            stall: None,
            object: ObjectKey::Question { task: 7, message: 1 },
            project: 4,
            group: InboxGroup::Questions,
            position: 1,
            at: Wall::EPOCH,
        })
        .unwrap();
    drive(
        d,
        Watched {
            watch: inbox_watch,
            event: WatchEvent::Delivered(Delivery::Inbox(Box::new(Inbox {
                entries,
                needs: document().needs,
                updates: Count { value: 0, limited: false },
                results_seen: 0,
                replies_seen: List::with_capacity(0),
                more: false,
            }))),
        },
    );
    let shared = d.objects().find(ObjectKey::Question { task: 7, message: 1 }).unwrap();
    let mut lineage = List::with_capacity(1);
    lineage
        .push(Chip {
            task: 6,
            title: DocumentText::complete(text(b"Ancestor")),
            executor: Executor::Party { party: 1 },
            phase: Phase::Done,
            details: Box::new(ChipPhase::Ended { how: Ending::Done, at: Wall::EPOCH }),
        })
        .unwrap();
    for place in [
        crate::cards::Place::Inbox { lineage },
        crate::cards::Place::Conversation,
        crate::cards::Place::TaskPage,
        crate::cards::Place::Elsewhere,
    ] {
        let mut b = Builder::new(&limits());
        b.open(Role::Page);
        assert!(crate::cards::open(&mut b, &d.view(), shared, &place, b"Question", words, bindings));
        crate::cards::close(&mut b);
        b.close();
        let tree = b.finish();
        super::cards_access::check(&tree);
        assert!(
            tree.nodes().iter().any(|n| n.role == Role::Region && n.name.as_deref() == Some(b"Question".as_slice()))
        );
        assert!(tree.nodes().iter().any(|n| n.role == Role::Item && n.key.is_some()));
        assert_eq!(
            contains(&tree, b"In project 4"),
            match place {
                crate::cards::Place::Inbox { .. } | crate::cards::Place::Elsewhere => true,
                crate::cards::Place::Conversation | crate::cards::Place::TaskPage => false,
            }
        );
        assert_eq!(
            contains(&tree, b"Task 6"),
            match place {
                crate::cards::Place::Inbox { .. } => true,
                crate::cards::Place::Conversation | crate::cards::Place::TaskPage | crate::cards::Place::Elsewhere =>
                    false,
            }
        );
    }
}

fn every_binding_kind_decodes_and_old_tokens_are_retired() {
    let mut bindings = JigBindings::new(64, 4);
    let object = Token::new(7);
    let actions = [
        Action::Go { address: Address::task(1) },
        Action::Submit { field: crate::fields::Purpose::Answer },
        Action::Intend { object, act: ActKind::Cancel },
        Action::Choose { option: 2 },
        Action::Confirm,
        Action::Dismiss,
        Action::Fold { object, open: true },
        Action::Filter { group: InboxGroup::Questions, project: None },
        Action::More { window: object, end: jig_client::thread::window::End::Before },
    ];
    for action in actions {
        let token = bindings.insert(JigMeaning::Action(action.clone())).token;
        assert_eq!(decode(&mut bindings, token, Act::Press), Some(action));
        assert_eq!(decode(&mut bindings, token, Act::Input { value: Box::new([]) }), None);
    }
    let field = crate::fields::Purpose::Answer;
    let token = bindings.insert(JigMeaning::Field(field)).token;
    assert_eq!(
        decode(&mut bindings, token, Act::Input { value: b"yes".as_slice().into() }),
        Some(Action::Edit { field, text: text(b"yes") })
    );
    assert_eq!(decode(&mut bindings, token, Act::Submit), Some(Action::Submit { field }));
    assert_eq!(decode(&mut bindings, token, Act::Input { value: b"large".as_slice().into() }), None);
    bindings.begin();
    assert_eq!(decode(&mut bindings, token, Act::Submit), None);
    assert_ne!(bindings.insert(JigMeaning::Field(field)).token, token);
}

fn jig_phrasebook_requires_every_sentence_and_says_wall_times_in_person_offset() {
    let mut language = english::language();
    let missing = language.phrases.last().unwrap().id;
    let size = language.phrases.len().checked_sub(1).unwrap();
    language.phrases = language.phrases.into_vec().into_iter().take(size).collect::<Vec<_>>().into_boxed_slice();
    assert_eq!(
        Phrasebook::load(
            shapes(128),
            Box::new([language]),
            &PhraseLimits {
                languages: 1,
                phrases: crate::owner_words::phrase_count(),
                holes: 4,
                segments: 1000,
                text_bytes: 10000,
                hole_bytes: 128,
            }
        )
        .unwrap_err(),
        skein_ui::phrases::Invalid::Missing { id: missing }
    );
    let words = words();
    let mut b = Builder::new(&limits());
    b.open(Role::Page);
    words.wall(&mut b, Wall::EPOCH);
    words.waiting(&mut b, Wall::from_nanos(240_000_000_000), Wall::EPOCH);
    words.amount(&mut b, u64::MAX);
    b.close();
    let tree = b.finish();
    super::cards_access::check(&tree);
    assert!(contains(&tree, b"At 01:00"));
    assert!(contains(&tree, b"Waiting 4 min"));
    assert!(contains(&tree, b"18446744073709551615"));
    assert_eq!(words.say(JigPhrase::Live, &[]).as_ref(), b"Live");
    assert_eq!(words.say(JigPhrase::Amount, &[Hole::Amount(12)]).as_ref(), b"12");
}

#[test]
fn jig_card_frames_bindings_and_phrase_contracts() {
    frame_builds_statuses_counts_navigation_and_number_submission_in_every_link_state();
    shared_card_frame_says_each_progress_refusal_and_departure_with_context_only_by_place();
    every_binding_kind_decodes_and_old_tokens_are_retired();
    jig_phrasebook_requires_every_sentence_and_says_wall_times_in_person_offset();
}

pub(super) fn party() -> Party {
    Party { number: 1, kind: PartyKind::Person, name: text(b"Pat") }
}
