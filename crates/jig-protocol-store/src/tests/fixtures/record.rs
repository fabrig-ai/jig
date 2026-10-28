//! Complete task rows for the codec fixtures (protocol/store.md, 5.1).
use super::{task_creation, task_name, task_phase, task_proposal, task_template};

pub fn task_word(number: u64) -> jig_domain_tasks::Word {
    jig_domain_tasks::Word {
        number,
        from: jig_domain_tasks::Party::Person(31),
        kind: jig_domain_tasks::MessageKind::Words,
        words: Box::from(b"message".as_slice()),
        at: skein_lib::Wall::from_nanos(59),
        hits: 3,
        eligible: true,
    }
}
pub fn task_record() -> jig_domain_tasks::TaskRecord {
    use jig_domain_tasks as task;
    use skein_lib::{Duration, Wall};
    let new = task_creation(17);
    task::TaskRecord {
        asked_requester: false,
        placed: None,
        settings: task::SettingsRevision { task: 3, run: Some(2) },
        added: Box::new([]),
        hold_bound_since: None,
        kept_account: Some(u32::MAX - 1),
        label: 7,
        flags: task::Flags { conversation: true, ..task::Flags::PLAIN },
        last_activity: Wall::from_nanos(19),
        ended_below: task::Endings { done: 3, failed: 5, cancelled: 7 },
        question_lines: task_question_lines(),
        hold: 13,
        proposal_rows: Box::new([task::ProposalRow { proposal: 43, revision: 3 }]),
        instructions: Some(Box::from(b"instructions".as_slice())),
        llm: new.llm.clone(),
        refused: Box::new([5]),
        grace: Some(task_grace()),
        revisions: task::Revisions { hold: 3, spec: 5, person: 7 },
        created_at: Wall::from_nanos(11),
        changed: Wall::from_nanos(13),
        phase_since: Wall::from_nanos(17),
        run_since: Some(Wall::from_nanos(19)),
        retry_notice: Some(task::RetryNotice::Failed { class: task::Class::Agent, count: 1, allowance: 3 }),
        ended_at: Some(Wall::from_nanos(23)),
        revision: 3,
        milestone: 13,
        activity: 29,
        due_reason: task::Why::Message { message: 5 },
        narrowing: true,
        result_position: 31,
        last_reply: 37,
        replies: reply_hints(),
        escalation: task::Escalation::Waiting {
            revision: 3,
            holder: task::EscalationHolder::Role { project: 3, role: 7 },
            entry: 43,
            since: Wall::from_nanos(47),
        },
        proposal: Some(Box::new(task_proposal())),
        result_proposal: true,
        number: new.number,
        project: new.project,
        requester: task::Party::Person(31),
        run_spent: 7,
        run_reserved: 11,
        root: 13,
        depth: 1,
        executor: new.executor,
        taken_by: Some(31),
        person_answer: Some(task::PersonAnswer { by: 31, at: Wall::from_nanos(53) }),
        person_wait: Some(task::PersonWait {
            since: Wall::from_nanos(59),
            stall: Duration::from_nanos(3),
            deadline: Wall::from_nanos(62),
            notice: Some(task::PersonStallNotice { message: 67, at: Wall::from_nanos(71), activity: 73 }),
        }),
        recurring: Some(Box::new(task::RecurringState {
            template: task_template(),
            last_batch: Box::new([79]),
            last_period: 83,
            pending_period: Some(89),
        })),
        tracked: new.tracked,
        spec: new.spec,
        contract: new.contract,
        authority: new.authority,
        numbers: new.numbers,
        funder: new.funder,
        dependencies: new.dependencies,
        waiting_on: Box::new([17]),
        holdings: new.holdings,
        holds_taken: true,
        hold_wait_since: Some(Wall::from_nanos(97)),
        delegates: Box::new([101]),
        result_followups: Box::new([103]),
        references: Box::new([107]),
        questions: Box::new([task_credit()]),
        subscriptions: Box::new([task::Subscription {
            number: 109,
            kind: task::SubscriptionKind::Topic { connector: 2, topic: 113 },
        }]),
        wake: new.wake,
        turn: 5,
        last_message: 5,
        inbox: Box::new([task_word(5)]),
        saved: Box::new([task::SavedResource { connector: 2, path: task_name().path }]),
        ever_turned: true,
        made: 2,
        attempt: 3,
        last_answer: Some(1),
        tries: task::Tries { transient: 2, permanent: 3, run: 5, agent: 7, lost: 11, invalid: 13 },
        invalid_result: Some(task::InvalidResult::Followups),
        refusals: 127,
        phase: task_phase(),
    }
}

fn reply_hints() -> Box<[jig_domain_tasks::ReplyHint]> {
    Box::new([jig_domain_tasks::ReplyHint {
        number: 37,
        activity: 29,
        attempt: 3,
        turn: 5,
        at: skein_lib::Wall::from_nanos(41),
    }])
}

fn task_grace() -> jig_domain_tasks::Grace {
    jig_domain_tasks::Grace {
        by: jig_domain_tasks::Party::Person(31),
        until: skein_lib::Wall::from_nanos(61),
        reason: b"finish".as_slice().into(),
        message: 1,
    }
}

fn task_question_lines() -> Box<[jig_domain_tasks::QuestionLine]> {
    Box::new([jig_domain_tasks::QuestionLine {
        message: 5,
        line: 7,
        answer: Some(jig_domain_tasks::AnswerLine { task: 19, position: 11 }),
    }])
}

fn task_credit() -> jig_domain_tasks::QuestionCredit {
    use jig_domain_tasks::{Party, QuestionCredit};
    QuestionCredit { number: 5, answerer: Party::Task(19), asked: Party::Task(19), line: 7 }
}
