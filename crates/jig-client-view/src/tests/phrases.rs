use super::cards::CLIENT;
use crate::words::Words;
use skein_lib::{List, bytes::zeroed};
use skein_ui::phrases::{
    Form, Language, Phrase, PhraseId, PhraseLimits, PhraseShape, Phrasebook, PluralClass, PluralRule, Segment,
    SpanUnit, WallFormat,
};

#[test]
fn a_partial_book_has_no_jig_capacity() {
    let book = partial_book();
    assert_eq!(crate::phrases::jig_capacity(&book), None, "partial jig words do not compose a complete jig page");
}

fn partial_book() -> Phrasebook {
    let language = Language {
        plural: PluralRule::English,
        span_unit: SpanUnit::Seconds,
        wall: WallFormat::Clock,
        phrases: Box::new([]),
    };
    let limits = PhraseLimits { languages: 1, phrases: 0, holes: 0, segments: 0, text_bytes: 0, hole_bytes: 0 };
    Phrasebook::load(Box::new([]), Box::new([language]), &limits).unwrap()
}

#[test]
fn english_loads_beside_the_reference_ui_s_phrases_in_one_phrasebook() {
    let book = crate::phrases::english::english(32).expect("both owners load together");
    assert_eq!(
        book.say(0, jig_client::words::Word::Phase(jig_client::contract::Phase::Held).id(), &[]).unwrap().text.as_ref(),
        b"Held"
    );
    assert_eq!(book.say(0, crate::phrases::jig::JigPhrase::Inbox.id(), &[]).unwrap().text.as_ref(), b"Inbox");
    assert!(crate::phrases::jig_capacity(&book).is_some());
}

#[test]
fn replacing_words_recomputes_capacity_without_changing_a_clone() {
    let mut words = Words::new(crate::phrases::english::english(CLIENT.text).unwrap(), 0, 60);
    let original = words.clone();
    assert_eq!(words.jig_capacity(), crate::phrases::jig_capacity(words.book()));
    words.set_book(crate::phrases::english::english(CLIENT.text * 2).unwrap());
    assert_eq!(words.jig_capacity(), crate::phrases::jig_capacity(words.book()));
    assert_ne!(words.jig_capacity(), original.jig_capacity());
    assert_eq!(original.jig_capacity(), crate::phrases::jig_capacity(original.book()));
    words.set_book(partial_book());
    assert_eq!(words.jig_capacity(), crate::phrases::jig_capacity(words.book()));
    assert_eq!(words.jig_capacity(), None);
    assert!(original.jig_capacity().is_some());
}

#[test]
fn words_capacity_covers_unselected_languages_and_ignores_other_owners() {
    let base = crate::phrases::english::english(CLIENT.text).unwrap();
    let longer = crate::phrases::jig_capacity(&base).unwrap() + 1;
    let unrelated = PhraseId { owner: crate::Owner::Application { number: 0 }.opaque(), number: 0 };
    let mut shapes = List::with_capacity(crate::owner_words::phrase_count() + 1);
    for shape in crate::phrases::jig::shapes(CLIENT.text) {
        shapes.push(shape).unwrap();
    }
    shapes.push(PhraseShape { id: unrelated, holes: Box::new([]), plural: None }).unwrap();
    let mut first = crate::phrases::english::language();
    let mut phrases = List::with_capacity(crate::owner_words::phrase_count() + 1);
    for phrase in first.phrases {
        phrases.push(phrase).unwrap();
    }
    phrases
        .push(Phrase {
            id: unrelated,
            forms: Box::new([Form {
                class: PluralClass::Other,
                segments: Box::new([Segment::Text(translation(longer * 2, b'u'))]),
            }]),
        })
        .unwrap();
    first.phrases = phrases.into_boxed();
    let mut second = first.clone();
    for phrase in &mut second.phrases {
        if phrase.id == crate::phrases::jig::JigPhrase::Inbox.id() {
            phrase.forms[0].segments = Box::new([Segment::Text(translation(longer, b's'))]);
        }
    }
    let book = Phrasebook::load(
        shapes.into_boxed(),
        Box::new([first, second]),
        &PhraseLimits {
            languages: 2,
            phrases: crate::owner_words::phrase_count() + 1,
            holes: 4,
            segments: 6000,
            text_bytes: 100_000,
            hole_bytes: CLIENT.text,
        },
    )
    .unwrap();
    let mut words = Words::new(book, 0, 0);
    for language in [0, 1] {
        words.set_language(language);
        assert_eq!(words.language(), language);
        assert_eq!(words.jig_capacity(), crate::phrases::jig_capacity(words.book()), "language {language}");
        assert_eq!(words.jig_capacity(), Some(longer), "language {language}");
        assert_eq!(words.book().capacity(unrelated), Some(longer * 2));
    }
}

fn translation(bytes: u32, letter: u8) -> Box<[u8]> {
    let mut text = zeroed(usize::try_from(bytes).unwrap());
    text.fill(letter);
    text
}

#[test]
fn full_answers_phrase_names_the_count_and_room_time() {
    use skein_ui::phrases::Hole;
    const HELD: u64 = 3; // three occupied answer places
    const ROOM: skein_lib::Wall = skein_lib::Wall::from_nanos(3_720_000_000_000); // 01:02 in the clock vocabulary
    let book = crate::phrases::english::english(CLIENT.text).expect("the vocabulary declarations fit");
    let said = book
        .say(0, crate::phrases::jig::JigPhrase::AnswersFull.id(), &[Hole::Number(HELD), Hole::Wall(ROOM)])
        .expect("typed full facts");
    assert_eq!(said.text.as_ref(), b"The engine's answer table is full: 3 places held; room again at 01:02");
}

#[test]
fn a_too_linked_read_names_its_join_bound() {
    let book = crate::phrases::english::english(CLIENT.text).expect("English words");
    let words = book
        .say(0, crate::phrases::jig::JigPhrase::TooLinked.id(), &[skein_ui::phrases::Hole::Number(4)])
        .expect("join bound");
    assert_eq!(words.text.as_ref(), b"Too linked: this needs more than 4 joins in one read.");
}
