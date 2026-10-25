#[path = "client/account_keys.rs"]
mod account_keys;
#[path = "client/accounts.rs"]
mod accounts;
#[path = "client/carried.rs"]
mod carried;
#[path = "client/llm.rs"]
mod llm;
#[path = "client/named.rs"]
mod named;
#[path = "client/named_accounts.rs"]
mod named_accounts;
#[path = "client/programs.rs"]
mod programs;
#[path = "client/words.rs"]
mod words;
use jig_client::{
    Answered, Ask, Asker, Handle, Key, Latest, Link, Outcome, Readable, Readout, State, Watchable, contract,
};
use jig_client_protocol_world::{
    client::{Referee, Story},
    fixtures,
};
use skein_lib::Duration;
const ME: Handle = Handle { asker: Asker::JIG, number: 1 };
const PROJECT: Handle = Handle { asker: Asker::JIG, number: 2 };
const INBOX: Handle = Handle { asker: Asker::JIG, number: 3 };
const TASK: u64 = 9;
#[test]
fn a_client_signed_in_by_the_printed_link_sees_me_live() {
    let mut story = Story::new(fixtures::SEED);
    story.sign_in();
    assert!(story.run_until(|s| s.client.link().state == Link::Live, Duration::from_secs(1)));
    assert_eq!(story.told_links(), vec![Link::Starting, Link::Live]);
    assert_eq!(story.client.link().party, Some(fixtures::FIRST_OWNER));
}
#[test]
fn a_client_reads_the_project_me_names_by_its_handle() {
    let mut story = Story::live(fixtures::SEED);
    story.watch(ME, Watchable::Jig(contract::Subject::Me));
    let Some(Latest::Jig(contract::Delivery::Me(me))) = story.client.document(ME).expect("watched handle").latest
    else {
        panic!("me");
    };
    let project = me.projects.as_slice().first().expect("owner's project").project;
    story.read(PROJECT, Readable::Jig(contract::Read::Project { project }));
    assert!(story.run_until(|s| !s.pages(PROJECT).is_empty(), Duration::from_secs(1)));
    let pages = story.pages(PROJECT);
    let [Readout::Jig(contract::ReadResult::Value(contract::ReadValue::OwnerProject(page)))] = pages.as_slice() else {
        panic!("one project page");
    };
    assert_eq!(page.number, project);
}
#[test]
fn a_dropped_stream_makes_each_handle_behind_then_live_with_me() {
    let mut story = Story::live(fixtures::SEED);
    story.watch(ME, Watchable::Jig(contract::Subject::Me));
    story.watch(INBOX, Watchable::Jig(contract::Subject::Inbox { window: 2 }));
    assert!(
        story.run_until(
            |s| s.client.document(INBOX).expect("watched handle").state == State::Live,
            Duration::from_secs(1)
        )
    );
    story.cut_stream();
    assert!(story.run_until(|s| s.client.link().state == Link::Offline, Duration::from_secs(1)));
    assert_eq!(story.client.document(ME).expect("watched handle").state, State::Behind);
    assert_eq!(story.client.document(INBOX).expect("watched handle").state, State::Behind);
    assert!(story.run_until(
        |s| s.client.link().state == Link::Live
            && s.client.document(INBOX).expect("watched handle").state == State::Live,
        Duration::from_secs(1)
    ));
    assert_eq!(story.client.document(ME).expect("watched handle").state, State::Live);
}
#[test]
fn each_delivery_the_client_took_reaches_the_core_as_taken() {
    let mut story = Story::live(fixtures::SEED);
    story.watch(INBOX, Watchable::Jig(contract::Subject::Inbox { window: 2 }));
    assert!(
        story.run_until(
            |s| s.client.document(INBOX).expect("watched handle").state == State::Live,
            Duration::from_secs(1)
        )
    );
    story.event(jig_client::Event::Unwatch { handle: INBOX });
    assert!(story.run_until(Story::deliveries_taken, Duration::from_secs(1)));
    assert_eq!(story.client.document(INBOX), None);
}
#[test]
fn a_document_older_than_one_told_is_caught() {
    let mut referee = Referee::default();
    let subject = Watchable::Jig(contract::Subject::Task { task: TASK });
    assert_eq!(referee.document(subject.clone(), 2), Ok(()));
    assert_eq!(referee.document(subject, 1), Err("document revision moved backwards"));
}

const STOP: Key = Key { asker: Asker(1), number: 1 };
const SECOND_STOP: Key = Key { asker: Asker(1), number: 2 };
const PROJECT_NUMBER: u32 = 1;
const WIRE_KEY: [u8; 16] = [19; 16];
fn stop(task: u64) -> Ask {
    Ask::Jig(Box::new(contract::Ask::Stop { project: PROJECT_NUMBER, task }))
}
fn assert_one_refusal(story: &Story) {
    let answers = story.answered(STOP);
    let [Outcome::Answered(Answered::Jig(contract::Answer::Refused { refusal: _, .. }))] = answers.as_slice() else {
        panic!("one refusal: {answers:?}");
    };
    assert_eq!(story.made(fixtures::FIRST_OWNER), 1);
}
#[test]
fn a_stop_is_answered_once_after_its_commit_is_durable() {
    let mut story = Story::live(fixtures::SEED);
    story.send(STOP, stop(TASK));
    assert!(story.run_until(|s| !s.answered(STOP).is_empty(), Duration::from_secs(1)));
    assert_one_refusal(&story);
    assert!(story.told_at(STOP) >= story.durable_at(fixtures::FIRST_OWNER));
}
#[test]
fn a_stop_kept_across_a_reload_is_answered_once() {
    let mut story = Story::live(fixtures::SEED);
    story.cut_answer();
    story.send(STOP, stop(TASK));
    assert!(story.run_until(|s| s.made(fixtures::FIRST_OWNER) == 1, Duration::from_secs(1)));
    assert_eq!(story.answered(STOP), vec![]);
    story.reload();
    assert!(story.run_until(|s| !s.answered(STOP).is_empty(), Duration::from_secs(1)));
    assert!(story.kept(STOP));
    assert_one_refusal(&story);
}
#[test]
fn a_stop_whose_record_is_refused_is_answered_not_kept_and_never_made() {
    let mut story = Story::live(fixtures::SEED);
    story.refuse_next_save();
    story.send(STOP, stop(TASK));
    assert!(story.run_until(|s| !s.answered(STOP).is_empty(), Duration::from_secs(1)));
    assert_eq!(story.answered(STOP), vec![Outcome::NotKept]);
    assert_eq!(story.made(fixtures::FIRST_OWNER), 0);
}
#[test]
fn the_engine_restarting_under_a_pending_stop_answers_it_once() {
    let mut story = Story::live(fixtures::SEED);
    story.cut_answer();
    story.send(STOP, stop(TASK));
    assert!(story.run_until(|s| s.made(fixtures::FIRST_OWNER) == 1, Duration::from_secs(1)));
    assert_eq!(story.answered(STOP), vec![]);
    story.restart_engine(Duration::from_millis(10));
    assert!(story.run_until(|s| !s.answered(STOP).is_empty(), Duration::from_secs(1)));
    assert_one_refusal(&story);
}
#[test]
fn two_clients_stopping_one_task_are_answered_once_each() {
    let mut story = Story::live(fixtures::SEED);
    let task = story.start_task();
    let made = story.made(fixtures::FIRST_OWNER);
    story.add_second_client();
    story.send(STOP, stop(task));
    story.send_second(SECOND_STOP, stop(task));
    assert!(story.run_until(
        |s| !s.answered(STOP).is_empty() && !s.second().answered(SECOND_STOP).is_empty(),
        Duration::from_secs(1)
    ));
    let answers = story.answered(STOP);
    assert_eq!(answers.len(), 1);
    assert_eq!(story.second().answered(SECOND_STOP), answers);
    assert_eq!(story.made(fixtures::FIRST_OWNER), made + 2);
}
#[test]
fn a_keyed_exchange_before_its_store_is_caught() {
    let referee = Referee::default();
    assert_eq!(
        referee.keyed(WIRE_KEY, Some(fixtures::FIRST_OWNER)),
        Err("keyed exchange left before its record was stored")
    );
}

#[test]
fn a_client_signed_out_with_a_stop_parked_sends_it_once_signed_in_again() {
    let mut story = Story::live(fixtures::SEED);
    story.event(jig_client::Event::SignOut);
    story.send(STOP, Ask::Jig(Box::new(contract::Ask::Stop { project: 1, task: 9 })));
    assert!(story.run_until(|s| s.client.link().state == Link::SignedOut, Duration::from_secs(1)));
    assert_eq!(story.client.pending(STOP).expect("kept stop").progress, jig_client::Progress::Parked);
    assert_eq!(story.made(fixtures::FIRST_OWNER), 0);
    story.sign_in();
    assert!(story.run_until(|s| !s.answered(STOP).is_empty(), Duration::from_secs(1)));
    assert_eq!(story.client.link().party, Some(fixtures::FIRST_OWNER));
    assert_eq!(story.answered(STOP).len(), 1);
    assert_eq!(story.made(fixtures::FIRST_OWNER), 1);
}

#[test]
fn a_family_request_is_answered_as_its_owner_s_bytes() {
    use jig_client_test_domain::{connector_values::ConnectorAnswer, first};
    use jig_test_connector::{ApplicationObject, ApplicationOutcome};
    let mut story = Story::live(fixtures::SEED);
    let request = first::Ask::ChangeObject {
        project: PROJECT_NUMBER,
        object: 1,
        revision: contract::Revision { value: 0 },
        value: 42,
    };
    story.send(
        STOP,
        Ask::Family {
            scope: jig_client_domain::contract::Scope::Project(1),
            record: jig_client_protocol_world::family::request(&request),
        },
    );
    assert!(story.run_until(|s| !s.answered(STOP).is_empty(), Duration::from_secs(1)));
    let answers = story.answered(STOP);
    let [Outcome::Answered(Answered::Family(answer))] = answers.as_slice() else {
        panic!("one owner's byte answer: {answers:?}");
    };
    assert_eq!(
        jig_client_protocol_world::family::answer(answer),
        Some(first::Answer::Connector(ConnectorAnswer {
            value: ApplicationOutcome::Changed { object: 1, now: ApplicationObject { revision: 1, value: 42 } },
        }))
    );
    assert_eq!(story.made(fixtures::FIRST_OWNER), 1);
}

#[test]
fn an_engine_outside_jig_s_version_leaves_the_client_out_of_date_sending_nothing() {
    let mut story = Story::live(fixtures::SEED);
    story.serve_jig(8);
    assert!(story.run_until(|s| s.client.link().state == Link::OutOfDate, Duration::from_secs(1)));
    let exchanges = story.exchanges();
    story.send(STOP, stop(TASK));
    story.read(PROJECT, Readable::Jig(contract::Read::Project { project: PROJECT_NUMBER }));
    story.watch(INBOX, Watchable::Jig(contract::Subject::Inbox { window: 2 }));
    assert!(story.run_until(|_| true, Duration::from_secs(1)));
    assert_eq!(story.exchanges(), exchanges);
    assert_eq!(story.made(fixtures::FIRST_OWNER), 0);
    assert_eq!(story.client.pending(STOP).expect("kept stop").progress, jig_client::Progress::Parked);
    assert_eq!(story.client.next_deadline(), None);
    assert_eq!(
        story.pages(PROJECT),
        vec![Readout::NotServed { family: None, served: Some(contract::VersionRange { first: 8, last: 8 }) }]
    );
}

const FAMILY_REQUEST: Key = Key { asker: Asker(1), number: 77 };
const FAMILY_OBJECT: u64 = 1;
const PROOF_WAIT: Duration = Duration::from_secs(1);
const HELD_WAIT: Duration = Duration::from_millis(160);
fn family_change() -> jig_client_test_domain::first::Ask {
    jig_client_test_domain::first::Ask::ChangeObject {
        project: PROJECT_NUMBER,
        object: FAMILY_OBJECT,
        revision: contract::Revision { value: 0 },
        value: 42,
    }
}
#[test]
fn a_request_pending_across_a_restart_that_stops_serving_its_family_stays_unsent() {
    let mut story = Story::live(fixtures::SEED);
    story.cut_answer();
    story.send(
        FAMILY_REQUEST,
        Ask::Family {
            scope: contract::Scope::Project(PROJECT_NUMBER),
            record: jig_client_protocol_world::family::request(&family_change()),
        },
    );
    assert!(story.run_until(|s| s.made(fixtures::FIRST_OWNER) == 1, PROOF_WAIT));
    assert_eq!(story.answered(FAMILY_REQUEST), vec![]);
    story.restart_serving(jig_client_world::test::list([
        jig_protocol_engine_clients::Served { family: None, first: 19, last: 19 },
        jig_protocol_engine_clients::Served { family: Some(1), first: 3, last: 3 },
    ]));
    assert!(story.run_until(|s| s.client.link().state == Link::Live, PROOF_WAIT));
    assert_eq!(story.client.pending(FAMILY_REQUEST).expect("kept request").progress, jig_client::Progress::NotServed);
    assert_eq!(story.client.link().not_served, &[1]);
    let exchanges = story.exchanges();
    assert!(!story.run_until(|_| false, HELD_WAIT));
    assert_eq!(story.exchanges(), exchanges);
    assert_eq!(story.answered(FAMILY_REQUEST), vec![]);
    assert_eq!(story.made(fixtures::FIRST_OWNER), 1);
}
#[test]
fn a_family_request_in_the_deployment_scope_reaches_the_people_child_with_it() {
    let mut story = Story::live_deployment(fixtures::SEED);
    story.send(
        FAMILY_REQUEST,
        Ask::Family {
            scope: contract::Scope::Deployment,
            record: jig_client_protocol_world::family::request(&family_change()),
        },
    );
    assert!(story.run_until(|s| !s.answered(FAMILY_REQUEST).is_empty(), PROOF_WAIT));
    let asks = story.application_asks();
    let [jig_domain_people::Ask::Application { scope, connector, .. }] = asks.as_slice() else {
        panic!("one admitted application ask: {asks:?}")
    };
    assert_eq!((*scope, *connector), (jig_domain_people::ApplicationScope::Deployment, 1));
}

const CANCEL: Key = Key { asker: Asker::JIG, number: 81 };
const OVERRIDE: Key = Key { asker: Asker::JIG, number: 82 };
const GRACE: Duration = Duration::from_millis(20);
const OVERRIDE_BUDGET: u64 = 50;
#[test]
fn a_cancel_with_a_grace_ends_the_task_once_its_grace_has_passed() {
    let mut story = Story::live(fixtures::SEED);
    let task = story.running_task();
    let reason = contract::Text::new(Box::from(b"enough".as_slice()), 64).expect("fits");
    let cancel = contract::Ask::Cancel { project: PROJECT_NUMBER, task, reason, grace: Some(GRACE) };
    story.send(CANCEL, Ask::Jig(Box::new(cancel)));
    assert!(story.run_until(|s| !s.answered(CANCEL).is_empty(), PROOF_WAIT));
    assert_eq!(
        story.answered(CANCEL),
        vec![Outcome::Answered(Answered::Jig(contract::Answer::Done(contract::Outcome::Owner(
            contract::OwnerOutcome::Cancelled { task }
        ))))]
    );
    assert!(story.heard_asked_to_finish(task));
    assert!(story.phase(task) != Some(contract::Phase::Cancelled), "the grace has not passed");
    assert!(story.run_until(|s| s.phase(task) == Some(contract::Phase::Cancelled), PROOF_WAIT));
    assert!(story.elapsed_since_answer(CANCEL) >= GRACE);
}
#[test]
fn an_owner_s_template_override_is_answered_policy_changed() {
    let mut story = Story::live(fixtures::SEED);
    let mut overrides = skein_lib::List::with_capacity(1);
    overrides
        .push(contract::TemplateOverride {
            template: 2,
            executor: None,
            budget: Some(contract::TemplateBudget::Amount(OVERRIDE_BUDGET)),
            deadline: None,
            priority: None,
        })
        .expect("one override");
    story.send(
        OVERRIDE,
        Ask::Jig(Box::new(contract::Ask::ChangePolicy {
            project: PROJECT_NUMBER,
            edit: contract::PolicyEdit::Templates { overrides },
        })),
    );
    assert!(story.run_until(|s| !s.answered(OVERRIDE).is_empty(), PROOF_WAIT));
    assert_eq!(
        story.answered(OVERRIDE),
        vec![Outcome::Answered(Answered::Jig(contract::Answer::Done(contract::Outcome::Owner(
            contract::OwnerOutcome::PolicyChanged { project: PROJECT_NUMBER }
        ))))]
    );
}

const COMPOUND_STOP: Key = Key { asker: Asker::JIG, number: 83 };
const COMPOUND: Key = Key { asker: Asker::JIG, number: 84 };
#[test]
fn a_compound_answer_names_the_durable_words_before_its_release() {
    let mut story = Story::live(fixtures::SEED);
    let task = story.running_task();
    story.send(COMPOUND_STOP, Ask::Jig(Box::new(contract::Ask::Stop { project: PROJECT_NUMBER, task })));
    assert!(story.run_until(|s| s.phase(task) == Some(contract::Phase::Held), PROOF_WAIT));
    let mut operations = skein_lib::List::with_capacity(2);
    operations
        .push(contract::Operation::Message {
            project: PROJECT_NUMBER,
            task,
            words: contract::Text::new(b"Please resume".as_slice().into(), 64).expect("bounded compound fixture"),
        })
        .expect("bounded compound fixture");
    operations
        .push(contract::Operation::Release {
            project: PROJECT_NUMBER,
            task,
            revision: story.hold_revision(task),
            again: None,
        })
        .expect("bounded compound fixture");
    story.send(COMPOUND, Ask::Jig(Box::new(contract::Ask::Compound { project: PROJECT_NUMBER, operations })));
    assert!(story.run_until(|s| !s.answered(COMPOUND).is_empty(), PROOF_WAIT));
    let answers = story.answered(COMPOUND);
    let [
        Outcome::Answered(Answered::Jig(contract::Answer::Done(contract::Outcome::Owner(
            contract::OwnerOutcome::Compound { project, operations, first_message },
        )))),
    ] = answers.as_slice()
    else {
        panic!("compound answer: {answers:?}");
    };
    assert_eq!((*project, *operations), (PROJECT_NUMBER, 2));
    assert_eq!(story.activity_message(task, *first_message), Some(b"Please resume".to_vec()));
    let message = *first_message;
    assert!(story.run_until(|s| s.worker_started_with(task, message), PROOF_WAIT));
}

const CREATE: Key = Key { asker: Asker::JIG, number: 85 };
#[test]
fn a_party_creates_a_batch_whose_as_allowed_task_hears_what_it_was_given() {
    let mut story = Story::live_batch(fixtures::SEED, true);
    assert_eq!(story.client.link().party, Some(fixtures::FIRST_OWNER));
    story.send(CREATE, Ask::Jig(Box::new(contract::Ask::Create { project: 1, tasks: fixtures::create_tasks() })));
    assert!(story.run_until(|s| !s.answered(CREATE).is_empty(), PROOF_WAIT));
    assert_eq!(
        story.answered(CREATE),
        vec![Outcome::Answered(Answered::Jig(contract::Answer::Done(contract::Outcome::Owner(
            contract::OwnerOutcome::Created {
                first: 1,
                last: 2,
                given: Some(contract::CreatedGiven { task: 1, amount: 20 })
            }
        ))))]
    );
    assert_eq!((story.task_budget(1), story.task_budget(2)), (Some(20), Some(10)));
    assert!(story.run_until(|s| s.worker_budget(1).is_some(), PROOF_WAIT));
    assert_eq!(story.worker_budget(1), Some(20));
}
#[test]
fn a_root_task_with_no_one_to_escalate_to_is_refused_saying_so() {
    let mut story = Story::live_batch(fixtures::SEED, false);
    story.send(CREATE, Ask::Jig(Box::new(contract::Ask::Create { project: 1, tasks: fixtures::create_tasks() })));
    assert!(story.run_until(|s| !s.answered(CREATE).is_empty(), PROOF_WAIT));
    assert_eq!(
        story.answered(CREATE),
        vec![Outcome::Answered(Answered::Jig(contract::Answer::Refused {
            refusal: contract::Refusal::NoEscalationHolder,
            at: None
        }))]
    );
    assert_eq!((story.task_budget(1), story.task_budget(2)), (None, None));
}

const PROPOSE: Key = Key { asker: Asker::JIG, number: 86 };
const WITHDRAW: Key = Key { asker: Asker::JIG, number: 87 };
const PROPOSAL: Handle = Handle { asker: Asker::JIG, number: 88 };
#[test]
fn a_party_proposes_a_batch_and_withdraws_it_before_it_is_decided() {
    let mut story = Story::live_batch(fixtures::SEED, true);
    story.send(
        PROPOSE,
        Ask::Jig(Box::new(contract::Ask::Propose {
            project: 1,
            action: contract::ProposeAction::Batch { tasks: fixtures::proposed_tasks() },
            reason: contract::Text::new(b"more work".as_slice().into(), 32).expect("bounded proposal reason"),
        })),
    );
    assert!(story.run_until(|s| !s.answered(PROPOSE).is_empty(), PROOF_WAIT));
    assert_eq!(
        story.answered(PROPOSE),
        vec![Outcome::Answered(Answered::Jig(contract::Answer::Done(contract::Outcome::Owner(
            contract::OwnerOutcome::Proposed { proposal: 1, to: 0 }
        ))))]
    );
    let key = contract::ProposalKey { proposer: contract::Proposer::Party(fixtures::FIRST_OWNER), number: 1 };
    story.read(PROPOSAL, Readable::Jig(contract::Read::Proposal { proposal: key }));
    assert!(story.run_until(|s| !s.pages(PROPOSAL).is_empty(), PROOF_WAIT));
    let pages = story.pages(PROPOSAL);
    let [Readout::Jig(contract::ReadResult::Value(contract::ReadValue::OwnerProposal(value)))] = pages.as_slice()
    else {
        panic!("proposal document: {pages:?}")
    };
    assert_eq!(value.key, key);
    assert_eq!(
        value.waiting.holder,
        contract::OwnerHistoryHolder::Policy { project: 1, kind: contract::OwnerMessageProposalKind::Batch }
    );
    assert_eq!(story.party_proposal(1).expect("durable party proposal").proposer, fixtures::FIRST_OWNER);
    story.send(WITHDRAW, Ask::Jig(Box::new(contract::Ask::Withdraw { project: 1, proposal: 1 })));
    assert!(story.run_until(|s| !s.answered(WITHDRAW).is_empty(), PROOF_WAIT));
    assert_eq!(
        story.answered(WITHDRAW),
        vec![Outcome::Answered(Answered::Jig(contract::Answer::Done(contract::Outcome::Owner(
            contract::OwnerOutcome::Withdrawn { proposal: 1 }
        ))))]
    );
    assert!(matches!(
        story.party_proposal(1).expect("durable party proposal").state,
        jig_domain_tasks::PersonProposalState::Withdrawn { .. }
    ));
    assert_eq!(story.party_proposal(1).expect("durable party proposal").proposer, fixtures::FIRST_OWNER);
    assert_eq!((story.task_budget(1), story.task_budget(2)), (None, None));
}
#[test]
fn a_create_beyond_authority_is_refused_saying_a_proposal_could_be_accepted() {
    let mut story = Story::live_batch(fixtures::SEED, true);
    story.send(CREATE, Ask::Jig(Box::new(contract::Ask::Create { project: 1, tasks: fixtures::proposed_tasks() })));
    assert!(story.run_until(|s| !s.answered(CREATE).is_empty(), PROOF_WAIT));
    assert_eq!(
        story.answered(CREATE),
        vec![Outcome::Answered(Answered::Jig(contract::Answer::Refused {
            refusal: contract::Refusal::Authority {
                lacking: contract::Lacking::Budget { needed: 160, left: 100 },
                proposable: Some(true)
            },
            at: None,
        }))]
    );
    assert_eq!(story.party_proposal(1), None);
    assert_eq!((story.task_budget(1), story.task_budget(2)), (None, None));
}

const OVERRIDE_TEMPLATE: Key = Key { asker: Asker::JIG, number: 501 };
const GOAL_TEMPLATE: u32 = 2;
const CHAT_TEMPLATE: u32 = 1;
const ME_WAIT: Duration = Duration::from_secs(1);
#[test]
fn me_serves_a_template_merged_with_its_project_s_override() {
    let mut story = Story::live(fixtures::SEED);
    story.watch(ME, Watchable::Jig(contract::Subject::Me));
    let override_goal = contract::TemplateOverride {
        template: GOAL_TEMPLATE,
        executor: None,
        budget: Some(contract::TemplateBudget::Amount(42)),
        deadline: None,
        priority: None,
    };
    let mut overrides = skein_lib::List::with_capacity(1);
    overrides.push(override_goal.clone()).expect("one override");
    story.send(
        OVERRIDE_TEMPLATE,
        Ask::Jig(Box::new(contract::Ask::ChangePolicy {
            project: PROJECT_NUMBER,
            edit: contract::PolicyEdit::Templates { overrides },
        })),
    );
    assert!(story.run_until(|s| !s.answered(OVERRIDE_TEMPLATE).is_empty(), ME_WAIT));
    assert_eq!(
        story.answered(OVERRIDE_TEMPLATE),
        vec![Outcome::Answered(Answered::Jig(contract::Answer::Done(contract::Outcome::Owner(
            contract::OwnerOutcome::PolicyChanged { project: PROJECT_NUMBER }
        ))))]
    );
    assert!(
        story
            .run_until(|s| me_template_budget(s, GOAL_TEMPLATE) == Some(contract::TemplateBudget::Amount(42)), ME_WAIT)
    );
    assert_eq!(me_template_budget(&story, CHAT_TEMPLATE), Some(contract::TemplateBudget::AsAllowed { cap: None }));
    story.read(PROJECT, Readable::Jig(contract::Read::Project { project: PROJECT_NUMBER }));
    assert!(story.run_until(|s| !s.pages(PROJECT).is_empty(), ME_WAIT));
    let pages = story.pages(PROJECT);
    let [Readout::Jig(contract::ReadResult::Value(contract::ReadValue::OwnerProject(project)))] = pages.as_slice()
    else {
        panic!("project page")
    };
    assert_eq!(project.policy.templates.as_slice(), &[override_goal]);
}
fn me_template_budget(story: &Story, template: u32) -> Option<contract::TemplateBudget> {
    let Some(Latest::Jig(contract::Delivery::Me(me))) = story.client.document(ME)?.latest else { return None };
    me.projects
        .iter()
        .find(|row| row.project == PROJECT_NUMBER)?
        .templates
        .iter()
        .find(|row| row.number == template)
        .map(|row| row.budget)
}

const QUERY: Handle = Handle { asker: Asker::JIG, number: 5 };
const QUERY_AMEND: Key = Key { asker: Asker::JIG, number: 91 };
#[test]
fn a_task_query_watch_delivers_the_live_lines_its_keys_select() {
    let mut story = Story::live_batch(fixtures::SEED, true);
    story.send(
        CREATE,
        Ask::Jig(Box::new(contract::Ask::Create { project: PROJECT_NUMBER, tasks: fixtures::tracked_tasks() })),
    );
    assert!(story.run_until(|s| !s.answered(CREATE).is_empty(), PROOF_WAIT));
    story.watch(
        QUERY,
        Watchable::Jig(contract::Subject::Tasks { query: fixtures::task_query(PROJECT_NUMBER), window: 2 }),
    );
    assert!(story.run_until(|s| fixtures::task_lines(s, QUERY).is_some(), PROOF_WAIT));
    assert_eq!(fixtures::tracked_rows(&story, QUERY), vec![(fixtures::FIRST_QUERY_TASK, Some(1))]);
    story.send(
        QUERY_AMEND,
        Ask::Jig(Box::new(contract::Ask::Amend {
            project: PROJECT_NUMBER,
            task: fixtures::SECOND_QUERY_TASK,
            revision: None,
            change: Box::new(fixtures::tracked_amendment(2)),
            reason: contract::Text::new(b"track".as_slice().into(), 32).expect("query story fixture and delivery"),
        })),
    );
    assert!(story.run_until(|s| fixtures::task_lines(s, QUERY).is_some_and(|tasks| tasks.rows.len() == 2), PROOF_WAIT));
    assert_eq!(
        fixtures::tracked_rows(&story, QUERY),
        vec![(fixtures::FIRST_QUERY_TASK, Some(1)), (fixtures::SECOND_QUERY_TASK, Some(2))]
    );
}

#[test]
fn a_task_line_says_what_its_task_waits_on() {
    let mut story = Story::live_person_batch(fixtures::SEED);
    story.send(
        CREATE,
        Ask::Jig(Box::new(contract::Ask::Create { project: PROJECT_NUMBER, tasks: fixtures::person_tasks() })),
    );
    assert!(story.run_until(|s| !s.answered(CREATE).is_empty(), PROOF_WAIT));
    story.watch(
        QUERY,
        Watchable::Jig(contract::Subject::Tasks { query: fixtures::task_query(PROJECT_NUMBER), window: 2 }),
    );
    assert!(story.run_until(|s| fixtures::task_lines(s, QUERY).is_some(), PROOF_WAIT));
    let rows = &fixtures::task_lines(&story, QUERY).expect("query story fixture and delivery").rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows.get(0).expect("query story fixture and delivery").waits_on,
        contract::WaitsOn::Party {
            whom: contract::Whom::Person(fixtures::FIRST_OWNER),
            entries: 1,
            task: fixtures::FIRST_QUERY_TASK,
            kind: contract::WaitedFor::PersonTask
        }
    );
}

fn query_read(story: &mut Story, query: contract::TaskQuery) -> contract::TaskPage {
    story.read(QUERY, Readable::Jig(contract::Read::Tasks { query, live: None, ended: None }));
    assert!(story.run_until(|s| !s.pages(QUERY).is_empty(), PROOF_WAIT));
    let pages = story.pages(QUERY);
    let [Readout::Jig(contract::ReadResult::Value(contract::ReadValue::Tasks(page)))] = pages.as_slice() else {
        panic!("task query page: {pages:?}")
    };
    *page.clone()
}
#[test]
fn an_ended_task_is_read_through_its_requester_s_index_after_the_live_ones() {
    let mut story = Story::live_batch(fixtures::SEED, true);
    let mut tasks = fixtures::create_tasks();
    tasks.get_mut(1).expect("second independent member").dependencies = skein_lib::List::with_capacity(0);
    story.send(CREATE, Ask::Jig(Box::new(contract::Ask::Create { project: PROJECT_NUMBER, tasks })));
    assert!(story.run_until(|s| !s.answered(CREATE).is_empty(), PROOF_WAIT));
    story.send(
        CANCEL,
        Ask::Jig(Box::new(contract::Ask::Cancel {
            project: PROJECT_NUMBER,
            task: fixtures::FIRST_QUERY_TASK,
            reason: contract::Text::new(b"done".as_slice().into(), 32).expect("reason fits"),
            grace: None,
        })),
    );
    assert!(story.run_until(|s| s.phase(fixtures::FIRST_QUERY_TASK) == Some(contract::Phase::Cancelled), PROOF_WAIT));
    let mut query = fixtures::task_query(PROJECT_NUMBER);
    query.project = None;
    query.tracked = None;
    query.requester = Some(contract::OwnerRequester::Person(fixtures::FIRST_OWNER));
    let page = query_read(&mut story, query);
    assert_eq!(page.live.iter().map(|line| line.chip.task).collect::<Vec<_>>(), [fixtures::SECOND_QUERY_TASK]);
    let contract::EndedGroup::Rows { rows, after, cut } = page.ended else { panic!("requester index") };
    assert_eq!(rows.iter().map(|line| line.chip.task).collect::<Vec<_>>(), [fixtures::FIRST_QUERY_TASK]);
    assert_eq!((page.live_after, after, cut), (None, None, false));
}
#[test]
fn a_query_naming_no_index_key_reads_no_ended_group() {
    let mut story = Story::live(fixtures::SEED);
    let task = story.running_task();
    let mut query = fixtures::task_query(PROJECT_NUMBER);
    query.project = None;
    query.tracked = None;
    let page = query_read(&mut story, query);
    assert_eq!(page.live.iter().map(|line| line.chip.task).collect::<Vec<_>>(), [task]);
    assert_eq!(page.ended, contract::EndedGroup::None);
    assert_eq!(page.live_after, None);
}

#[test]
fn a_running_chat_reads_as_writing_from_the_engine_s_documents() {
    use jig_client::facts::ChatStatus;
    let mut story = Story::writing_chat(fixtures::SEED);
    let task = story.running_task();
    story.watch_chat_facts(task);
    assert!(story.run_until(|s| s.chat_statuses(task) == Some((ChatStatus::Writing, ChatStatus::Writing)), PROOF_WAIT));
    assert_eq!(story.chat_statuses(task), Some((ChatStatus::Writing, ChatStatus::Writing)));
    let waiting = ChatStatus::WaitingForYou { kind: contract::WaitedFor::Question, entries: 1 };
    assert!(story.run_until(|s| s.chat_statuses(task) == Some((waiting.clone(), waiting.clone())), PROOF_WAIT));
    assert_eq!(story.chat_statuses(task), Some((waiting.clone(), waiting)));
}

const SHAPED_CHAT: Key = Key { asker: Asker::JIG, number: 101 };
const SHAPED_CLOSE: Key = Key { asker: Asker::JIG, number: 102 };
const SHAPED_GOAL: Key = Key { asker: Asker::JIG, number: 103 };
const SHAPED_DELEGATE: u64 = 2;
const CLOSE_FIRST_MESSAGE: u64 = 2;
const GOAL_PROPOSAL: u64 = 1;
const GRACE_PASSED: Duration = Duration::from_secs(6);
#[test]
fn a_chat_made_by_shapes_is_a_conversation_under_the_chat_label() {
    let mut story = Story::live(fixtures::SEED);
    let task = story.shapes_chat(SHAPED_CHAT);
    story.watch_chat_facts(task);
    assert!(story.run_until(|s| s.line(task).is_some(), PROOF_WAIT));
    let line = story.line(task).expect("delivered chat line");
    assert_eq!(line.label, 1);
    assert!(line.flags.conversation);
}
#[test]
fn closing_a_chat_by_shapes_moves_its_delegate_then_cancels_it_after_its_grace() {
    let mut story = Story::writing_chat(fixtures::SEED);
    let task = story.running_task();
    story.watch_chat_facts(task);
    assert!(story.run_until(|s| s.phase(SHAPED_DELEGATE).is_some(), PROOF_WAIT));
    story.watch_delegate(SHAPED_DELEGATE);
    assert!(story.run_until(|s| s.delegate_requester().is_some(), PROOF_WAIT));
    assert_eq!(story.delegate_requester(), Some(contract::OwnerRequester::Task(task)));
    let grace = close_shaped_chat(&mut story, task);
    assert!(story.run_until(
        |s| s.delegate_requester() == Some(contract::OwnerRequester::Person(fixtures::FIRST_OWNER)),
        PROOF_WAIT
    ));
    assert_ne!(story.phase(task), Some(contract::Phase::Cancelled));
    assert!(story.run_until(|s| s.phase(task) == Some(contract::Phase::Cancelled), GRACE_PASSED));
    assert!(story.elapsed_since_answer(SHAPED_CLOSE) >= grace);
    assert_ne!(story.phase(SHAPED_DELEGATE), Some(contract::Phase::Cancelled));
}
#[test]
fn a_goal_past_the_first_bound_is_proposed_to_the_policy_s_role() {
    use jig_client::shapes::{self, GoalDraft, Judged};
    let mut story = Story::live(fixtures::SEED);
    let me = story.shapes_me();
    let words = contract::Text::new(b"goal".as_slice().into(), 32).expect("words fit");
    let draft = GoalDraft {
        words: words.clone(),
        executor: contract::OwnerExecutor::Agent { charter: 1 },
        budget: 150,
        deadline: None,
        priority: Some(7),
        llm: None,
    };
    let Judged::Propose { ask, to } = shapes::set_goal(&me, PROJECT_NUMBER, draft, words) else {
        panic!("goal proposed")
    };
    let mut roles = skein_lib::List::with_capacity(1);
    roles.push(0).expect("owner role");
    assert_eq!(to, contract::Recipient::Roles { project: PROJECT_NUMBER, roles });
    story.send(SHAPED_GOAL, Ask::Jig(Box::new(ask)));
    assert!(story.run_until(|s| !s.answered(SHAPED_GOAL).is_empty(), PROOF_WAIT));
    assert_eq!(
        story.answered(SHAPED_GOAL),
        vec![Outcome::Answered(Answered::Jig(contract::Answer::Done(contract::Outcome::Owner(
            contract::OwnerOutcome::Proposed { proposal: GOAL_PROPOSAL, to: 0 }
        ))))]
    );
}

fn close_shaped_chat(story: &mut Story, task: u64) -> Duration {
    let me = story.shapes_me();
    let grace = jig_client::shapes::template(&me, PROJECT_NUMBER, jig_client::shapes::CHAT)
        .expect("chat template")
        .grace
        .expect("grace");
    let reason = contract::Text::new(b"enough".as_slice().into(), 32).expect("reason fits");
    let ask = jig_client::shapes::close_chat(
        &me,
        PROJECT_NUMBER,
        task,
        &[jig_client::shapes::Delegate { task: SHAPED_DELEGATE, fate: jig_client::shapes::Fate::Move }],
        reason,
    )
    .expect("close offered");
    story.send(SHAPED_CLOSE, Ask::Jig(Box::new(ask)));
    assert!(story.run_until(|s| !s.answered(SHAPED_CLOSE).is_empty(), PROOF_WAIT));
    assert_eq!(
        story.answered(SHAPED_CLOSE),
        vec![Outcome::Answered(Answered::Jig(contract::Answer::Done(contract::Outcome::Owner(
            contract::OwnerOutcome::Compound {
                project: PROJECT_NUMBER,
                operations: 2,
                first_message: CLOSE_FIRST_MESSAGE
            }
        ))))]
    );
    grace
}

const WORDS: Key = Key { asker: Asker::JIG, number: 91 };
const THREAD_TASK: Handle = Handle { asker: Asker::JIG, number: 31 };
const THREAD_ACTIVITY: Handle = Handle { asker: Asker::JIG, number: 32 };
const READ_MESSAGE: u64 = 1;
const COMMITTED_TURN_POSITION: u64 = 4;
#[test]
fn a_person_s_words_read_as_being_sent_then_sent_then_read() {
    use jig_client::thread::{Thread, Words};
    let mut story = Story::reading_chat(fixtures::SEED);
    let task = story.running_task();
    let mut thread = Thread::new(task, &jig_client::thread::Limits { lines: 4, bytes: 65536, text: 32 });
    let words = contract::Text::new(Box::from(b"Please continue".as_slice()), 32).expect("bounded words");
    assert!(thread.sending(WORDS, fixtures::FIRST_OWNER, words.clone(), 0));
    story.send(WORDS, Ask::Jig(Box::new(contract::Ask::Message { project: PROJECT_NUMBER, task, words })));
    assert_eq!(thread.words.rows.get(0).expect("submitted words").state, Words::BeingSent);
    assert!(story.run_until(|s| !s.answered(WORDS).is_empty(), PROOF_WAIT));
    let answers = story.answered(WORDS);
    let [Outcome::Answered(Answered::Jig(answer))] = answers.as_slice() else {
        panic!("durable words: {answers:?}");
    };
    thread.answered(WORDS, answer);
    assert_eq!(thread.words.rows.get(0).expect("durable words").state, Words::Sent { message: READ_MESSAGE });
    story.watch(THREAD_TASK, Watchable::Jig(contract::Subject::Task { task }));
    assert!(story.run_until(|s| s.thread_activity(THREAD_TASK, task) >= COMMITTED_TURN_POSITION, PROOF_WAIT));
    let Some(Latest::Jig(contract::Delivery::Task(document))) =
        story.client.document(THREAD_TASK).expect("task watch").latest
    else {
        panic!("task document");
    };
    story.read(THREAD_ACTIVITY, Readable::Jig(thread.document(document).expect("new committed activity")));
    assert!(story.run_until(|s| !s.pages(THREAD_ACTIVITY).is_empty(), PROOF_WAIT));
    let pages = story.pages(THREAD_ACTIVITY);
    let [Readout::Jig(contract::ReadResult::Value(contract::ReadValue::Activity(page)))] = pages.as_slice() else {
        panic!("activity page: {pages:?}");
    };
    assert_eq!(thread.page((**page).clone()), None);
    assert_eq!(thread.words.rows.get(0).expect("read words").state, Words::Read { message: READ_MESSAGE });
}
