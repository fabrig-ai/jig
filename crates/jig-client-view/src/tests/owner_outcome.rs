use crate::{owner_outcome as view, owner_outcome_words as phrases, words::Words};
use skein_ui::Limits;
use skein_ui::phrases::{Hole, HoleKind, Invalid, Language, PhraseLimits, Phrasebook, Segment};
use skein_ui::{Builder, Role, Tree};

use jig_client::contract::{
    OwnerEscalationChoice, OwnerOutcome, OwnerProposalChoice, ProposalKey, Proposer, Revision, SeenThrough,
};
use skein_lib::{Wall, bytes::copy_of};

fn phrase_limits() -> PhraseLimits {
    PhraseLimits {
        languages: 2,
        phrases: u32::try_from(phrases::ALL.len()).unwrap(),
        holes: 1,
        segments: 200,
        text_bytes: 8192,
        hole_bytes: 0,
    }
}
fn words() -> Words {
    Words::new(Phrasebook::load(phrases::shapes(), Box::new([phrases::english()]), &phrase_limits()).unwrap(), 0, 0)
}
fn render(value: &OwnerOutcome, words: &Words) -> Tree {
    let required = view::required(words).expect("closed fragment fits");
    let limits = Limits {
        nodes: required.nodes,
        depth: required.depth,
        text_per_node: required.text_per_node,
        text_bytes: required.text_bytes,
        drawing_bytes: 0,
        styles_per_node: 0,
        patches: 0,
        bindings: 0,
        slots: 0,
        markdown_depth: 0,
        scene_marks: 0,
        scene_bytes: 0,
    };
    let mut b = Builder::new(&limits);
    view::render(&mut b, value, words);
    let tree = b.finish();
    assert!(tree.len() <= required.nodes);
    let mut bytes = 0_usize;
    for node in tree.nodes() {
        if let Some(name) = &node.name {
            bytes = bytes.checked_add(name.len()).unwrap();
        }
        for run in &node.text {
            bytes = bytes.checked_add(run.text.len()).unwrap();
        }
    }
    assert!(bytes <= usize::try_from(required.text_bytes).unwrap());
    tree
}
fn has(tree: &Tree, wanted: &[u8]) -> bool {
    for node in tree.nodes() {
        for run in &node.text {
            if run.text.as_ref() == wanted {
                return true;
            }
        }
    }
    false
}
fn expect_lines(tree: &Tree, lines: &[&[u8]]) {
    for line in lines {
        assert!(has(tree, line), "supplied fact is present");
    }
    let mut text_nodes = 0_usize;
    for node in tree.nodes() {
        if node.role == Role::Text {
            text_nodes = text_nodes.checked_add(1).unwrap();
        }
    }
    assert_eq!(text_nodes, lines.len());
}
fn max_cases() -> [(OwnerOutcome, &'static [&'static [u8]]); 23] {
    [
        created_case(),
        (OwnerOutcome::Withdrawn { proposal: u64::MAX }, &[b"Withdrawn", b"Proposal number: 18446744073709551615"]),
        (
            OwnerOutcome::MarkedSeen { seen: SeenThrough::Results { through: u64::MAX } },
            &[b"Read position marked seen", b"Results seen through: 18446744073709551615"],
        ),
        (
            OwnerOutcome::MarkedSeen { seen: SeenThrough::Conversation { task: u64::MAX, through: u64::MAX } },
            &[
                b"Read position marked seen",
                b"Chat: 18446744073709551615",
                b"Replies seen through: 18446744073709551615",
            ],
        ),
        (OwnerOutcome::NoteEdited { name: u64::MAX }, &[b"Note edited", b"Note: 18446744073709551615"]),
        (OwnerOutcome::ServiceMade { person: u64::MAX }, &[b"Service made", b"Person: 18446744073709551615"]),
        (OwnerOutcome::Adopted { project: u32::MAX }, &[b"Project adopted", b"Project: 4294967295"]),
        (OwnerOutcome::Stopped { task: u64::MAX }, &[b"Task stopped", b"Task: 18446744073709551615"]),
        (OwnerOutcome::Cancelled { task: u64::MAX }, &[b"Task cancelled", b"Task: 18446744073709551615"]),
        (OwnerOutcome::Released { task: u64::MAX }, &[b"Task released", b"Task: 18446744073709551615"]),
        (OwnerOutcome::PersonTaken { task: u64::MAX }, &[b"Person task taken", b"Task: 18446744073709551615"]),
        (
            OwnerOutcome::PersonHandedBack { task: u64::MAX },
            &[b"Person task handed back", b"Task: 18446744073709551615"],
        ),
        (OwnerOutcome::PersonAnswered { task: u64::MAX }, &[b"Person task answered", b"Task: 18446744073709551615"]),
        (OwnerOutcome::Moved { task: u64::MAX }, &[b"Task moved", b"Task: 18446744073709551615"]),
        (
            OwnerOutcome::ProposalDecided {
                proposal: ProposalKey { proposer: Proposer::Task(u64::MAX), number: u64::MAX },
                by: u64::MAX,
                at: Wall::from_nanos(u64::MAX),
                choice: OwnerProposalChoice::Accepted,
            },
            &[
                b"Proposal decided",
                b"Proposing task: 18446744073709551615",
                b"Proposal number: 18446744073709551615",
                b"Decided by person: 18446744073709551615",
                b"Decided at: 18446744073709551615",
                b"Accepted",
            ],
        ),
        (
            OwnerOutcome::Said { task: u64::MAX, message: u64::MAX },
            &[b"Words admitted", b"Task: 18446744073709551615", b"Message: 18446744073709551615"],
        ),
        (
            OwnerOutcome::QuestionAnswered { task: u64::MAX, question: u64::MAX, message: u64::MAX },
            &[
                b"Question answered",
                b"Task: 18446744073709551615",
                b"Question: 18446744073709551615",
                b"Message: 18446744073709551615",
            ],
        ),
        (OwnerOutcome::Amended { task: u64::MAX }, &[b"Task amended", b"Task: 18446744073709551615"]),
        (
            OwnerOutcome::Proposed { proposal: u64::MAX, to: u32::MAX },
            &[b"Proposed", b"Proposal number: 18446744073709551615"],
        ),
        (OwnerOutcome::RolesSet { project: u32::MAX }, &[b"Project roles set", b"Project: 4294967295"]),
        (OwnerOutcome::PolicyChanged { project: u32::MAX }, &[b"Project policy changed", b"Project: 4294967295"]),
        (
            OwnerOutcome::PoolSet { project: u32::MAX, person: u64::MAX },
            &[b"Person pool set", b"Project: 4294967295", b"Person: 18446744073709551615"],
        ),
        (
            OwnerOutcome::EscalationDecided {
                task: u64::MAX,
                revision: Revision { value: u64::MAX },
                by: u64::MAX,
                at: Wall::from_nanos(u64::MAX),
                choice: OwnerEscalationChoice::Released,
            },
            &[
                b"Escalation decided",
                b"Task: 18446744073709551615",
                b"Hold revision: 18446744073709551615",
                b"Decided by person: 18446744073709551615",
                b"Decided at: 18446744073709551615",
                b"Released",
            ],
        ),
    ]
}
#[test]
fn all_committed_owner_outcomes_render_supplied_maximum_numbers() {
    let words = words();
    for (value, lines) in max_cases() {
        let tree = render(&value, &words);
        expect_lines(&tree, lines);
    }
}
#[test]
fn proposal_decisions_keep_both_makers_and_every_closed_choice_without_explanation() {
    let words = words();
    for proposer in [Proposer::Task(9), Proposer::Party(9)] {
        for (choice, wanted) in [
            (OwnerProposalChoice::Accepted, b"Accepted".as_slice()),
            (OwnerProposalChoice::Rejected, b"Rejected".as_slice()),
            (OwnerProposalChoice::Passed, b"Passed up".as_slice()),
            (OwnerProposalChoice::Withdrawn, b"Withdrawn".as_slice()),
            (OwnerProposalChoice::Stale, b"No longer applies".as_slice()),
        ] {
            let value = OwnerOutcome::ProposalDecided {
                proposal: ProposalKey { proposer, number: 17 },
                by: 23,
                at: Wall::from_nanos(31),
                choice,
            };
            let tree = render(&value, &words);
            let maker: &[u8] = match proposer {
                Proposer::Task(_) => b"Proposing task: 9",
                Proposer::Party(_) => b"Proposing party: 9",
            };
            expect_lines(
                &tree,
                &[
                    b"Proposal decided",
                    maker,
                    b"Proposal number: 17",
                    b"Decided by person: 23",
                    b"Decided at: 31",
                    wanted,
                ],
            );
            assert_eq!(tree.len(), 8, "all proposal facts remain visible");
        }
    }
}
#[test]
fn escalation_decisions_keep_revision_winner_clock_and_every_closed_choice() {
    let words = words();
    for (choice, wanted) in [
        (OwnerEscalationChoice::Released, b"Released".as_slice()),
        (OwnerEscalationChoice::Rejected, b"Rejected".as_slice()),
        (OwnerEscalationChoice::Passed, b"Passed up".as_slice()),
    ] {
        let value = OwnerOutcome::EscalationDecided {
            task: 7,
            revision: Revision { value: 13 },
            by: 19,
            at: Wall::from_nanos(29),
            choice,
        };
        let tree = render(&value, &words);
        expect_lines(
            &tree,
            &[
                b"Escalation decided",
                b"Task: 7",
                b"Hold revision: 13",
                b"Decided by person: 19",
                b"Decided at: 29",
                wanted,
            ],
        );
        assert!(tree.len() <= view::required(&words).unwrap().nodes);
    }
}
#[test]
fn released_flags_and_question_message_identities_remain_independent() {
    let words = words();
    {
        let cases: [(OwnerOutcome, &[&[u8]]); 3] = [
            (OwnerOutcome::Said { task: 7, message: 11 }, &[b"Words admitted", b"Task: 7", b"Message: 11"]),
            (OwnerOutcome::Amended { task: 13 }, &[b"Task amended", b"Task: 13"]),
            (OwnerOutcome::Proposed { proposal: 23, to: 0 }, &[b"Proposed", b"Proposal number: 23"]),
        ];
        for (value, expected) in cases {
            expect_lines(&render(&value, &words), expected);
        }
    }
    let question = render(&OwnerOutcome::QuestionAnswered { task: 7, question: 11, message: 19 }, &words);
    expect_lines(&question, &[b"Question answered", b"Task: 7", b"Question: 11", b"Message: 19"]);
    let seen = render(&OwnerOutcome::MarkedSeen { seen: SeenThrough::Conversation { task: 13, through: 17 } }, &words);
    expect_lines(&seen, &[b"Read position marked seen", b"Chat: 13", b"Replies seen through: 17"]);
}
#[test]
fn pool_person_remains_distinct_from_the_project() {
    let words = words();
    let pool = render(&OwnerOutcome::PoolSet { project: 13, person: 23 }, &words);
    expect_lines(&pool, &[b"Person pool set", b"Project: 13", b"Person: 23"]);
}
fn selected_language() -> Language {
    let mut language = phrases::english();
    for phrase in &mut language.phrases {
        for form in &mut phrase.forms {
            for segment in &mut form.segments {
                match segment {
                    Segment::Text(bytes) => *bytes = copy_of(b"selected committed outcome language: "),
                    Segment::Hole { .. } => {}
                }
            }
        }
    }
    language
}
#[test]
fn selected_outcome_language_is_checked_and_maximum_fragment_fits() {
    let book =
        Phrasebook::load(phrases::shapes(), Box::new([phrases::english(), selected_language()]), &phrase_limits())
            .unwrap();
    let words = Words::new(book, 1, 1);
    let value = OwnerOutcome::ProposalDecided {
        proposal: ProposalKey { proposer: Proposer::Party(u64::MAX), number: u64::MAX },
        by: u64::MAX,
        at: Wall::EPOCH,
        choice: OwnerProposalChoice::Rejected,
    };
    let tree = render(&value, &words);
    assert!(tree.len() <= view::required(&words).unwrap().nodes);
    assert!(has(&tree, b"selected committed outcome language: 18446744073709551615"));
    assert!(has(&tree, b"selected committed outcome language: 60000000000"));
    assert!(!has(&tree, b"Rejected"));
    assert_eq!(words.book().say(2, phrases::OutcomePhrase::Rejected.id(), &[]).unwrap_err(), Invalid::Language);
    assert_eq!(
        words.book().say(1, phrases::OutcomePhrase::At.id(), &[Hole::Number(7)]).unwrap_err(),
        Invalid::Values { id: phrases::OutcomePhrase::At.id() }
    );
    let mut missing = phrases::english();
    missing.phrases = Box::new([]);
    assert_eq!(
        Phrasebook::load(phrases::shapes(), Box::new([missing]), &phrase_limits()).unwrap_err(),
        Invalid::Missing { id: phrases::OutcomePhrase::MarkedSeen.id() }
    );
    let mut wrong = phrases::english();
    for phrase in &mut wrong.phrases {
        if phrase.id == phrases::OutcomePhrase::Revision.id() {
            phrase.forms[0].segments[1] = Segment::Hole { index: 0, kind: HoleKind::Wall };
        }
    }
    assert_eq!(
        Phrasebook::load(phrases::shapes(), Box::new([wrong]), &phrase_limits()).unwrap_err(),
        Invalid::Holes { id: phrases::OutcomePhrase::Revision.id() }
    );
}

#[test]
fn normal_words_register_every_outcome_once_and_bound_the_selected_language() {
    use crate::phrases::{english, jig};
    use skein_lib::List;

    let normal_shapes = jig::shapes(super::cards::CLIENT.text);
    assert_eq!(normal_shapes.len(), usize::try_from(crate::owner_words::phrase_count()).unwrap());
    for outcome in phrases::ALL {
        let mut copies = 0_u32;
        for shape in &normal_shapes {
            if shape.id == outcome.id() {
                copies = copies.checked_add(1).unwrap();
            }
        }
        assert_eq!(copies, 1, "each owner outcome shape is registered once");
    }
    let limits = PhraseLimits {
        languages: 2,
        phrases: crate::owner_words::phrase_count(),
        holes: 4,
        segments: 5000,
        text_bytes: 200_000,
        hole_bytes: super::cards::CLIENT.text,
    };
    let mut selected = english::language();
    for phrase in &mut selected.phrases {
        if phrase.id == phrases::OutcomePhrase::Rejected.id() {
            phrase.forms[0].segments[0] = Segment::Text(Box::new([b'R'; 2048]));
        }
    }
    let book = Phrasebook::load(normal_shapes, Box::new([english::language(), selected]), &limits).unwrap();
    for outcome in phrases::ALL {
        assert!(book.capacity(outcome.id()).is_some());
    }
    let words = Words::new(book, 1, 1);
    let fragment = view::required(&words).unwrap();
    assert!(fragment.text_per_node >= 2048);
    // This positive language proof needs one row, not every full page window.
    let client = crate::Limits { rows: 1, ..super::cards::CLIENT };
    assert!(crate::limits::of_words(&client, &words).unwrap().text_per_node >= 2048);
    let value = OwnerOutcome::ProposalDecided {
        proposal: ProposalKey { proposer: Proposer::Party(u64::MAX), number: 17 },
        by: 23,
        at: Wall::EPOCH,
        choice: OwnerProposalChoice::Rejected,
    };
    let tree = render(&value, &words);
    assert_eq!(tree.len(), 8);
    assert!(has(&tree, &[b'R'; 2048]));
    assert!(has(&tree, b"Proposing party: 18446744073709551615"));
    assert!(has(&tree, b"Proposal number: 17"));
    assert!(has(&tree, b"Decided by person: 23"));
    assert!(!has(&tree, b"Rejected"));

    let mut incomplete = english::language();
    let mut kept = List::with_capacity(crate::owner_words::phrase_count());
    for phrase in incomplete.phrases {
        if phrase.id != phrases::OutcomePhrase::Compound.id() {
            kept.push(phrase).unwrap();
        }
    }
    incomplete.phrases = kept.into_boxed();
    assert_eq!(
        Phrasebook::load(jig::shapes(super::cards::CLIENT.text), Box::new([incomplete]), &limits).unwrap_err(),
        Invalid::Missing { id: phrases::OutcomePhrase::Compound.id() }
    );
}

#[test]
fn durable_answer_rendering_keeps_every_owner_fact_within_the_existing_notice_envelope() {
    let words = words();
    let required = view::required(&words).unwrap();
    let limits = Limits {
        nodes: required.nodes,
        depth: required.depth,
        text_per_node: required.text_per_node,
        text_bytes: required.text_bytes,
        drawing_bytes: 0,
        styles_per_node: 0,
        patches: 0,
        bindings: 0,
        slots: 0,
        markdown_depth: 0,
        scene_marks: 0,
        scene_bytes: 0,
    };
    for (value, expected) in max_cases() {
        let mut builder = Builder::new(&limits);
        words.outcome(&mut builder, &jig_client::contract::Outcome::Owner(value));
        let tree = builder.finish();
        expect_lines(&tree, expected);
        assert!(tree.len() <= required.nodes);
    }
    // The enclosing notice includes the largest whole per-host outcome.
    let client = crate::Limits { rows: 0, notices: 1, ..super::cards::CLIENT };
    assert!(crate::notices_nodes(&client).unwrap() >= required.nodes + 4);
}

fn created_case() -> (OwnerOutcome, &'static [&'static [u8]]) {
    (
        OwnerOutcome::Created {
            first: 1,
            last: u64::MAX,
            given: Some(jig_client::contract::CreatedGiven { task: 2, amount: u64::MAX }),
        },
        &[
            b"Tasks created",
            b"First task: 1",
            b"Last task: 18446744073709551615",
            b"Budget given to task: 2",
            b"Amount given: 18446744073709551615",
        ],
    )
}

#[test]
fn the_account_sign_in_outcomes_say_their_phrases() {
    let words = words();
    expect_lines(
        &render(
            &OwnerOutcome::AccountSignInStarted { source: 7, address: b"http://issuer/visit".as_slice().into() },
            &words,
        ),
        &[b"LLM account sign-in started"],
    );
    for confirmed in [false, true] {
        expect_lines(
            &render(&OwnerOutcome::AccountSignedOut { confirmed, failed_hosts: Box::new([]) }, &words),
            if confirmed {
                &[b"LLM account signed out", b"Sign-out confirmed"]
            } else {
                &[b"LLM account signed out", b"Sign-out not confirmed"]
            },
        );
    }
}

#[test]
fn a_program_outcome_shows_its_declared_name() {
    let tree =
        render(&OwnerOutcome::AccountProgramDeclared { source: 0, name: b"personal".as_slice().into() }, &words());
    assert!(has(&tree, b"personal"));
}
#[test]
fn a_sign_out_outcome_shows_every_whole_host_name() {
    use jig_client::contract::{AccountHostFailure, AccountHostWhy};
    let mut rows = skein_lib::List::with_capacity(128);
    for _ in 0_u32..128 {
        rows.push(AccountHostFailure { name: Box::from([b'a'; 4096]), why: AccountHostWhy::NotInContact }).unwrap();
    }
    let rows = rows.into_boxed();
    let tree = render(&OwnerOutcome::AccountSignedOut { confirmed: false, failed_hosts: rows }, &words());
    assert_eq!(tree.len(), view::required(&words()).unwrap().nodes);
    assert!(has(&tree, &[b'a'; 4096]));
}

#[test]
fn project_outcomes_say_what_was_made_or_renamed_and_keep_the_number() {
    let words = words();
    expect_lines(&render(&OwnerOutcome::Renamed { project: 17 }, &words), &[b"Renamed the project", b"Project: 17"]);
    expect_lines(&render(&OwnerOutcome::ProjectMade { project: 19 }, &words), &[b"Made project", b"Project: 19"]);
}

#[test]
fn moved_in_outcomes_show_whether_the_account_was_renewed() {
    let words = words();
    for (renewed, expected) in [(false, b"moved in".as_slice()), (true, b"renewed")] {
        let tree = render(&OwnerOutcome::AccountMovedIn { account: u32::MAX, renewed }, &words);
        assert!(super::cards::contains(&tree, expected));
        assert!(super::cards::contains(&tree, b"4294967295"));
    }
}
