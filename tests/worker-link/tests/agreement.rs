#[path = "../src/joined.rs"]
pub mod joined;
mod agreement {
    use super::joined::{Joined, application, credential, form, received, sent, values};
    use jig_domain_host as host;
    use jig_protocol_engine_workers as engine;
    use jig_protocol_worker_link::Request;
    use jig_protocol_worker_wire::Part;
    use jig_worker_link_world::fixtures::{self, ATTEMPT_TWO, RUN_SEVEN};
    use skein_lib::{Duration, Token};
    fn world() -> Joined {
        let mut world = Joined::new(&fixtures::joined_limits(), 3);
        world.run();
        world.acknowledge(false);
        world
    }
    fn up(world: &Joined) -> (&host::LinkUp, &[Part]) {
        let engine::Request::Up { up, parts, .. } = world.engine_root.last().expect("record") else { panic!("up") };
        (up, parts)
    }
    fn down(world: &Joined) -> (&host::LinkDown, &[Part]) {
        let Request::Down { down, parts } = world.worker_root.last().expect("record") else { panic!("down") };
        (down, parts)
    }
    fn cross_up(record: host::LinkUp, expected: &host::LinkUp) {
        let mut world = world();
        world.worker_up(record, Box::new([]));
        world.run();
        assert_eq!(up(&world).0, expected);
        assert!(up(&world).1.is_empty());
        world.finish();
    }
    fn cross_down(record: host::LinkDown, expected: &host::LinkDown) {
        let mut world = world();
        world.engine_down(record, Box::new([]), Box::new([]));
        world.run();
        assert_eq!(down(&world).0, expected);
        assert!(down(&world).1.is_empty());
        world.finish();
    }
    #[test]
    fn a_hello_crosses_unchanged() {
        let mut world = world();
        let engine::Request::Hello { hello, .. } = &world.engine_root[0] else { panic!("hello") };
        assert_eq!(hello, &fixtures::hello());
        world.finish();
    }
    #[test]
    fn a_turn_crosses_unchanged() {
        cross_up(fixtures::turn(), &fixtures::turn());
    }
    #[test]
    fn a_call_crosses_unchanged() {
        cross_up(fixtures::call(&[0, 2, 7]), &fixtures::call(&[0, 2, 7]));
    }
    #[test]
    fn an_answer_crosses_unchanged() {
        let mut world = world();
        world.worker_up(fixtures::answer(), fixtures::work_parts());
        world.run();
        let (received, parts) = up(&world);
        let mut expected = fixtures::answer();
        let host::LinkUp::Answer { answer: host::Answer { ending: host::Ending::Ended { work, .. }, .. }, .. } =
            &mut expected
        else {
            panic!("ended")
        };
        work.left = Some(Token::new(1));
        work.saved = Some(Token::new(2));
        assert_eq!(received, &expected);
        assert_eq!(
            parts,
            [
                Part { token: Token::new(1), bytes: b"left".as_slice().into() },
                Part { token: Token::new(2), bytes: b"saved".as_slice().into() }
            ]
        );
        world.finish();
    }
    fn fact() -> host::LinkUp {
        host::LinkUp::Activity {
            run: RUN_SEVEN,
            attempt: ATTEMPT_TWO,
            activity: host::Activity::Long { operation: host::Operation::Command, span: Duration::from_secs(2) },
        }
    }
    #[test]
    fn a_fact_crosses_unchanged() {
        cross_up(fact(), &fact());
    }
    #[test]
    fn every_piece_kind_crosses_unchanged_with_a_full_width_offset() {
        for kind in [
            host::LiveKind::Text,
            host::LiveKind::Reasoning,
            host::LiveKind::Input { call: vec![b'i'; 256].into_boxed_slice(), tool: vec![b't'; 64].into_boxed_slice() },
        ] {
            let record = host::LinkUp::Piece {
                run: RUN_SEVEN,
                attempt: ATTEMPT_TWO,
                piece: host::Piece { turn: 17, retry: 23, kind, offset: u64::MAX, bytes: Box::from([0xff, 0, 0x80]) },
            };
            let host::LinkUp::Piece { run, attempt, piece } = &record else { panic!("piece") };
            let copy = host::LinkUp::Piece { run: *run, attempt: *attempt, piece: piece.clone() };
            cross_up(copy, &record);
        }
    }
    #[test]
    fn each_live_ask_crosses_both_translations_unchanged() {
        for ask in [
            host::LiveAsk::NONE,
            host::LiveAsk { text: true, reasoning: false, input: false },
            host::LiveAsk { text: false, reasoning: true, input: false },
            host::LiveAsk { text: false, reasoning: false, input: true },
            host::LiveAsk { text: true, reasoning: true, input: true },
        ] {
            let mut world = world();
            let mut assignment = fixtures::assignment();
            assignment.assignment.live = ask;
            world.engine_down(host::LinkDown::Assign { assignment }, fixtures::assignment_parts(), values());
            world.run();
            let host::LinkDown::Assign { assignment } = down(&world).0 else { panic!("assignment") };
            assert_eq!(assignment.assignment.live, ask);
            world.finish();
        }
    }
    #[test]
    fn start_messages_keep_the_finish_fact_across_both_codecs() {
        for finish_requested in [false, true] {
            let mut world = world();
            let mut assignment = fixtures::assignment();
            assignment.messages[0].finish_requested = finish_requested;
            world.engine_down(host::LinkDown::Assign { assignment }, fixtures::assignment_parts(), values());
            world.run();
            let host::LinkDown::Assign { assignment } = down(&world).0 else { panic!("assignment") };
            assert_eq!(assignment.messages[0].finish_requested, finish_requested);
            assert_eq!(assignment.messages[0].words.as_ref(), b"words");
            world.finish();
        }
    }
    fn unauthorized() -> host::LinkUp {
        host::LinkUp::Unauthorized { run: RUN_SEVEN, attempt: ATTEMPT_TWO, account: 1, generation: 3 }
    }
    #[test]
    fn an_unauthorized_notice_crosses_unchanged() {
        cross_up(unauthorized(), &unauthorized());
    }
    fn limited(reset: Option<Duration>) -> host::LinkUp {
        host::LinkUp::Limited { run: RUN_SEVEN, attempt: ATTEMPT_TWO, account: 1, reset }
    }
    #[test]
    fn a_limited_notice_crosses_unchanged_with_its_reset_or_none() {
        cross_up(limited(Some(Duration::from_secs(60))), &limited(Some(Duration::from_secs(60))));
        cross_up(limited(None), &limited(None));
    }
    #[test]
    fn an_account_refused_notice_crosses_unchanged() {
        cross_up(fixtures::account_refused(), &fixtures::account_refused());
    }
    #[test]
    fn an_assignment_crosses_unchanged() {
        let mut world = world();
        world.engine_down(
            host::LinkDown::Assign { assignment: fixtures::assignment() },
            fixtures::assignment_parts(),
            values(),
        );
        world.run();
        let mut expected = fixtures::assignment();
        expected.assignment.workspace.as_mut().expect("workspace").items = Token::new(1);
        assert_eq!(down(&world).0, &host::LinkDown::Assign { assignment: expected });
        assert_eq!(down(&world).1, [Part { token: Token::new(1), bytes: b"items".as_slice().into() }]);
        world.finish();
    }
    #[test]
    fn a_grant_crosses_as_a_name_and_a_value() {
        for sent_value in [credential(engine::Scheme::Bearer), credential(engine::Scheme::Key), application()] {
            let (form, expected) = (form(&sent_value), sent(&sent_value));
            let mut world = world();
            world.engine_down(
                host::LinkDown::Grant { run: RUN_SEVEN, attempt: ATTEMPT_TWO, grant: fixtures::grant(form) },
                Box::new([]),
                Box::new([Some(sent_value)]),
            );
            world.run();
            assert_eq!(
                down(&world).0,
                &host::LinkDown::Grant { run: RUN_SEVEN, attempt: ATTEMPT_TWO, grant: fixtures::grant(form) }
            );
            let Request::Value { account, generation, expires, value } = &world.values[0] else { panic!("value") };
            assert_eq!(
                (*account, *generation, *expires),
                (1, 3, skein_lib::Time::ZERO.saturating_add(Duration::from_secs(60)))
            );
            assert_eq!(received(value), expected);
            assert_eq!(world.worker_order, ["value", "down"]);
            world.finish();
        }
    }
    fn message(finish_requested: bool) -> host::LinkDown {
        host::LinkDown::Message {
            run: RUN_SEVEN,
            attempt: ATTEMPT_TWO,
            name: Token::new(6),
            sender: b"sender".as_slice().into(),
            words: b"later".as_slice().into(),
            finish_requested,
        }
    }
    #[test]
    fn a_message_crosses_unchanged() {
        for finish_requested in [false, true] {
            cross_down(message(finish_requested), &message(finish_requested));
        }
    }
    #[test]
    fn a_relayed_answer_crosses_unchanged() {
        let mut world = world();
        world.worker_up(fixtures::call(b"call"), Box::new([]));
        world.run();
        world.engine_down(fixtures::relay_answer(b"call"), Box::new([]), Box::new([]));
        world.run();
        assert_eq!(down(&world).0, &fixtures::relay_answer(b"call"));
        world.finish();
    }
    fn kept() -> host::LinkDown {
        host::LinkDown::TurnKept { run: RUN_SEVEN, attempt: ATTEMPT_TWO, turn: 1 }
    }
    #[test]
    fn a_turn_kept_crosses_unchanged() {
        cross_down(kept(), &kept());
    }
    fn acknowledged() -> host::LinkDown {
        host::LinkDown::Acknowledge { run: RUN_SEVEN, attempt: ATTEMPT_TWO }
    }
    #[test]
    fn an_acknowledgement_crosses_unchanged() {
        cross_down(acknowledged(), &acknowledged());
    }
    #[test]
    fn a_park_crosses_unchanged() {
        cross_down(
            host::LinkDown::Park { run: RUN_SEVEN, attempt: ATTEMPT_TWO },
            &host::LinkDown::Park { run: RUN_SEVEN, attempt: ATTEMPT_TWO },
        );
    }
    fn cancel() -> host::LinkDown {
        host::LinkDown::Cancel { run: RUN_SEVEN, attempt: ATTEMPT_TWO }
    }
    #[test]
    fn a_cancel_crosses_unchanged() {
        cross_down(cancel(), &cancel());
    }
}
