//! Keep: frame, owner page and two slot passes (root.md, 9).
//! Replace: connector fragments, family addresses and phrasebooks.
use crate::{
    Action, Limits,
    bindings::{self, Bindings},
    infrastructure, observability,
    words::{self, Word},
};
use jig_client_view::{JigBindings, Owner};
use jig_ops_client_domain::Domain;
use jig_ops_domain_connectors::Connector;
use skein_ui::Act;
use skein_ui::{Builder, NodeId, Patch, Role, Tree};

use skein_lib::Queue;
/// The composed semantic tree and its owner binding tables.
#[derive(Debug)]
pub struct View {
    render: skein_ui::View,
    words: jig_client_view::words::Words,
    observability_words: words::Words,
    infrastructure_words: words::Words,
    jig: JigBindings,
    observability: Bindings,
    infrastructure: Bindings,
    limits: Limits,
    project: u32,
    native_environments: bool,
}
impl View {
    #[must_use]
    pub fn new(
        limits: &Limits,
        domain: &jig_ops_client_domain::Limits,
        words: jig_client_view::words::Words,
        observability_words: words::Words,
        infrastructure_words: words::Words,
        project: u32,
    ) -> Self {
        assert!(
            observability_words.connector == jig_ops_domain_connectors::number(Connector::Observability)
                && infrastructure_words.connector == jig_ops_domain_connectors::number(Connector::Infrastructure),
            "phrasebooks belong to their connector owners"
        );
        assert!(
            crate::valid(limits, domain, &words, &observability_words, &infrastructure_words),
            "view capacities are checked"
        );
        Self {
            render: skein_ui::View::new(&limits.tree),
            jig: JigBindings::new(limits.tree.bindings, domain.jig.text),
            observability: Bindings::new(
                limits.tree.bindings,
                domain.observability.text,
                jig_ops_domain_connectors::number(Connector::Observability),
            ),
            infrastructure: Bindings::new(
                limits.tree.bindings,
                domain.infrastructure.text,
                jig_ops_domain_connectors::number(Connector::Infrastructure),
            ),
            words,
            observability_words,
            infrastructure_words,
            limits: *limits,
            project,
            native_environments: false,
        }
    }
    /// Select the native owner environment vocabulary before building a tree.
    pub fn native_environments(&mut self) {
        self.native_environments = true;
    }
    #[must_use]
    pub fn tree(&self) -> &Tree {
        self.render.tree()
    }
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }
}
#[expect(clippy::too_many_lines, reason = "the root exhaustively routes two connectors in both slot passes")]
pub fn build(view: &mut View, root: &Domain, out: &mut Queue<Patch>) {
    let shown = jig_ops_client_domain::shown(root);
    if !view.render.stale(shown) {
        return;
    }
    view.jig.begin();
    view.observability.begin();
    view.infrastructure.begin();
    let mut builder = view.render.builder();
    builder.open(Role::Page);
    if let Some(task) = arranged_task(&root.jig().borrow(root.client())) {
        task_page(&mut builder, &root.jig().borrow(root.client()), task, &view.words, &mut view.jig);
    } else if !jig_client_view::jig(
        &mut builder,
        &root.jig().borrow(root.client()),
        observability::NAV,
        &view.words,
        &mut view.jig,
    ) {
        builder.open(Role::Main);
        if root.observability().address().is_some() {
            observability::page(&mut builder, root.observability(), &view.observability_words, &mut view.observability);
        } else {
            infrastructure::page(
                &mut builder,
                root.infrastructure(),
                &view.infrastructure_words,
                &view.words,
                &mut view.infrastructure,
            );
        }
        builder.close();
        jig_client_view::after_page(&mut builder, &root.jig().borrow(root.client()), &view.words, &mut view.jig);
    }
    builder.close();
    for pass in 0..2 {
        for slot in builder.slots(pass) {
            let mut fragment = Builder::new(&view.limits.tree);
            if slot.reference == jig_client_view::signin::SLOT {
                let label = view
                    .words
                    .say(jig_client_view::phrases::jig::JigPhrase::SignInWith, &[jig_client_view::words::name(b"Ops")]);
                fragment.open(Role::Button);
                fragment.name(&label);
                fragment.bind(view.observability.insert(bindings::Meaning::Action(Action::SignIn)));
                fragment.close();
                builder.splice(slot.id, fragment);
                continue;
            }
            let filled = match Owner::from_opaque(slot.reference.owner) {
                Some(Owner::Jig) => {
                    let key = match view.observability.object(slot.reference.token) {
                        Some(key) => Some(key),
                        None => view.infrastructure.object(slot.reference.token),
                    };
                    match key {
                        Some(key) => match root.jig().objects().find(key) {
                            Some(id) => jig_client_view::cards::card(
                                &mut fragment,
                                &root.jig().borrow(root.client()),
                                id,
                                &jig_client_view::cards::Place::Elsewhere,
                                &view.words,
                                &mut view.jig,
                            ),
                            None => false,
                        },
                        None => match view.observability.section(slot.reference.token) {
                            Some(section) => jig_client_view::sections::section(
                                &mut fragment,
                                &root.jig().borrow(root.client()),
                                &view.words,
                                &mut view.jig,
                                section,
                            ),
                            None => false,
                        },
                    }
                }
                Some(Owner::Application { number }) => match jig_ops_domain_connectors::connector(number) {
                    Some(Connector::Observability) => {
                        if slot.reference == observability::NAV {
                            observability::navigation(
                                &mut fragment,
                                view.project,
                                &view.observability_words,
                                &mut view.observability,
                            );
                            infrastructure::navigation(
                                &mut fragment,
                                view.project,
                                view.native_environments,
                                &view.infrastructure_words,
                                &mut view.infrastructure,
                            );
                            true
                        } else {
                            match view.jig.part(slot.reference.token) {
                                Some(part) => {
                                    observability::part(
                                        &mut fragment,
                                        root.observability(),
                                        part,
                                        &view.observability_words,
                                        &mut view.observability,
                                    );
                                    true
                                }
                                None => false,
                            }
                        }
                    }
                    Some(Connector::Infrastructure) => match infrastructure_part(&view.jig, slot.reference.token) {
                        Some(part) => {
                            infrastructure::part(
                                &mut fragment,
                                root.infrastructure(),
                                &part,
                                &view.infrastructure_words,
                                &view.words,
                                &mut view.infrastructure,
                            );
                            true
                        }
                        None => false,
                    },
                    None => false,
                },
                None => false,
            };
            if filled {
                builder.splice(slot.id, fragment);
            } else {
                builder.unshowable(slot.id, &view.observability_words.say(Word::Unshowable, &[]));
            }
        }
    }
    view.render.finish(builder, shown, out);
}

fn arranged_task(client: &jig_client_view::Borrowed<'_>) -> Option<u64> {
    use jig_client_view::{address::Address, page::Page};
    match client.page() {
        Page::Work { address, .. } => match address {
            Address::Task { task, .. } => Some(*task),
            Address::Inbox { .. } | Address::Conversations { .. } | Address::Project { .. } | Address::Deployment => {
                None
            }
        },
        Page::Starting | Page::Elsewhere | Page::SigningIn | Page::Missing | Page::OutOfDate | Page::SomeoneElse => {
            None
        }
    }
}

// The application's arrangement keeps jig's outline and its whole sections (pages.md, 12.3).
fn task_page(
    b: &mut Builder,
    client: &jig_client_view::Borrowed<'_>,
    task: u64,
    words: &jig_client_view::words::Words,
    bindings: &mut JigBindings,
) {
    use jig_client_view::phrases::jig::JigPhrase;
    use jig_client_view::{address::TaskSection, page::Section};
    use skein_ui::phrases::Hole;
    jig_client_view::frame::banner(b, client, words, bindings);
    jig_client_view::frame::navigation(b, client, observability::NAV, words, bindings);
    b.open(Role::Main);
    jig_client_view::pages::heading(b, words, JigPhrase::TaskPage, &[Hole::Number(task)]);
    jig_client_view::sections::task::header(b, client, words, bindings);
    for section in [TaskSection::Overview, TaskSection::Activity, TaskSection::Plan, TaskSection::History] {
        jig_client_view::sections::section(b, client, words, bindings, Section::Task { task, section });
    }
    jig_client_view::pages::excess(b, client, words);
    b.close();
    b.open(Role::Complementary);
    for section in [TaskSection::Budget, TaskSection::Panels] {
        jig_client_view::sections::section(b, client, words, bindings, Section::Task { task, section });
    }
    b.close();
    jig_client_view::after_page(b, client, words, bindings);
}
fn infrastructure_part(bindings: &JigBindings, token: skein_lib::Token) -> Option<jig_client::contract::Part> {
    match bindings.part(token) {
        Some(part) => Some(part.clone()),
        None => match bindings.owner_part(token) {
            Some(part) => jig_client_view::application::owner_effect(part),
            None => None,
        },
    }
}
#[must_use]
#[expect(clippy::manual_map, reason = "step code explicitly wraps the owner action")]
pub fn decode(view: &mut View, node: NodeId, act: Act) -> Option<Action> {
    let bound = view.render.act(node, act)?;
    match Owner::from_opaque(bound.binding.owner) {
        Some(Owner::Jig) => match jig_client_view::decode(&mut view.jig, bound.binding.token, bound.act) {
            Some(value) => Some(Action::Jig(value)),
            None => None,
        },
        Some(Owner::Application { number }) => match jig_ops_domain_connectors::connector(number) {
            Some(Connector::Observability) => bindings::decode(&view.observability, bound.binding.token, bound.act),
            Some(Connector::Infrastructure) => bindings::decode(&view.infrastructure, bound.binding.token, bound.act),
            None => None,
        },
        None => None,
    }
}
