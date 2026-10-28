//! Faulty client and front run through real callbacks, trees, patches and people.
use jig_client_view::Owner;
use jig_client_world::{Until, harness::*};
use skein_fake_person::{Face, Find, Found, Step, TreeFace};
use skein_ui::{Act, Binding};
use skein_ui::{Key, NodeId, Patch, Role, Run, Tree, View};

use skein_lib::{Duration, Env, Queue, Time, Token, Wall};

#[global_allocator]
static HEAP: skein_world::domain::heap::Counting = skein_world::domain::heap::Counting;

struct Broken<const BAD: u8>;
struct Root {
    answered: bool,
    shown: u64,
}
enum Event {
    Start,
    Press,
    Answer,
}
struct Wire;
const KEY: Key = Key { owner: Owner::Jig.opaque(), kind: 1, a: 1, b: 1 };
const LIMITS: skein_ui::Limits = skein_ui::Limits {
    nodes: 24,
    depth: 4,
    text_per_node: 64,
    text_bytes: 1024,
    drawing_bytes: 0,
    styles_per_node: 0,
    patches: 128,
    bindings: 2,
    slots: 0,
    markdown_depth: 1,
    scene_marks: 0,
    scene_bytes: 0,
};
impl<const BAD: u8> App for Broken<BAD> {
    type Root = Root;
    type View = View;
    type Limits = ();
    type Event = Event;
    type Request = Wire;
    type Record = ();
    type Key = ();
    type Address = ();
    fn limits() {}
    fn new(_: u64) -> (Root, View) {
        (Root { answered: false, shown: 0 }, View::new(&LIMITS))
    }
    fn start((): (), _: Vec<()>, _: i32) -> Event {
        Event::Start
    }
    fn went((): ()) -> Event {
        Event::Start
    }
    fn step(root: &mut Root, _: &Env<()>, event: Event, out: &mut Queue<Wire>) {
        match event {
            Event::Start => {}
            Event::Press => {
                out.push(Wire);
            }
            Event::Answer => root.answered = true,
        }
        root.shown += 1;
    }
    fn fire(_: &mut Root, _: &Env<()>, _: &mut Queue<Wire>) {}
    fn take(_: &mut Root, _: &Env<()>, _: &mut Queue<Wire>) {}
    fn next_deadline(_: &Root) -> Option<Time> {
        None
    }
    fn build(view: &mut View, root: &Root, out: &mut Queue<Patch>) {
        let mut b = view.builder();
        b.open(Role::Page);
        b.open(Role::Banner);
        for region in [
            jig_client_view::regions::JigRegion::Link,
            jig_client_view::regions::JigRegion::Needs,
            jig_client_view::regions::JigRegion::Party,
        ] {
            b.open(Role::Region);
            b.region(region.region());
            b.close();
        }
        b.close();
        b.open(Role::Main);
        b.open(Role::Status);
        b.name(if BAD == 9 { b"Offline" } else { b"Online" });
        b.close();
        if !(BAD == 5 && root.answered) {
            b.open(Role::Item);
            b.key(KEY);
            b.open(Role::Status);
            b.name(if root.answered { if BAD == 10 { b"Leaving" } else { b"Done" } } else { b"Waiting" });
            b.close();
            b.text(Run::Plain, b"Revision 7");
            b.open(Role::Button);
            b.name(b"Choose");
            b.bind(Binding { owner: Owner::Jig.opaque(), token: Token::new(1) });
            b.close();
            b.close();
        }
        if root.answered {
            b.open(Role::Status);
            b.name(b"Sent words");
            b.text(Run::Plain, b"Words");
            b.close();
        }
        b.close();
        b.close();
        view.finish(b, root.shown, out);
    }
    fn tree(view: &View) -> &Tree {
        view.tree()
    }
    fn act(_: &mut View, _: NodeId, _: Act) -> Option<Event> {
        Some(Event::Press)
    }
    fn duty(_: Wire) -> Duty<Self> {
        Duty::Wire(Wire)
    }
    fn record_key((): &()) {}
    fn unsent(_: Wire) -> Option<Event> {
        None
    }
    fn view_limits(_: &View) -> skein_ui::Limits {
        LIMITS
    }
    fn patch_limit(_: &View) -> u32 {
        LIMITS.patches
    }
    fn worst_case((): &()) -> u64 {
        if BAD == 7 { 0 } else { 65536 }
    }
    fn sent(_: &Wire) -> Option<Sent> {
        Some(Sent {
            attempt: Token::new(1),
            key: [1; 16],
            card: Some(CardKey::of(KEY)),
            revision: Some((0, if BAD == 4 { 8 } else { 7 })),
            words: Some((9, b"Words".to_vec())),
        })
    }
    fn received(event: &Event) -> Vec<Receipt> {
        match event {
            Event::Answer => vec![Receipt::Answer { attempt: Token::new(1), final_answer: true, words: Some((9, 1)) }],
            Event::Start | Event::Press => vec![],
        }
    }
    fn card(_: &Tree, _: NodeId) -> Option<CardKey> {
        Some(CardKey::of(KEY))
    }
    fn seen(tree: &Tree) -> Vec<Seen> {
        let has = |name: &[u8]| matches!(TreeFace::new(tree).find(&Find::named(Role::Status, name)), Found::Node(_));
        let mut seen = vec![Seen::Link { live: has(b"Online") }];
        if has(b"Waiting") || has(b"Done") || has(b"Leaving") {
            seen.push(Seen::Card {
                key: CardKey::of(KEY),
                state: if has(b"Leaving") {
                    CardSeen::Left { reason: false }
                } else if has(b"Done") {
                    CardSeen::Done
                } else {
                    CardSeen::Waiting
                },
                revision: Some(7),
            });
        }
        if has(b"Sent words") {
            seen.push(Seen::Words { task: 9, message: Some(1), words: b"Words".to_vec() });
        }
        seen
    }
}
enum Progress {
    Quiet,
    Sent,
    Reported,
    Received,
    RetentionChecked,
}
struct FrontOf<const BAD: u8> {
    client: Option<ClientId>,
    progress: Progress,
    truth: Vec<Observed>,
}
impl<const BAD: u8> Front<Broken<BAD>> for FrontOf<BAD> {
    fn bind(&mut self, client: ClientId, _: u64) {
        self.client = Some(client);
        if BAD == 6 || BAD == 9 {
            self.truth.push(Observed::Reachable { live: BAD == 9 });
        }
    }
    fn wire(&mut self, _: ClientId, _: Time, _: Wall, _: Wire) {
        self.progress = Progress::Sent;
    }
    fn poll(&mut self, now: Time, _: Wall) -> Vec<(ClientId, Event)> {
        if matches!(self.progress, Progress::Received) && (BAD == 11 || BAD == 12) {
            self.progress = Progress::RetentionChecked;
            self.truth.push(Observed::Durable { commit: 2, at: now });
            if BAD == 12 {
                self.truth.push(Observed::Ended { task: 9, commit: 2 });
            }
            self.truth.push(Observed::Retained { task: 9, messages: vec![], commit: 2 });
            return vec![];
        }
        if !matches!(self.progress, Progress::Sent) {
            return vec![];
        }
        self.progress = Progress::Reported;
        if BAD != 1 {
            self.truth.push(Observed::Durable { commit: 1, at: now });
            self.truth.push(Observed::Made { party: 7, key: [1; 16], commit: 1 });
        }
        if BAD == 2 {
            self.truth.push(Observed::Made { party: 7, key: [1; 16], commit: 1 });
        }
        if BAD != 3 {
            self.truth.push(Observed::Kept {
                task: 9,
                message: 1,
                words: b"Words".to_vec(),
                commit: u64::from(BAD != 1),
            });
        }
        if BAD == 8 { vec![] } else { vec![(self.client.expect("bound"), Event::Answer)] }
    }
    fn observed(&mut self) -> Vec<Observed> {
        std::mem::take(&mut self.truth)
    }
    fn next_due(&self) -> Option<Time> {
        None
    }
    fn fault(&mut self, _: Time, _: FrontFault) {}
    fn reload(&mut self, _: ClientId, _: Time) {}
    fn received(&mut self, _: ClientId, event: &Event) {
        if matches!(event, Event::Answer) {
            self.progress = Progress::Received;
        }
    }
    fn sign_in(&mut self, _: ClientId, _: Time, _: Wall) {}
    fn sign_out(&mut self, _: ClientId, _: Time, _: Wall) {}
}
fn run<const BAD: u8>() {
    let settings = Settings {
        memory: BAD == 7,
        heartbeat: Duration::from_millis(5),
        request_within: Duration::from_millis(20),
        link_within: Duration::from_millis(5),
        ..Settings::default()
    };
    let mut w = World::<Broken<BAD>, _>::new(
        settings,
        FrontOf::<BAD> { client: None, progress: Progress::Quiet, truth: vec![] },
    );
    let c = w.client(7, ());
    if BAD != 6 && BAD != 9 {
        w.person(
            c,
            vec![
                Step::Press { find: Find::named(Role::Button, b"Choose") },
                Step::Pause { span: Duration::from_millis(10) },
            ],
        );
    }
    let result = w.run(Until::Done);
    assert_eq!(result.verdict, skein_world::domain::Verdict::Passed);
}
#[test]
fn correct_boundaries_meet_the_request_and_keep_the_words() {
    run::<0>();
}
#[test]
#[should_panic(expected = "nothing shown done before durable")]
fn an_answer_before_the_write_is_caught() {
    run::<1>();
}
#[test]
#[should_panic(expected = "made once")]
fn two_makes_for_one_party_key_are_caught() {
    run::<2>();
}
#[test]
#[should_panic(expected = "nothing sent lost")]
fn sent_words_missing_from_committed_activity_are_caught() {
    run::<3>();
}
#[test]
#[should_panic(expected = "what they saw is what they decided")]
fn an_act_with_a_different_revision_is_caught() {
    run::<4>();
}
#[test]
#[should_panic(expected = "a card leaves for a reason")]
fn a_card_removed_without_a_visible_reason_is_caught() {
    run::<5>();
}
#[test]
#[should_panic(expected = "Offline")]
fn unreachable_truth_without_offline_is_caught() {
    run::<6>();
}
#[test]
#[should_panic(expected = "bounded")]
fn a_client_exceeding_its_heap_envelope_is_caught() {
    run::<7>();
}
#[test]
#[should_panic(expected = "Request")]
fn a_request_without_a_final_answer_misses_its_deadline() {
    run::<8>();
}
#[test]
#[should_panic(expected = "Online")]
fn recovered_truth_without_an_online_view_misses_its_deadline() {
    run::<9>();
}

#[test]
fn the_test_clients_family_revision_and_durable_outcome_are_observed() {
    use jig_client_test_domain::family;
    let mut w = jig_client_world::stories::chat_ready(20);
    let subject = family::Subject { scope: family::Scope::Project(1), kind: family::Kind::Item(1) };
    let mut state = jig_client_world::scripted::Family::default();
    state.documents.insert(
        subject,
        family::Document::Item(family::Item {
            project: 1,
            key: 1,
            revision: jig_client_domain::contract::Revision { value: 7 },
            state: family::ItemState::Ready,
            value: 1,
            task: 9,
            second: None,
            proposals: jig_client_world::test::list([]),
            touchable: true,
        }),
    );
    w.front()
        .engine
        .act_application(Time::ZERO, Wall::EPOCH, jig_client_test_domain::FIRST, 1, state, vec![])
        .expect("family fixture fits");
    let c = w.client(
        7,
        jig_client_test_domain::Address::First(jig_client_test_domain::first::Address { project: 1, item: 1 }),
    );
    w.person(
        c,
        vec![
            Step::See { find: Find::named(Role::Button, b"Touch"), within: Duration::from_secs(1) },
            Step::Press { find: Find::named(Role::Button, b"Touch") },
            Step::See { find: Find::named(Role::Status, b"Touched revision 8"), within: Duration::from_secs(1) },
        ],
    );
    let result = w.run(Until::Done);
    assert_eq!(result.verdict, skein_world::domain::Verdict::Passed);
    assert!(result.judged.1 > 0);
}

#[test]
fn the_test_clients_sent_words_are_found_in_committed_activity() {
    use jig_client_domain::contract::{Activity, ActivityLine, Answer, Ask, Outcome, Read};
    use jig_client_world::stories;
    let mut w = stories::chat_ready(21);
    w.front()
        .engine
        .script(jig_client_scripted_engine::Rule {
            party: Some(7),
            ask: Ask::Message { project: 1, task: 9, words: stories::text(b"Words") },
            answer: Answer::Done(Outcome::Owner(jig_client::contract::OwnerOutcome::Said { task: 9, message: 1 })),
            changes: vec![jig_client_scripted_engine::Change::Source(jig_client_scripted_engine::Source {
                party: 7,
                read: Read::Activity { task: 9, before: None },
                rows: jig_client_scripted_engine::Rows::Activity(vec![ActivityLine {
                    position: 1,
                    at: Wall::EPOCH,
                    event: Activity::Words { by: stories::party(7), message: 1, words: stories::text(b"Words") },
                }]),
            })],
        })
        .expect("words script fits");
    let c = w.client(7, stories::chat());
    w.person(
        c,
        vec![
            Step::See { find: Find::named(Role::Button, b"Write"), within: Duration::from_secs(1) },
            Step::Press { find: Find::named(Role::Button, b"Write") },
            Step::Type { find: Find::named(Role::TextArea, b"Your answer"), words: Box::from(*b"Words") },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::See { find: Find::named(Role::Status, b"Sent"), within: Duration::from_secs(1) },
            Step::Pause { span: Duration::from_millis(30) },
        ],
    );
    let result = w.run(Until::Done);
    assert_eq!(result.verdict, skein_world::domain::Verdict::Passed);
    assert!(result.judged.1 > 0);
}

#[test]
#[should_panic(expected = "explanation absent")]
fn a_departure_after_a_write_still_needs_an_explanation() {
    run::<10>();
}
#[test]
#[should_panic(expected = "nothing sent lost")]
fn previously_sent_words_cannot_leave_later_activity() {
    run::<11>();
}
#[test]
fn ending_the_task_discharges_word_retention() {
    run::<12>();
}

#[test]
fn me_cannot_unpark_a_client_running_outdated_code() {
    let mut referee = skein_world::domain::Referee::new(referee::Rules::new(
        Duration::from_millis(10),
        Duration::from_millis(10),
        Duration::from_millis(10),
    ));
    let client = ClientId(0);
    let mut out = Vec::new();
    referee.observe(Time::ZERO, Observation::Client { client }, &mut out);
    referee.observe(Time::ZERO, Observation::Truth(Observed::Reachable { live: true }), &mut out);
    referee.observe(Time::ZERO, Observation::Delivered { client, receipt: Receipt::OutOfDate }, &mut out);
    referee.observe(
        Time::ZERO,
        Observation::Delivered { client, receipt: Receipt::Party { party: 7, watch: Some(Token::new(1)) } },
        &mut out,
    );
    referee.fire(Time::from_nanos(20_000_000), &mut out);
    assert_eq!(referee.verdict(), skein_world::domain::Verdict::Passed);
    referee.observe(
        Time::from_nanos(20_000_000),
        Observation::Delivered { client, receipt: Receipt::Started },
        &mut out,
    );
    referee.observe(
        Time::from_nanos(20_000_000),
        Observation::Delivered { client, receipt: Receipt::Party { party: 7, watch: Some(Token::new(1)) } },
        &mut out,
    );
    assert_eq!(referee.next_deadline(), Some(Time::from_nanos(30_000_000)));
    referee.observe(
        Time::from_nanos(20_000_000),
        Observation::Shown { client, seen: vec![Seen::Link { live: true }] },
        &mut out,
    );
    assert_eq!(referee.verdict(), skein_world::domain::Verdict::Passed);
}

/// A referee of 10 ms heartbeats whose link comes back within 40 ms, with one
/// tab admitted at zero while the engine is `live` or not.
fn reloading(live: bool) -> (skein_world::domain::Referee<referee::Rules>, ClientId) {
    let span = Duration::from_millis(10);
    let mut referee = skein_world::domain::Referee::new(referee::Rules::new(span, span, Duration::from_millis(40)));
    let client = ClientId(0);
    let mut out = Vec::new();
    referee.observe(Time::ZERO, Observation::Client { client }, &mut out);
    referee.observe(Time::ZERO, Observation::Truth(Observed::Reachable { live }), &mut out);
    referee.observe(Time::ZERO, Observation::Shown { client, seen: vec![Seen::Link { live }] }, &mut out);
    (referee, client)
}

#[test]
fn a_reloaded_tab_owes_its_link_afresh() {
    let (mut referee, client) = reloading(false);
    let mut out = Vec::new();
    referee.observe(Time::from_nanos(5_000_000), Observation::Reloaded { client }, &mut out);
    assert_eq!(referee.next_deadline(), Some(Time::from_nanos(45_000_000)));
    referee.fire(Time::from_nanos(20_000_000), &mut out);
    referee.observe(
        Time::from_nanos(30_000_000),
        Observation::Shown { client, seen: vec![Seen::Link { live: false }] },
        &mut out,
    );
    referee.fire(Time::from_nanos(50_000_000), &mut out);
    assert_eq!(referee.verdict(), skein_world::domain::Verdict::Passed);
}

#[test]
fn a_reloaded_tab_never_told_its_link_is_caught() {
    for live in [false, true] {
        let (mut referee, client) = reloading(live);
        let mut out = Vec::new();
        referee.observe(Time::from_nanos(5_000_000), Observation::Reloaded { client }, &mut out);
        referee.fire(Time::from_nanos(50_000_000), &mut out);
        assert!(
            matches!(referee.verdict(), skein_world::domain::Verdict::Failed(_)),
            "the earlier page's link stood for the reloaded one, live: {live}"
        );
    }
}

#[test]
fn a_tree_breaking_the_outline_fails_the_referee() {
    let mut referee = skein_world::domain::Referee::new(referee::Rules::new(
        Duration::from_millis(10),
        Duration::from_millis(10),
        Duration::from_millis(10),
    ));
    let mut out = Vec::new();
    referee.observe(
        Time::ZERO,
        Observation::Shown {
            client: ClientId(0),
            seen: vec![Seen::Outline {
                fault: skein_ui::outline::Fault { rule: skein_ui::outline::Rule::OneMain, node: 2 },
            }],
        },
        &mut out,
    );
    assert_eq!(
        referee.verdict(),
        skein_world::domain::Verdict::Failed(skein_world::domain::Failure {
            at: Time::ZERO,
            why: "the page's outline: a tree broke skein's ui.md, 14".to_owned(),
            pending: vec![],
        })
    );
}

const WATCH_ENDS: &[jig_client_domain::contract::WatchEnd] = &[
    jig_client_domain::contract::WatchEnd::Closed,
    jig_client_domain::contract::WatchEnd::Dropped,
    jig_client_domain::contract::WatchEnd::Busy,
    jig_client_domain::contract::WatchEnd::Stopping,
    jig_client_domain::contract::WatchEnd::Unknown,
    jig_client_domain::contract::WatchEnd::NotAllowed,
    jig_client_domain::contract::WatchEnd::SubjectEnded,
    jig_client_domain::contract::WatchEnd::SignedOut,
];
#[test]
fn me_after_its_watch_ends_is_no_evidence() {
    use jig_client::Up;
    use jig_client_domain::contract::{Delivery, WatchEvent};
    use jig_client_world::observe::up_received;
    const WATCH: Token = Token::new(1);
    for end in WATCH_ENDS {
        let span = Duration::from_millis(10);
        let mut referee = skein_world::domain::Referee::new(referee::Rules::new(span, span, span));
        let client = ClientId(0);
        let mut out = Vec::new();
        referee.observe(Time::ZERO, Observation::Client { client }, &mut out);
        referee.observe(Time::ZERO, Observation::Truth(Observed::Reachable { live: true }), &mut out);
        referee.observe(Time::ZERO, Observation::Delivered { client, receipt: Receipt::Parked }, &mut out);
        for event in [
            Up::WatchEnded { watch: WATCH, end: *end },
            Up::Watched {
                watch: WATCH,
                event: WatchEvent::Delivered(Delivery::Me(Box::new(jig_client_world::stories::frame(7)))),
            },
        ] {
            for receipt in up_received(&event) {
                referee.observe(Time::ZERO, Observation::Delivered { client, receipt }, &mut out);
            }
        }
        referee.fire(Time::from_nanos(20_000_000), &mut out);
        assert_eq!(referee.verdict(), skein_world::domain::Verdict::Passed, "{end:?}");
    }
}

#[test]
fn a_sign_out_shown_done_before_the_engine_received_it_is_caught() {
    let span = Duration::from_millis(10);
    let mut referee = skein_world::domain::Referee::new(referee::Rules::new(span, span, span));
    let client = ClientId(0);
    let mut out = Vec::new();
    referee.observe(Time::ZERO, Observation::Client { client }, &mut out);
    referee.observe(
        Time::ZERO,
        Observation::Delivered { client, receipt: Receipt::Party { party: 7, watch: None } },
        &mut out,
    );
    referee.observe(Time::ZERO, Observation::SignOutAsked { client }, &mut out);
    referee.observe(Time::ZERO, Observation::Shown { client, seen: vec![Seen::SignedOut] }, &mut out);
    assert_eq!(
        referee.verdict(),
        skein_world::domain::Verdict::Failed(skein_world::domain::Failure {
            at: Time::ZERO,
            why: "a sign-out told: the engine did not receive sign-out".to_owned(),
            pending: vec![]
        })
    );
}

#[test]
fn a_sign_in_the_engine_ended_shows_the_sign_in_page_after_a_lost_sign_out() {
    let span = Duration::from_millis(10);
    let mut referee = skein_world::domain::Referee::new(referee::Rules::new(span, span, span));
    let client = ClientId(0);
    let mut out = Vec::new();
    referee.observe(Time::ZERO, Observation::Truth(Observed::SignInEnded { client }), &mut out);
    referee.observe(Time::ZERO, Observation::SignOutAsked { client }, &mut out);
    referee.observe(Time::ZERO, Observation::Shown { client, seen: vec![Seen::SignedOut] }, &mut out);
    assert_eq!(referee.verdict(), skein_world::domain::Verdict::Passed);
}

const DRAFT_SPAN: Duration = Duration::from_millis(10);
const DRAFT_CLIENT: ClientId = ClientId(0);
const DRAFT_KEY: jig_client::Key = jig_client::Key { asker: jig_client::Asker::JIG, number: 47 };
const DRAFT_ATTEMPT: Token = Token::new(DRAFT_KEY.number);
fn draft_answer(offer: jig_client::shapes::Fall) -> skein_world::domain::Verdict {
    let mut referee = skein_world::domain::Referee::new(referee::Rules::new(DRAFT_SPAN, DRAFT_SPAN, DRAFT_SPAN));
    let mut out = Vec::new();
    referee.observe(
        Time::ZERO,
        Observation::Shown { client: DRAFT_CLIENT, seen: vec![Seen::Draft { key: DRAFT_KEY, offer }] },
        &mut out,
    );
    referee.observe(
        Time::ZERO,
        Observation::Delivered {
            client: DRAFT_CLIENT,
            receipt: Receipt::DraftAnswer {
                attempt: DRAFT_ATTEMPT,
                outcome: DraftOutcome::Proposed(jig_client_world::stories::proposal().key),
            },
        },
        &mut out,
    );
    referee.observe(
        Time::ZERO,
        Observation::Delivered {
            client: DRAFT_CLIENT,
            receipt: Receipt::Document {
                value: Box::new(jig_client::contract::ReadValue::Proposal(Box::new(
                    jig_client_world::stories::proposal(),
                ))),
            },
        },
        &mut out,
    );
    referee.verdict()
}
#[test]
fn a_draft_within_its_offer_answered_proposed_fails_offers_kept() {
    assert_eq!(
        draft_answer(jig_client::shapes::Fall::Within),
        skein_world::domain::Verdict::Failed(skein_world::domain::Failure {
            at: Time::ZERO,
            why: "offers kept: draft answer differs from its offer".to_owned(),
            pending: vec![],
        })
    );
}
#[test]
fn a_goal_past_the_first_bound_answered_proposed_keeps_the_offer() {
    assert_eq!(
        draft_answer(jig_client::shapes::Fall::Proposable(jig_client::contract::Recipient::Party(7))),
        skein_world::domain::Verdict::Passed
    );
}
#[test]
fn a_draft_proposed_to_another_recipient_fails_offers_kept() {
    assert_eq!(
        draft_answer(jig_client::shapes::Fall::Proposable(jig_client::contract::Recipient::Party(8))),
        skein_world::domain::Verdict::Failed(skein_world::domain::Failure {
            at: Time::ZERO,
            why: "offers kept: proposed to a different recipient".to_owned(),
            pending: vec![],
        })
    );
}
