mod records {
    use jig_engine_workers_world::{
        World,
        fixtures::{self, SECRET_A, WORKER_A},
    };
    use jig_protocol_engine_workers::Request;
    use skein_lib::Token;
    fn world() -> World {
        World::new(&fixtures::limits(), &fixtures::workers(), 1)
    }
    fn reporting() -> skein_fake_channel::ScriptedPeer {
        fixtures::reporting_worker(WORKER_A, SECRET_A)
    }
    #[derive(Debug, PartialEq, Eq)]
    enum Record {
        Hello,
        Turn,
        Call,
        Answer,
        Activity,
        Piece,
        Unauthorized,
        Limited,
        AccountRefused,
        Lost,
        Logins,
        SignedOut,
    }
    fn record(request: &Request) -> Record {
        use jig_domain_host::LinkUp;
        match request {
            Request::Hello { .. } => Record::Hello,
            Request::Up { up, .. } => match up {
                LinkUp::Logins { .. } => Record::Logins,
                LinkUp::SignedOut { .. } => Record::SignedOut,
                LinkUp::Turn { .. } => Record::Turn,
                LinkUp::Call { .. } => Record::Call,
                LinkUp::Answer { .. } => Record::Answer,
                LinkUp::Piece { .. } => Record::Piece,
                LinkUp::Activity { .. } => Record::Activity,
                LinkUp::Unauthorized { .. } => Record::Unauthorized,
                LinkUp::Limited { .. } => Record::Limited,
                LinkUp::AccountRefused { .. } => Record::AccountRefused,
            },
            Request::Lost { .. } => Record::Lost,
            Request::Take { .. }
            | Request::TurnAway { .. }
            | Request::Lower { .. }
            | Request::Close { .. }
            | Request::Abort { .. }
            | Request::Closed => panic!("an up record"),
        }
    }
    fn observed(world: &World) -> Vec<Record> {
        world.root().iter().map(record).collect()
    }
    #[test]
    fn each_up_record_reaches_the_root_in_order() {
        let mut peer = reporting();
        for frame in fixtures::up_frames() {
            peer.play(frame);
        }
        let mut world = world();
        world.connect(peer);
        world.run();
        let records = observed(&world);
        assert_eq!(
            records,
            [
                Record::Hello,
                Record::Turn,
                Record::Call,
                Record::Answer,
                Record::Activity,
                Record::Unauthorized,
                Record::Limited,
                Record::AccountRefused
            ]
        );
    }
    #[test]
    fn a_piece_fact_reaches_the_root_as_a_piece_with_every_field() {
        use jig_domain_host as host;
        use jig_protocol_worker_wire as wire;
        let limits = wire::sizes::limits(&fixtures::SIZES).expect("codec");
        for (kind, expected) in [
            (wire::LiveKind::Text, host::LiveKind::Text),
            (wire::LiveKind::Reasoning, host::LiveKind::Reasoning),
            (
                wire::LiveKind::Input(
                    wire::InputCall::new(
                        &limits,
                        wire::InputCallParts { id: Box::from(&b"call"[..]), tool: Box::from(&b"tool"[..]) },
                    )
                    .expect("call"),
                ),
                host::LiveKind::Input { call: Box::from(&b"call"[..]), tool: Box::from(&b"tool"[..]) },
            ),
        ] {
            let piece = wire::Piece::new(
                &limits,
                wire::PieceParts { turn: 17, retry: 23, kind, offset: u64::MAX, bytes: Box::from([0xff, 0, 0x80]) },
            )
            .expect("piece");
            let fact = wire::Fact::new(
                &limits,
                wire::FactParts { task: 83, attempt: 97, value: wire::FactValue::Piece(piece) },
            )
            .expect("fact");
            let mut body = skein_lib::Writer::new(usize::try_from(fact.measure()).expect("body length"));
            fact.encode(&mut body).expect("body");
            let mut peer = reporting();
            peer.play(fixtures::frame(wire::Kind::Fact.number(), &body.finish()));
            let mut world = world();
            world.connect(peer);
            world.run();
            assert_eq!(observed(&world), [Record::Hello, Record::Piece]);
            let Request::Up { up, parts, .. } = &world.root()[1] else { panic!("piece") };
            assert!(parts.is_empty());
            assert_eq!(
                *up,
                host::LinkUp::Piece {
                    run: Token::new(83),
                    attempt: Token::new(97),
                    piece: host::Piece {
                        turn: 17,
                        retry: 23,
                        kind: expected,
                        offset: u64::MAX,
                        bytes: Box::from([0xff, 0, 0x80])
                    }
                }
            );
        }
    }
    #[test]
    fn an_undecodable_turn_body_is_refused_as_framing() {
        let mut peer = reporting();
        peer.play(fixtures::frame(0x0101, b"bad"));
        let mut world = world();
        let connection = world.connect(peer);
        world.run();
        assert_eq!(world.refusal(connection), Some((3, Box::from(b"".as_slice()))));
        let records = observed(&world);
        assert_eq!(records, [Record::Hello, Record::Lost]);
        let Request::Lost { channel } = world.root()[1] else {
            panic!("lost");
        };
        assert_eq!(channel, Token::new(1));
    }
    fn answering() -> skein_fake_channel::ScriptedPeer {
        let mut peer = reporting();
        peer.play(fixtures::answer_frame());
        peer
    }
    fn application_parts(world: &World) -> Vec<(Token, Token, Vec<u8>)> {
        let mut collected = Vec::new();
        for request in world.root() {
            if let Request::Up { channel, up: jig_domain_host::LinkUp::Answer { answer, .. }, parts } = request {
                let jig_domain_host::Ending::Ended { work, .. } = &answer.ending else {
                    panic!("the fixture ended");
                };
                assert_eq!(parts.len(), 2);
                assert_eq!(work.left, Some(parts[0].token));
                assert_eq!(work.saved, Some(parts[1].token));
                for part in parts {
                    collected.push((*channel, part.token, part.bytes.to_vec()));
                }
            }
        }
        collected
    }
    #[test]
    fn application_part_tokens_never_repeat_when_a_worker_is_replaced() {
        let mut world = world();
        for _ in 0..3 {
            world.connect(answering());
            world.run();
        }
        assert_eq!(
            application_parts(&world),
            vec![
                (Token::new(1), Token::new(1), b"left".to_vec()),
                (Token::new(1), Token::new(2), b"saved".to_vec()),
                (Token::new(2), Token::new(3), b"left".to_vec()),
                (Token::new(2), Token::new(4), b"saved".to_vec()),
                (Token::new(3), Token::new(5), b"left".to_vec()),
                (Token::new(3), Token::new(6), b"saved".to_vec()),
            ]
        );
    }
}

mod down_records {
    use jig_domain_host::LinkDown;
    use jig_engine_workers_world::{World, fixtures};
    use jig_protocol_engine_workers::{Event, Request};
    use skein_lib::Token;

    fn world() -> World {
        World::new(&fixtures::limits(), &fixtures::workers(), 1)
    }
    fn joined() -> (World, Token, Token) {
        let mut world = world();
        let connection = world.connect(fixtures::reporting_worker(fixtures::WORKER_A, fixtures::SECRET_A));
        world.run();
        let Request::Hello { channel, .. } = world.root()[0] else {
            panic!("hello first");
        };
        (world, connection, channel)
    }
    fn send(world: &mut World, channel: Token, down: LinkDown) {
        world.down(Event::Down { channel, down, parts: Box::new([]), values: Box::new([]) });
    }
    fn records() -> [LinkDown; 6] {
        let run = Token::new(7);
        let attempt = Token::new(2);
        [
            LinkDown::Message {
                run,
                attempt,
                name: Token::new(8),
                sender: Box::from(b"sender".as_slice()),
                words: Box::from(b"message".as_slice()),
                finish_requested: false,
            },
            LinkDown::Relayed { run, attempt, call: Box::from([0, 2, 7]), answer: jig_domain_host::Relayed::Busy },
            LinkDown::TurnKept { run, attempt, turn: 3 },
            LinkDown::Acknowledge { run, attempt },
            LinkDown::Cancel { run, attempt },
            LinkDown::Park { run, attempt },
        ]
    }
    #[test]
    fn each_down_record_reaches_the_worker_in_order() {
        let (mut world, connection, channel) = joined();
        world.down(fixtures::assignment_down(channel));
        for record in records() {
            send(&mut world, channel, record);
        }
        world.down(turn_busy(channel));
        world.down(Event::Down {
            channel,
            down: LinkDown::Grant {
                run: Token::new(7),
                attempt: Token::new(2),
                grant: fixtures::grant(jig_domain_host::Form::Credential(jig_domain_host::Scheme::Bearer)),
            },
            parts: Box::new([]),
            values: Box::new([fixtures::values()[0].take()]),
        });
        world.run();
        let kinds: Vec<_> = world.observed(connection).iter().map(|record| record.kind).collect();
        assert_eq!(kinds, [0x0180, 0x0181, 0x0182, 0x0183, 0x0185, 0x0186, 0x018b, 0x0184, 0x0187]);
    }
    #[test]
    fn a_replaced_channel_receives_no_later_down_record() {
        let (mut world, older, channel) = joined();
        let newer = world.connect(fixtures::reporting_worker(fixtures::WORKER_A, fixtures::SECRET_A));
        world.run();
        world.down(fixtures::assignment_down(channel));
        world.run();
        for connection in [older, newer] {
            assert!(world.observed(connection).iter().all(|record| record.kind != 0x0180));
        }
        assert_eq!(world.dropped_down(), 1);
    }
    fn turn_busy(channel: Token) -> Event {
        Event::TurnBusy { channel, run: Token::new(7), attempt: Token::new(2), turn: 3 }
    }
    fn busy(world: &mut World, channel: Token) {
        world.down(turn_busy(channel));
        world.run();
    }
    #[test]
    fn a_turn_busy_reaches_the_worker_with_its_attempt_and_turn() {
        let (mut world, connection, channel) = joined();
        busy(&mut world, channel);
        let record = world.observed(connection).iter().find(|record| record.kind == 0x0184).expect("busy crossed");
        assert_eq!(record.body.as_ref(), &[0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3]);
    }
    #[test]
    fn a_turn_busy_for_a_lost_channel_is_dropped() {
        let (mut world, connection, channel) = joined();
        world.down(Event::Gone { connection });
        busy(&mut world, channel);
        assert!(world.observed(connection).iter().all(|record| record.kind != 0x0184));
        assert_eq!(world.dropped_down(), 1);
    }
    #[test]
    fn a_turn_busy_before_the_hello_is_dropped() {
        let mut world = world();
        let connection = world.connect(fixtures::worker(fixtures::WORKER_A, fixtures::SECRET_A, fixtures::PAYLOAD));
        world.run();
        busy(&mut world, Token::new(1));
        assert!(world.observed(connection).iter().all(|record| record.kind != 0x0184));
        assert_eq!(world.dropped_down(), 1);
    }
}
