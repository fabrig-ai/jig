use super::*;
fn watching(root: &Domain) -> Token {
    // These caption tests address the first owner handle directly. Transport
    // watch tokens belong to the client; the owner's first slab slot is zero.
    let token = Token::new(0);
    assert!(root.client().document(jig_client::Handle { asker: jig_client::Asker(1), number: token.raw() }).is_some());
    token
}
#[test]
fn denied_and_ended_family_values_have_distinct_public_captions_without_mutation_bindings() {
    for (end, caption) in [
        (first::WatchEnd::NotAllowed, b"Family object is not allowed".as_slice()),
        (first::WatchEnd::SubjectEnded, b"Family subject ended".as_slice()),
    ] {
        let mut root = ready();
        let rows = drive(
            &mut root,
            Event::Went { place: Place::Address(Address::First(first::Address { project: 3, item: 5 })) },
        );
        let watch = watching(&root);
        let document = family::Document::Item(family::Item {
            project: 3,
            key: 5,
            revision: Revision { value: u64::MAX },
            state: family::ItemState::Ready,
            value: u64::MAX,
            task: 9,
            second: None,
            proposals: list([]),
            touchable: true,
        });
        drive(
            &mut root,
            Event::First(first::Event::Read {
                read: family_read(&rows),
                result: first::ReadResult::Value(document.clone()),
            }),
        );
        let mut view = view();
        patches(&mut view, &root);
        assert!(
            view.tree().nodes().iter().any(|n| n.name.as_deref() == Some(b"Touch".as_slice()) && n.binding.is_some())
        );
        drive(&mut root, Event::First(first::Event::Ended { watch, end }));
        patches(&mut view, &root);
        assert!(contains(&view, caption));
        assert!(contains(&view, b"18446744073709551615"));
        assert!(!contains(&view, b"Gone"));
        assert!(!view.tree().nodes().iter().any(|n| (n.name.as_deref() == Some(b"Touch".as_slice())
            || n.name.as_deref() == Some(b"Apply parameters".as_slice())
            || n.name.as_deref() == Some(b"Parameters".as_slice()))
            && n.binding.is_some()));
        assert_eq!(root.first().object(root.first().page().unwrap()).unwrap().value, first::Value::Known(document));
        assert!(patches(&mut view, &root).is_empty());
        assert!(view.tree().len() <= view.limits().tree.nodes);
    }
}
#[test]
fn every_family_departure_fits_the_public_view_and_checked_selected_words() {
    let endings = [
        (first::WatchEnd::Closed, None),
        (first::WatchEnd::Dropped, Some(b"Retrying".as_slice())),
        (first::WatchEnd::Unknown, Some(b"Unknown family object".as_slice())),
        (first::WatchEnd::Unsupported, Some(b"Cannot show this part".as_slice())),
        (first::WatchEnd::SignedOut, None),
        (first::WatchEnd::NotAllowed, Some(b"Family object is not allowed".as_slice())),
        (first::WatchEnd::SubjectEnded, Some(b"Family subject ended".as_slice())),
        (first::WatchEnd::Stopping, Some(b"Engine stopping; retrying family watch".as_slice())),
        (first::WatchEnd::Busy, Some(b"Family watch busy; retrying".as_slice())),
    ];
    let family = family_words();
    for (end, caption) in endings {
        let mut root = ready();
        let rows = drive(
            &mut root,
            Event::Went { place: Place::Address(Address::First(first::Address { project: 3, item: 5 })) },
        );
        let watch = watching(&root);
        let read = family_read(&rows);
        let document = family::Document::Item(family::Item {
            project: 3,
            key: 5,
            revision: Revision { value: u64::MAX },
            state: family::ItemState::Ready,
            value: u64::MAX,
            task: 9,
            second: None,
            proposals: list([
                ProposalKey { proposer: Proposer::Task(9), number: 1 },
                ProposalKey { proposer: Proposer::Task(9), number: 2 },
            ]),
            touchable: true,
        });
        drive(&mut root, Event::First(first::Event::Read { read, result: first::ReadResult::Value(document) }));
        drive(&mut root, Event::First(first::Event::Ended { watch, end }));
        let mut view = view();
        patches(&mut view, &root);
        if let Some(caption) = caption {
            assert!(contains(&view, caption), "{end:?}");
        }
        assert!(view.tree().len() <= view.limits().tree.nodes);

        assert_eq!(family.book.limits().phrases, 31);
        assert!(patches(&mut view, &root).is_empty());
    }
    let checked = &family;
    for (word, number) in [
        (words::Word::NotAllowed, 7800),
        (words::Word::SubjectEnded, 7801),
        (words::Word::WatchBusy, 7802),
        (words::Word::WatchStopping, 7803),
    ] {
        assert_eq!(
            word.id(),
            skein_ui::phrases::PhraseId {
                owner: (Owner::Application { number: jig_client_test_domain::FIRST }).opaque(),
                number
            }
        );
        assert!(!checked.say(word, &[]).is_empty());
        assert!(checked.book.capacity(word.id()).unwrap() <= checked.capacity().unwrap());
    }
}
#[test]
fn read_not_allowed_unknown_and_unsupported_keep_separate_public_captions() {
    for (result, caption) in [
        (first::ReadResult::NotAllowed, b"Family object is not allowed".as_slice()),
        (first::ReadResult::Unknown, b"Unknown family object".as_slice()),
        (first::ReadResult::Unsupported, b"Cannot show this part".as_slice()),
    ] {
        let mut root = ready();
        let rows = drive(
            &mut root,
            Event::Went { place: Place::Address(Address::First(first::Address { project: 3, item: 5 })) },
        );
        drive(&mut root, Event::First(first::Event::Read { read: family_read(&rows), result }));
        let mut view = view();
        patches(&mut view, &root);
        assert!(contains(&view, caption));
        assert!(!contains(&view, b"Touched"));
        assert!(!contains(&view, b"Gone"));
        assert!(
            !view.tree().nodes().iter().any(|n| n.name.as_deref() == Some(b"Touch".as_slice()) && n.binding.is_some())
        );
    }
}
fn selected_words() -> words::Words {
    use skein_ui::phrases::{
        Form, HoleKind, HoleShape, Language, Phrase, PhraseLimits, PhraseShape, Phrasebook, PluralClass, PluralRule,
        Segment, SpanUnit, WallFormat,
    };
    use words::Word as W;
    let all = [
        W::Navigation,
        W::Family,
        W::Item,
        W::Loading,
        W::Proposal,
        W::ReviewProposal,
        W::Unknown,
        W::Unshowable,
        W::Value,
        W::Drawing,
        W::Ready,
        W::Held,
        W::Ended,
        W::Touch,
        W::Parameters,
        W::Apply,
        W::Deciding,
        W::Retrying,
        W::Parked,
        W::Touched,
        W::Moved,
        W::Gone,
        W::Connector,
        W::Resource,
        W::Kind,
        W::Part,
        W::OutboxEntry,
        W::NotAllowed,
        W::SubjectEnded,
        W::WatchBusy,
        W::WatchStopping,
    ];
    let mut shapes = Vec::new();
    let mut languages = Vec::new();
    for selected in [false, true] {
        let mut phrases = Vec::new();
        for word in all {
            let hole = match word {
                W::Item | W::Value | W::Touched | W::Moved | W::Connector | W::Kind | W::Part | W::OutboxEntry => {
                    Some(HoleKind::Number)
                }
                W::Resource => Some(HoleKind::Name),
                _ => None,
            };
            let literal = if selected {
                match word {
                    W::NotAllowed => b"Chosen denial".as_slice(),
                    W::SubjectEnded => b"Chosen departure".as_slice(),
                    W::WatchBusy => b"Chosen busy recovery".as_slice(),
                    W::WatchStopping => b"Chosen stopping recovery".as_slice(),
                    _ => b"Chosen family".as_slice(),
                }
            } else {
                b"Other language".as_slice()
            };
            let mut segments = Vec::new();
            segments.push(Segment::Text(literal.into()));
            let mut holes = Vec::new();
            if let Some(kind) = hole {
                segments.push(Segment::Hole { index: 0, kind });
                holes.push(HoleShape { kind, bytes: if kind == HoleKind::Name { 32 } else { 0 } });
            }
            if !selected {
                shapes.push(PhraseShape { id: word.id(), holes: holes.into_boxed_slice(), plural: None });
            }
            phrases.push(Phrase {
                id: word.id(),
                forms: Box::new([Form { class: PluralClass::Other, segments: segments.into_boxed_slice() }]),
            });
        }
        languages.push(Language {
            plural: PluralRule::English,
            span_unit: SpanUnit::Seconds,
            wall: WallFormat::Seconds,
            phrases: phrases.into_boxed_slice(),
        });
    }
    words::Words {
        book: Phrasebook::load(
            shapes.into_boxed_slice(),
            languages.into_boxed_slice(),
            &PhraseLimits { languages: 2, phrases: 31, holes: 1, segments: 124, text_bytes: 4096, hole_bytes: 32 },
        )
        .unwrap(),
        language: 1,
    }
}
#[test]
fn family_terminal_captions_come_from_the_selected_checked_language_without_english_fallback() {
    for (end, caption) in [
        (first::WatchEnd::NotAllowed, b"Chosen denial".as_slice()),
        (first::WatchEnd::SubjectEnded, b"Chosen departure".as_slice()),
        (first::WatchEnd::Busy, b"Chosen busy recovery".as_slice()),
        (first::WatchEnd::Stopping, b"Chosen stopping recovery".as_slice()),
    ] {
        let mut root = ready();
        drive(&mut root, Event::Went { place: Place::Address(Address::First(first::Address { project: 3, item: 5 })) });
        let watch = watching(&root);
        drive(&mut root, Event::First(first::Event::Ended { watch, end }));
        let jig = words();
        let family = selected_words();
        let bounds = Limits::for_world(&limits(), jig, &family, 1024, 256).unwrap();
        let mut view = View::new(&bounds, &limits(), jig, &family, first::Address { project: 3, item: 5 });
        patches(&mut view, &root);
        assert!(contains(&view, caption));
        assert!(!contains(&view, b"Other language"));
        assert!(!contains(&view, b"Family object is not allowed"));
        assert!(!contains(&view, b"Family subject ended"));
        assert!(!contains(&view, b"Engine stopping; retrying family watch"));
        assert!(view.tree().len() <= view.limits().tree.nodes);
    }
}
