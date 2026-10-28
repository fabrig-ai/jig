//! Only supplied Deployment availability renders the two checked diagnostic captions.
use super::*;

fn value(rows: List<OutboxEntry>) -> ClientDocument {
    ClientDocument::Deployment {
        outbox: rows,
        subject_limit: u32::MAX,
        staged_limit: u32::MAX - 1,
        request_budget: DiagnosticAvailability::NotKept,
        system_last_response: DiagnosticAvailability::NotKept,
    }
}

#[test]
fn deployment_view_preserves_waiting_uncertain_and_all_outbox_phases_with_honest_capacity_captions() {
    let words = words::english(64).unwrap();
    for (phase, caption) in [
        (EffectPhase::Kept, b"Effect kept".as_slice()),
        (EffectPhase::Sent, b"Effect sent"),
        (EffectPhase::Uncertain, b"Effect uncertain"),
        (EffectPhase::Retry, b"Effect retry"),
        (EffectPhase::Repeat, b"Effect repeat"),
        (EffectPhase::Held, b"Effect held"),
    ] {
        let mut row = entry();
        row.phase = phase;
        let tree = document(value(list(3, [row])), Scope::Deployment, &words);
        expect(
            &tree,
            &[
                caption,
                b"Deployment scope",
                b"Subject limit: 4294967295",
                b"Staged limit: 4294967294",
                b"System request budget: not kept by this connector",
                b"What the system last said: not kept by this connector",
                b"Entry: 18446744073709551615",
                b"Key completion: 4294967295",
                b"\\xFF\\x00",
            ],
        );
        assert!(!contains(&tree, b"Object revision:"));
        for node in tree.nodes() {
            assert!(node.binding.is_none() && node.link.is_none());
        }
    }
    let tree = document(value(list(3, [])), Scope::Deployment, &words);
    expect(
        &tree,
        &[
            b"System request budget: not kept by this connector",
            b"What the system last said: not kept by this connector",
        ],
    );
    assert!(!contains(&tree, b"Effect uncertain"));
}

#[test]
fn deployment_diagnostic_captions_select_the_checked_language_and_require_both_no_hole_shapes() {
    let normal = words::language(phrases::english::language());
    let mut selected = words::language(phrases::english::language());
    for phrase in &mut selected.phrases {
        let literal = if phrase.id == Word::SystemRequestBudgetNotKept.id() {
            Some(b"Budget du systeme non conserve".as_slice())
        } else if phrase.id == Word::LastSystemResponseNotKept.id() {
            Some(b"Derniere reponse non conservee".as_slice())
        } else {
            None
        };
        if let Some(literal) = literal {
            phrase.forms = Box::new([PhraseForm {
                class: PluralClass::Other,
                segments: Box::new([Segment::Text(copy_of(literal))]),
            }]);
        }
    }
    let count = u32::try_from(normal.phrases.len()).unwrap();
    let mut segments = 0_u32;
    let mut text_bytes = 0_u32;
    for language in [&normal, &selected] {
        for phrase in &language.phrases {
            for form in &phrase.forms {
                segments += u32::try_from(form.segments.len()).unwrap();
                for segment in &form.segments {
                    if let Segment::Text(bytes) = segment {
                        text_bytes += u32::try_from(bytes.len()).unwrap();
                    }
                }
            }
        }
    }
    let bounds = PhraseLimits { languages: 2, phrases: count, holes: 4, segments, text_bytes, hole_bytes: 64 };
    let book =
        Phrasebook::load(words::shapes(phrases::jig::shapes(64)), Box::new([normal, selected]), &bounds).unwrap();
    let selected_words = jig_client_view::words::Words::new(book, 1, 0);
    let tree = document(value(list(3, [entry()])), Scope::Deployment, &selected_words);
    expect(
        &tree,
        &[b"Budget du systeme non conserve", b"Derniere reponse non conservee", b"Subject limit: 4294967295"],
    );
    assert!(!contains(&tree, b"System request budget: not kept by this connector"));
    for word in [Word::SystemRequestBudgetNotKept, Word::LastSystemResponseNotKept] {
        assert!(word.shape().holes.is_empty());
        let mut language = words::language(phrases::english::language());
        language.phrases = language.phrases.into_vec().into_iter().filter(|phrase| phrase.id != word.id()).collect();
        let Err(_) = Phrasebook::load(words::shapes(phrases::jig::shapes(64)), Box::new([language]), &bounds) else {
            panic!("missing diagnostic caption is rejected")
        };
        let mut language = words::language(phrases::english::language());
        let phrase = language.phrases.iter_mut().find(|phrase| phrase.id == word.id()).unwrap();
        phrase.forms = Box::new([PhraseForm {
            class: PluralClass::Other,
            segments: Box::new([Segment::Hole { index: 0, kind: HoleKind::Number }]),
        }]);
        let Err(_) = Phrasebook::load(words::shapes(phrases::jig::shapes(64)), Box::new([language]), &bounds) else {
            panic!("a diagnostic hole is rejected")
        };
    }
}

#[test]
fn deployment_diagnostic_nodes_fit_the_existing_whole_envelope_with_empty_and_full_reserved_rows() {
    let words = words::english(64).unwrap();
    for rows in [list(3, []), list(3, [entry(), entry(), entry()])] {
        let tree = document(value(rows), Scope::Deployment, &words);
        let bounds = render::requirements(&limits(), &words).unwrap();
        let text_per_node = usize::try_from(bounds.text_per_node).unwrap();
        assert!(tree.len() <= bounds.nodes);
        for node in tree.nodes() {
            if let Some(name) = &node.name {
                assert!(name.len() <= text_per_node);
            }
            for run in &node.text {
                assert!(run.text.len() <= text_per_node);
            }
        }
    }
}
