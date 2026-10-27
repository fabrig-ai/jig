//! Permanent people answers preserve their request and refusal facts (protocol/store.md, 5.1).
use super::{fixtures, people_round_trip};
use fixtures::people_outcomes as values;
use jig_domain_people as domain;
use skein_lib::Wall;

#[test]
fn account_move_asks_read_back_with_their_numbers_and_digest() {
    for change in [
        domain::AccountChange::MoveOut { account: 3 },
        domain::AccountChange::Written { number: 7 },
        domain::AccountChange::MoveIn { digest: [0xa5; 32] },
        domain::AccountChange::FinishSignIn { source: 5, digest: [0xa5; 32] },
    ] {
        super::ask_round_trip(domain::Ask::AccountChange { holder: domain::AccountHolder::Own, change });
    }
}

#[test]
fn account_move_answers_read_back_without_a_record() {
    for outcome in [
        domain::Outcome::AccountMovingOut { account: 3, number: 7, source: 5 },
        domain::Outcome::AccountChanged,
        domain::Outcome::AccountMovedIn { account: 3, renewed: false },
        domain::Outcome::AccountMovedIn { account: 3, renewed: true },
    ] {
        people_round_trip(values::answered(outcome));
    }
}

#[test]
fn every_account_move_refusal_reads_back() {
    for why in [
        domain::AccountMoveWhy::NotSignedIn,
        domain::AccountMoveWhy::NotOurs,
        domain::AccountMoveWhy::Held,
        domain::AccountMoveWhy::Cancelled,
        domain::AccountMoveWhy::Moving,
    ] {
        people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::AccountMove { why })));
    }
}
fn each_again_reads_back() {
    for value in values::again_values() {
        people_round_trip(values::answered(domain::Outcome::Again(value)));
    }
}
fn each_failure_class_reads_back() {
    for value in values::failure_class_values() {
        people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Moved {
            now: domain::Moved::Hold { revision: 3, reason: domain::HoldReason::Failures { class: value } },
        })));
    }
}
fn each_hold_reason_reads_back() {
    for value in values::hold_reason_values() {
        people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Moved {
            now: domain::Moved::Hold { revision: 3, reason: value },
        })));
    }
}
fn each_moved_reads_back() {
    for value in values::moved_values() {
        people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Moved { now: value })));
    }
}
fn each_party_reads_back() {
    for value in values::party_values() {
        people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Ended {
            how: domain::Ending::Cancelled { by: value, at: Wall::from_nanos(19) },
        })));
    }
}
fn each_ending_reads_back() {
    for value in values::ending_values() {
        people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Ended { how: value })));
    }
}
fn each_lack_reads_back() {
    for value in values::lack_values() {
        people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Authority {
            lacking: value,
            proposable: Some(domain::Accepter::Role(0)),
        })));
    }
}
fn each_limit_kind_reads_back() {
    for value in values::limit_kind_values() {
        people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Limit { which: value })));
    }
}
fn each_refusal_reads_back() {
    for value in values::refusal_values() {
        people_round_trip(values::answered(domain::Outcome::Refused(value)));
    }
}
fn each_proposal_choice_reads_back() {
    for value in values::proposal_choice_values() {
        people_round_trip(values::answered(domain::Outcome::ProposalDecided {
            proposer: domain::Proposer::Task(2),
            proposal: 3,
            by: 5,
            at: Wall::from_nanos(19),
            choice: value,
            repeated: false,
        }));
    }
}
fn each_escalation_choice_reads_back() {
    for value in values::escalation_choice_values() {
        people_round_trip(values::answered(domain::Outcome::EscalationDecided {
            task: 2,
            revision: 3,
            by: 5,
            at: Wall::from_nanos(19),
            choice: value,
        }));
    }
}
fn an_answered_key_reads_back_with_its_ask_and_outcome() {
    for value in values::outcome_values() {
        people_round_trip(values::answered(value));
    }
}
fn a_refusal_lacking_a_budget_reads_back() {
    people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Authority {
        lacking: domain::Lack::Budget { needs: 17, left: 11 },
        proposable: Some(domain::Accepter::Role(0)),
    })));
}
fn a_role_refusal_without_a_role_reads_back() {
    people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Role { held: None })));
}
fn an_authority_refusal_without_proposability_reads_back() {
    people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Authority {
        lacking: domain::Lack::Funding,
        proposable: None,
    })));
}
fn an_authority_refusal_prevents_a_proposal_reads_back() {
    people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Authority {
        lacking: domain::Lack::Funding,
        proposable: Some(domain::Accepter::Nobody),
    })));
}

#[test]
fn people_outcomes_read_back_as_written() {
    table([
        ("each_again_reads_back", each_again_reads_back),
        ("each_failure_class_reads_back", each_failure_class_reads_back),
        ("each_hold_reason_reads_back", each_hold_reason_reads_back),
        ("each_moved_reads_back", each_moved_reads_back),
        ("each_party_reads_back", each_party_reads_back),
        ("each_ending_reads_back", each_ending_reads_back),
        ("each_lack_reads_back", each_lack_reads_back),
        ("each_limit_kind_reads_back", each_limit_kind_reads_back),
        ("each_refusal_reads_back", each_refusal_reads_back),
        ("each_proposal_choice_reads_back", each_proposal_choice_reads_back),
        ("each_escalation_choice_reads_back", each_escalation_choice_reads_back),
        ("an_answered_key_reads_back_with_its_ask_and_outcome", an_answered_key_reads_back_with_its_ask_and_outcome),
        ("a_refusal_lacking_a_budget_reads_back", a_refusal_lacking_a_budget_reads_back),
        ("a_role_refusal_without_a_role_reads_back", a_role_refusal_without_a_role_reads_back),
        (
            "an_authority_refusal_without_proposability_reads_back",
            an_authority_refusal_without_proposability_reads_back,
        ),
        ("an_authority_refusal_prevents_a_proposal_reads_back", an_authority_refusal_prevents_a_proposal_reads_back),
    ]);
}

type Case = (&'static str, fn());

fn table<const N: usize>(cases: [Case; N]) {
    let mut failures = [None; N];
    let mut passed = true;
    for ((name, case), failure) in cases.into_iter().zip(&mut failures) {
        if std::panic::catch_unwind(case).is_err() {
            *failure = Some(name);
            passed = false;
        }
    }
    assert!(passed, "{failures:?}");
}

#[test]
fn an_account_sign_in_address_reads_back_at_its_bounds() {
    for length in [0, 1, domain::ADDRESS] {
        let address = skein_lib::bytes::zeroed(usize::try_from(length).expect("small bound"));
        people_round_trip(values::answered(domain::Outcome::AccountSignInStarted { source: 2, address }));
    }
}

#[test]
fn an_account_sign_in_address_over_its_bound_is_an_encode_error() {
    let address = skein_lib::bytes::zeroed(usize::try_from(domain::ADDRESS).expect("small bound") + 1);
    let row = values::answered(domain::Outcome::AccountSignInStarted { source: 2, address });
    let result = crate::codec::people::encode(&row, &super::people_limits(), &fixtures::model_names(&[]));
    assert_eq!(
        result,
        Err(crate::people_v13::Problem {
            path: crate::people_v13::Path::OutcomeAccountSignInStartedAddress,
            reason: skein_codec::Reason::Bound,
        })
    );
}

#[test]
fn settings_asks_outcomes_and_all_faults_round_trip_under_people_13() {
    super::ask_round_trip(domain::Ask::ChangeSettings { against: 7, digest: [0xa5; 32] });
    people_round_trip(values::answered(domain::Outcome::SettingsChanged { revision: 8 }));
    for why in [
        domain::Refusal::Stale { newest: 8 },
        domain::Refusal::Full { revision: 8, tasks: Box::new([1, 9]), more: 3 },
        domain::Refusal::NotWellFormed { at: 0, why: domain::Malformed::SettingsUnchanged },
    ] {
        people_round_trip(values::answered(domain::Outcome::Refused(why)));
    }
    for setting in
        [domain::Setting::Charter(1), domain::Setting::Template(2), domain::Setting::Policy(3), domain::Setting::Rules]
    {
        for fault in [
            domain::SettingFault::Shape,
            domain::SettingFault::Count,
            domain::SettingFault::Reference,
            domain::SettingFault::Rules,
            domain::SettingFault::Budget,
            domain::SettingFault::Grace,
            domain::SettingFault::Owner,
        ] {
            people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::NotWellFormed {
                at: 0,
                why: domain::Malformed::Settings { setting, fault },
            })));
        }
    }
}

#[test]
fn move_to_newest_asks_and_outcomes_round_trip_under_people_13() {
    super::ask_round_trip(domain::Ask::MoveToNewest { revision: 7 });
    people_round_trip(values::answered(domain::Outcome::MovedToNewest { newest: 9, moved: 3, stayed: 1 }));
}

#[test]
fn restore_asks_and_outcomes_round_trip_under_people_13() {
    super::ask_round_trip(domain::Ask::Restore { revision: 7, reason: b"earlier choices".as_slice().into() });
    people_round_trip(values::answered(domain::Outcome::Restored { proposal: 11, to: 7, left_out: Box::new([]) }));
    people_round_trip(values::answered(domain::Outcome::Refused(domain::Refusal::Forgotten { oldest: 3 })));
}
