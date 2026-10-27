use super::*;
const PARENT: u64 = 1;
const CHILD: u64 = 7;
const CHARTER: u32 = 1;
const ATTEMPT: u64 = 1;
const DELEGATES: [(tasks::Phase, &[u8]); 2] = [
    (
        tasks::Phase::Active(tasks::Active::Running { attempt: ATTEMPT }),
        b"delegate 7: running, spec revision 1\n".as_slice(),
    ),
    (
        tasks::Phase::Held { was: tasks::Was::Waiting, why: tasks::Hold::Budget, source: None },
        b"delegate 7: held, spec revision 1, hold revision 2\n".as_slice(),
    ),
];
#[test]
fn the_briefs_delegates_show_their_spec_and_hold_revisions() {
    let (mut core, _) = make_tree();
    for (phase, expected) in DELEGATES {
        let mut context = charters::context(PARENT, CHARTER);
        context.delegates = Box::new([tasks::DelegateState {
            task: CHILD,
            phase,
            revisions: tasks::Revisions { spec: 1, hold: 2, person: 0 },
        }]);
        core.contexts.insert(PARENT, context).expect("context room");
        let text = read_brief_part(
            &core,
            BriefTextLimits { parts: 1, read_bytes: 256, brief_bytes: 256 },
            PARENT,
            BriefPart::Delegates,
        )
        .expect("delegates section");
        assert_eq!(text.as_ref(), expected);
    }
}

fn document(path: &[u8]) -> crate::Document {
    crate::Document {
        connector: 1,
        name: Box::new([Box::from(b"resource".as_slice())]),
        path: path.into(),
        heading: None,
    }
}
fn planned_documents(core: &mut Core, limits: &Limits, task: u64) -> Box<[crate::Document]> {
    send_task(core, limits, tasks::Event::Prepare { reply_to: ReplyTo::new(Token::new(task)), task });
    core.contexts.insert(task, core.tasks.run_context(task).unwrap()).unwrap();
    core.begin_transcript(task, 0);
    let out = watch_outputs(core, limits, Event::BriefNotes { task, lines: List::with_capacity(0), more: 0 });
    for request in out {
        if let Request::Now(now) = request
            && let Now::BriefCorePlanned { sections, documents, .. } = *now
        {
            let core_count = usize::try_from(sections.len()).unwrap().checked_sub(documents.len()).unwrap();
            for section in &sections.as_slice()[..core_count] {
                let jig_domain_brief::Planned::Core { .. } = section else { panic!("core sections precede documents") };
            }
            let mut values = List::with_capacity(u32::try_from(documents.len()).unwrap());
            for (section, (token, document)) in sections.as_slice()[core_count..].iter().zip(documents) {
                let jig_domain_brief::Planned::Connector { connector, token: bound, required, .. } = section else {
                    panic!("document section")
                };
                assert_eq!(*connector, document.connector);
                assert_eq!(*bound, token);
                assert!(!required);
                values.push(document).unwrap();
            }
            return values.into_boxed();
        }
    }
    panic!("planned brief");
}
#[test]
fn a_runs_brief_plans_its_charter_versions_documents_in_order() {
    let (mut core, mut limits) = make_tree();
    limits.brief.sections = 8;
    let first = Box::new([document(b"first.md"), document(b"second.md")]);
    core.current.test_charter_mut(1).documents = first.clone();
    let mut changed = core.charter(1).unwrap().clone();
    changed.documents = Box::new([document(b"new.md")]);
    let second = changed.documents.clone();
    let items = [settings::Item {
        setting: settings::Setting::Charter(1),
        edit: settings::Edit::Set(settings::Value::Charter(Box::new(changed))),
    }];
    let plan = core.held.plan(&core.current, &items, 2, Some(1), &limits);
    let mut out = Queue::with_capacity(64);
    core.held.retain(&core.current, &plan, &mut out);
    core.held.heads.insert(1, core.current.head.clone()).unwrap();
    let _writes = settings::apply(&mut core.current, &items, 2);
    core.current.head = settings::Head { number: 2, against: 1, ..core.current.head.clone() };
    let versions = crate::charters::held_models(&core, &limits);
    send_task(&mut core, &limits, tasks::Event::Charters { newest: 2, charters: versions, efforts: Box::new([2]) });
    let new = new_row(&core, 5, tasks::Funder::Period { project: 1, period: 1 }, None);
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(5)),
            creator: tasks::Party::Person(1),
            batch: Box::new([new]),
        },
    );
    assert_eq!(core.tasks.task(1).unwrap().settings.task, 1);
    assert_eq!(core.tasks.task(5).unwrap().settings.task, 2);
    assert_eq!(planned_documents(&mut core, &limits, 1).as_ref(), first.as_ref());
    assert_eq!(planned_documents(&mut core, &limits, 5).as_ref(), second.as_ref());
}
