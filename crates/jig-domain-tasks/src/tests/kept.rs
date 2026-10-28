//! Kept facts and requester authority (domain/tasks.md, 5.7 and 5.8).
use super::revisions::{Harness, LIMITS, task};
use crate::{Domain, Event, Party, Stored};
use alloc::boxed::Box;

#[test]
fn shared_kept_rule_separates_claims_from_finish_authority() {
    use crate::{End, EndedBy, Executor, TaskResult};
    let done = End::Finished { result: TaskResult::Report { words: Box::new([]) }, cancel_delegates: false };
    let failed = End::Finished { result: TaskResult::Failure { reason: Box::new([]) }, cancel_delegates: false };
    let malformed = End::Finished {
        result: TaskResult::Verdict { fields: Box::new([]), code: 0, words: Box::new([]) },
        cancel_delegates: false,
    };
    let non_finish = End::Failed(crate::Class::Invalid);
    for (dependencies, ended_by, blocks, discards, parks_done, parks_failure, parks_invalid) in [
        (&[][..], EndedBy::Result, false, false, false, false, false),
        (&[2][..], EndedBy::Result, true, false, true, false, true),
        (&[][..], EndedBy::Requester, false, true, false, false, false),
        (&[2][..], EndedBy::Requester, true, true, true, false, true),
    ] {
        let kept = crate::run::ends::pending(dependencies, ended_by);
        assert_eq!(kept.blocks_claims(), blocks);
        assert_eq!(kept.may_be_due(Executor::Agent { charter: 1 }), !blocks);
        assert_eq!(kept.discards(&done), discards);
        assert_eq!(kept.discards(&failed), discards);
        assert_eq!(kept.discards(&malformed), discards);
        assert!(!kept.discards(&non_finish));
        assert_eq!(kept.parks(&done), parks_done);
        assert_eq!(kept.parks(&failed), parks_failure);
        assert_eq!(kept.parks(&malformed), parks_invalid);
        assert!(!kept.parks(&non_finish));
    }
}

#[test]
fn loaded_requester_endings_keep_the_committed_authority() {
    let mut h = Harness::new();
    let mut open = task(1, &[]);
    open.flags.conversation = true;
    open.flags.ended_by = crate::EndedBy::Requester;
    h.make([open].into());
    h.restart();
    let original = h.domain.task(1).expect("restored open task").clone();
    assert_eq!(original.flags.ended_by, crate::EndedBy::Requester);
    for requester in [Party::Person(0), Party::Task(1), Party::Deployment { project: 1 }] {
        let mut broken = original.clone();
        broken.requester = requester;
        let mut cold = Harness::new();
        cold.domain = Domain::new(&LIMITS, 91, 1, crate::tests::charters(), Box::new([0]));
        assert!(cold.send(Event::Restore { record: Stored::Live(Box::new(broken.clone())) }).is_empty());
        assert_eq!(cold.domain.task(broken.number), Some(&broken));
    }
}
