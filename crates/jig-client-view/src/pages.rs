//! Page composition over domain-owned ordering and windows (client/domain/pages.md).
//! Objects stay in their shared cards; connector values stay in owner slots.
use crate::phrases::jig::JigPhrase;
use crate::{JigBindings, words::Words};
use jig_client::thread::window::{Edge, Edges, End};
use skein_ui::phrases::Hole;
use skein_ui::{Builder, Role, Run};

use crate::{Borrowed, action::Act};
use skein_lib::Token;
mod not_the_work;
pub(crate) mod work;
pub use not_the_work::not_the_work;

pub fn heading(b: &mut Builder, words: &Words, phrase: JigPhrase, holes: &[Hole]) {
    b.open(Role::Heading { level: 1 });
    let title = words.say(phrase, holes);
    b.name(&title);
    b.text(Run::Plain, &title);
    b.close();
}
pub(crate) fn cut(b: &mut Builder, words: &Words, cut: bool) {
    if cut {
        words.status(b, JigPhrase::PageCut, &[]);
    }
}

pub(crate) fn edges(b: &mut Builder, words: &Words, bindings: &mut JigBindings, edges: &Edges, window: u64) {
    edge(b, words, bindings, &edges.before, window, End::Before);
    edge(b, words, bindings, &edges.after, window, End::After);
}
fn edge(b: &mut Builder, words: &Words, bindings: &mut JigBindings, edge: &Edge, window: u64, end: End) {
    match edge {
        Edge::End => words.text(
            b,
            match end {
                End::Before => JigPhrase::WindowStart,
                End::After => JigPhrase::WindowEnd,
            },
            &[],
        ),
        Edge::More(_) => crate::frame::control(
            b,
            words,
            bindings,
            Role::Button,
            match end {
                End::Before => JigPhrase::ReadEarlier,
                End::After => JigPhrase::ReadLater,
            },
            &[],
            Act::More { window: Token::new(window), end },
        ),
    }
}
/// Show the page's excess rows and admission refusal after its sections.
pub fn excess(b: &mut Builder, client: &Borrowed<'_>, words: &Words) {
    let text_limit = client.text_limit;
    match client.page() {
        crate::page::Page::Work { excess, refused, .. } => {
            if *excess > 0 {
                words.status(b, JigPhrase::MoreRows, &[]);
            }
            if let Some(which) = refused {
                words.refusal(b, &jig_client::contract::Refusal::Limit { which: *which }, text_limit);
            }
        }
        crate::page::Page::Starting
        | crate::page::Page::Elsewhere
        | crate::page::Page::SigningIn
        | crate::page::Page::Missing
        | crate::page::Page::OutOfDate
        | crate::page::Page::SomeoneElse => {}
    }
}

pub(crate) fn object_key(object: jig_client::contract::ObjectKey) -> skein_ui::Key {
    use jig_client::contract::ObjectKey;
    let (kind, a, b) = match object {
        ObjectKey::Task(task) => (0, task, 0),
        ObjectKey::Proposal(key) => match key.proposer {
            jig_client::contract::Proposer::Task(task) => (1, task, key.number),
            jig_client::contract::Proposer::Party(party) => (29, party, key.number),
        },
        ObjectKey::Escalation(task) => (2, task, 0),
        ObjectKey::Question { task, message } => (3, task, message),
        ObjectKey::PersonTask(task) => (4, task, 0),
        ObjectKey::Result(task) => (5, task, 0),
        ObjectKey::Note { project, note } => (6, u64::from(project), note),
    };
    skein_ui::Key { owner: crate::Owner::Jig.opaque(), kind, a, b }
}

/// Build the current jig page; an application's own page returns false.
pub fn main(b: &mut Builder, client: &Borrowed<'_>, words: &Words, bindings: &mut JigBindings) -> bool {
    use crate::{address::Address, page::Page};
    match client.page() {
        Page::Elsewhere => return false,
        Page::Work { address, .. } => {
            b.open(Role::Main);
            match address {
                Address::Inbox { .. } => {
                    heading(b, words, JigPhrase::Inbox, &[]);
                    crate::sections::inbox::header(b, client, words, bindings);
                    for section in client.sections() {
                        crate::sections::section(b, client, words, bindings, *section);
                    }
                    crate::sections::inbox::status(b, client, words);
                    excess(b, client, words);
                }
                Address::Conversations { .. } => {
                    heading(b, words, JigPhrase::Conversations, &[]);
                    for section in client.sections() {
                        crate::sections::section(b, client, words, bindings, *section);
                    }
                    excess(b, client, words);
                }
                Address::Task { task, .. } => {
                    heading(b, words, JigPhrase::TaskPage, &[Hole::Number(*task)]);
                    crate::sections::task::header(b, client, words, bindings);
                    if !crate::sections::task::all_shown(client) {
                        crate::sections::task::tabs(b, client, words, bindings);
                    }
                    for section in client.sections() {
                        crate::sections::section(b, client, words, bindings, *section);
                    }
                    excess(b, client, words);
                }
                Address::Project { .. } => {
                    heading(b, words, JigPhrase::ProjectPage, &[]);
                    crate::sections::project::header(b, client, words, bindings);
                    if !crate::sections::project::all_shown(client) {
                        crate::sections::project::tabs(b, client, words, bindings);
                    }
                    for section in client.sections() {
                        crate::sections::section(b, client, words, bindings, *section);
                    }
                    excess(b, client, words);
                }
                Address::Deployment => {
                    heading(b, words, JigPhrase::Deployment, &[]);
                    for section in client.sections() {
                        crate::sections::section(b, client, words, bindings, *section);
                    }
                    excess(b, client, words);
                }
            }
            b.close();
        }
        Page::Starting | Page::SigningIn | Page::Missing | Page::OutOfDate | Page::SomeoneElse => {
            return not_the_work(b, client, words, bindings);
        }
    }
    true
}

/// Agent activity is the conversation log owned by its current task page.
pub use crate::turns::conversation;
/// Compose jig's frame and current page under the root's open page node.
/// When false, the root builds its application's page, then calls `after_page`.
pub fn jig(
    b: &mut Builder,
    client: &Borrowed<'_>,
    nav: skein_ui::SlotRef,
    words: &Words,
    bindings: &mut JigBindings,
) -> bool {
    crate::frame::banner(b, client, words, bindings);
    crate::frame::navigation(b, client, nav, words, bindings);
    let built = main(b, client, words, bindings);
    if built {
        after_page(b, client, words, bindings);
    }
    built
}
/// Finish page composition with the one confirmation and visible notices.
pub fn after_page(b: &mut Builder, client: &Borrowed<'_>, words: &Words, bindings: &mut JigBindings) {
    // A source can stop naming a card before its terminal facts arrive. The
    // owner retains it on this page until the pending answer or linger ends.
    let mut owed = false;
    match client.page() {
        crate::page::Page::Work { .. } | crate::page::Page::Elsewhere => {
            for (_, id) in client.objects().ids() {
                if client.owes(*id) {
                    if !owed {
                        b.open(Role::Complementary);
                        b.region(crate::regions::JigRegion::Owed.region());
                        owed = true;
                    }
                    crate::cards::card(b, client, *id, &crate::cards::Place::Elsewhere, words, bindings);
                }
            }
        }
        crate::page::Page::Starting
        | crate::page::Page::SigningIn
        | crate::page::Page::Missing
        | crate::page::Page::OutOfDate
        | crate::page::Page::SomeoneElse => {}
    }
    if owed {
        b.close();
    }
    crate::confirmation::confirmation(b, client, words, bindings);
    crate::notices::notices(b, client, words, bindings);
}

pub(crate) fn mark_current(b: &mut Builder, client: &Borrowed<'_>, address: crate::address::Address) {
    match client.page() {
        crate::page::Page::Work { address: own, .. } => {
            if client.sections().len() > 1 && own == &address {
                b.state(skein_ui::State::Current);
            }
        }
        crate::page::Page::Starting
        | crate::page::Page::SigningIn
        | crate::page::Page::Elsewhere
        | crate::page::Page::Missing
        | crate::page::Page::OutOfDate
        | crate::page::Page::SomeoneElse => {}
    }
}
