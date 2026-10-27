use super::*;
use crate::address::Address;

#[test]
fn whole_wake_channel_edits_preserve_batched_rules_flags_and_separate_subscriptions() {
    for rule in
        [WakeRule::Never, WakeRule::Immediate, WakeRule::Batch { count: u32::MAX, age: Duration::from_nanos(u64::MAX) }]
    {
        for results in [ResultsWake::Never, ResultsWake::Each, ResultsWake::LastOrFailure] {
            let mut row = task_doc(&[ActKind::Amend]);
            row.wake = OwnerWakePolicy {
                markers: 0,
                words: rule,
                notices: WakeRule::Batch { count: u32::MAX, age: Duration::from_nanos(u64::MAX) },
                news: WakeRule::Immediate,
                results,
                questions: true,
                answers: false,
                timers: true,
            };
            row.subscriptions.entries = list([Subscription {
                number: u64::MAX,
                kind: SubscriptionKind::Topic { connector: u16::MAX, topic: u64::MAX },
            }]);
            let initial = row.clone();
            let (mut domain, id) = loaded(ObjectKey::Task(8), ReadValue::Task(Box::new(row)));
            drive(&mut domain, Event::Act { action: Action::Intend { object: id.token(), act: ActKind::Amend } });
            drive(&mut domain, Event::Act { action: Action::Edit { field: Purpose::Reason, text: text(b"") } });
            let words = words();
            let mut bindings = JigBindings::new(limits().bindings, CLIENT.text);
            let tree = dialog(&domain, words, &mut bindings);
            assert!(contains(&tree, b"4294967295 messages or 18446744073709551615 ns"));
            assert!(contains(&tree, b"Questions wake"));
            let action = press(&tree, b"Wakes on inbox messages", &mut bindings);
            drive(&mut domain, Event::Act { action });
            let selected = domain.confirmation().unwrap().choices.change.as_ref().unwrap();
            let mut expected = initial.wake;
            expected.words = match rule {
                WakeRule::Never => WakeRule::Immediate,
                WakeRule::Immediate | WakeRule::Batch { .. } => WakeRule::Never,
            };
            assert_eq!(selected.wake, Some(expected));
            assert!(selected.dependencies.is_none());
            assert!(selected.instructions.is_none());
            assert!(selected.authority.is_none());
            let tree = dialog(&domain, words, &mut bindings);
            drive(&mut domain, Event::Act { action: press(&tree, b"Wakes on a delegate result", &mut bindings) });
            expected.results = match results {
                ResultsWake::Never => ResultsWake::Each,
                ResultsWake::Each | ResultsWake::LastOrFailure => ResultsWake::Never,
            };
            assert_eq!(domain.confirmation().unwrap().choices.change.as_ref().unwrap().wake, Some(expected));
            let Some(crate::objects::Body::Task(supplied)) = &domain.objects().get(id).unwrap().body else {
                panic!("whole supplied task remains present");
            };
            assert_eq!(**supplied, initial);
            let out = drive(&mut domain, Event::Act { action: Action::Confirm });
            let requests: Vec<_> = out
                .iter()
                .filter_map(|request| match request {
                    Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } => {
                        match &**ask {
                            Ask::Amend { change, .. } => Some(change.as_ref()),
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].wake, Some(expected));
        }
    }
}

#[test]
fn task_overview_renders_numbered_standing_interests_independently_of_activation_choices() {
    for kind in [
        SubscriptionKind::Task { target: u64::MAX, held: true, result: false },
        SubscriptionKind::Topic { connector: u16::MAX, topic: u64::MAX },
        SubscriptionKind::Timer { at: Wall::EPOCH, period: None },
        SubscriptionKind::Timer { at: Wall::EPOCH, period: Some(Duration::from_nanos(u64::MAX)) },
    ] {
        let (mut domain, out) = super::super::work_pages::entered(Address::task(8));
        let watch = out
            .iter()
            .find_map(|request| match request {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Task { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
        let mut row = task_doc(&[]);
        row.spec.specification.words = DocumentText::complete(text(b""));
        row.wake = OwnerWakePolicy {
            markers: 0,
            words: WakeRule::Batch { count: u32::MAX, age: Duration::from_nanos(u64::MAX) },
            notices: WakeRule::Never,
            news: WakeRule::Immediate,
            results: ResultsWake::LastOrFailure,
            questions: false,
            answers: true,
            timers: false,
        };
        row.subscriptions.entries = List::with_capacity(2);
        row.subscriptions.entries.push(Subscription { number: u64::MAX, kind }).unwrap();
        row.cut = true;
        super::super::pages::deliver(&mut domain, watch, Delivery::Task(Box::new(row.clone())));
        let (tree, _) = super::super::pages::build(&domain, 3, words());
        super::super::cards_access::check(&tree);
        assert!(tree.nodes().iter().any(|node| node.name.as_deref() == Some(b"Wake policy")));
        assert!(tree.nodes().iter().any(|node| node.name.as_deref() == Some(b"Subscriptions")));
        assert!(contains(&tree, b"4294967295 messages or 18446744073709551615 ns"));
        assert!(contains(&tree, b"last delegate result or a failure"));
        assert!(contains(&tree, b"Some task lists are incomplete"));
        assert!(contains(&tree, b"18446744073709551615"));
        match kind {
            SubscriptionKind::Task { .. } => assert!(contains(&tree, b"Task 18446744073709551615")),
            SubscriptionKind::Topic { .. } => assert!(contains(&tree, b"Connector 65535, topic 18446744073709551615")),
            SubscriptionKind::Timer { period, .. } => {
                assert!(contains(&tree, b"Timer at"));
                assert_eq!(contains(&tree, b"Fires once"), period.is_none());
                assert_eq!(contains(&tree, b"Repeats every 18446744073709551615 ns"), period.is_some());
            }
        }
        assert!(tree.len() <= crate::task_nodes(&CLIENT, 0).unwrap());
        let id = domain.task_page().header.unwrap();
        let Some(crate::objects::Body::Task(supplied)) = &domain.objects().get(id).unwrap().body else {
            panic!("supplied task remains whole");
        };
        assert_eq!(**supplied, row);
        assert_eq!(supplied.subscriptions.entries.capacity(), 2);
    }
}
