use super::{PROOF_WAIT, STOP, Story, contract, fixtures, stop};

#[test]
fn a_stopped_task_s_hold_reads_in_jig_s_words() {
    use jig_client::words::{self, Word};
    use skein_ui::phrases::{PhraseLimits, Phrasebook};
    let mut story = Story::live(fixtures::SEED);
    let task = story.running_task();
    story.watch_chat_facts(task);
    story.send(STOP, stop(task));
    assert!(
        story.run_until(|s| s.chat_document().is_some_and(|doc| doc.chip.phase == contract::Phase::Held), PROOF_WAIT)
    );
    let document = story.chat_document().expect("held task document delivered");
    let contract::ChipPhase::Owner { phase: contract::OwnerChipPhase::Held { reason, .. }, .. } =
        document.chip.details.as_ref()
    else {
        panic!("held task's reason: {:?}", document.chip);
    };
    let limits = PhraseLimits {
        languages: 1,
        phrases: words::PHRASES,
        holes: 3,
        segments: 256,
        text_bytes: 8192,
        hole_bytes: 32,
    };
    let book = Phrasebook::load(words::phrase_shapes(32), Box::new([words::english()]), &limits)
        .expect("public English words");
    assert_eq!(
        book.say(0, Word::hold(reason).id(), &[]).expect("bounded hold phrase").text.as_ref(),
        b"Held by a placed hold"
    );
}
