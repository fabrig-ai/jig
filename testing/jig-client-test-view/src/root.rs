//! Compose the frame, owner's page, and two slot passes (root.md, section 9).
use crate::{
    Action, Limits,
    bindings::{self, Bindings},
    family, words,
};
use jig_client_test_domain::{Connector, Domain, first};
use jig_client_view::{JigBindings, Owner};
use skein_ui::Act;
use skein_ui::{Builder, NodeId, Patch, Role, Tree};

use skein_lib::Queue;
#[derive(Debug)]
pub struct View<'a> {
    render: skein_ui::View,
    words: &'a jig_client_view::words::Words,
    first_words: &'a words::Words,
    jig: JigBindings,
    pub(crate) first: Bindings,
    limits: Limits,
    navigation: first::Address,
    hide_proposal_effect: bool,
}
impl<'a> View<'a> {
    #[must_use]
    pub fn new(
        limits: &Limits,
        domain: &jig_client_test_domain::Limits,
        words: &'a jig_client_view::words::Words,
        first_words: &'a words::Words,
        navigation: first::Address,
    ) -> Self {
        assert!(crate::valid(limits, domain, words, first_words), "view capacities are checked");
        Self {
            render: skein_ui::View::new(&limits.tree),
            jig: JigBindings::new(limits.tree.bindings, domain.jig.text),
            first: Bindings::new(limits.tree.bindings, limits.text),
            words,
            first_words,
            limits: *limits,
            navigation,
            hide_proposal_effect: false,
        }
    }
    /// Exercises the referee against an application omitting decision facts.
    pub fn hide_proposal_effect(&mut self) {
        self.hide_proposal_effect = true;
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
pub fn build(view: &mut View<'_>, root: &Domain, out: &mut Queue<Patch>) {
    let shown = jig_client_test_domain::shown(root);
    if !view.render.stale(shown) {
        return;
    }
    view.jig.begin();
    view.first.begin();
    let mut b = view.render.builder();
    b.open(Role::Page);
    if !jig_client_view::jig(&mut b, &root.jig().borrow(root.client()), family::NAV, view.words, &mut view.jig) {
        b.open(Role::Main);
        family::page(&mut b, root, view.first_words, view.words, &mut view.first, view.hide_proposal_effect);
        b.close();
        jig_client_view::after_page(&mut b, &root.jig().borrow(root.client()), view.words, &mut view.jig);
    }
    crate::proposal::confirmation(
        &mut b,
        root,
        view.words,
        view.first_words,
        &mut view.first,
        view.hide_proposal_effect,
    );
    b.close();
    slots(view, root, &mut b);
    view.render.finish(b, shown, out);
}
pub(crate) fn slots(view: &mut View<'_>, root: &Domain, b: &mut Builder) {
    for pass in 0..2 {
        for slot in b.slots(pass) {
            let mut fragment = Builder::new(&view.limits.tree);
            if slot.reference == jig_client_view::signin::SLOT {
                sign_in_slot(view, &mut fragment);
                b.splice(slot.id, fragment);
                continue;
            }
            let filled = match Owner::from_opaque(slot.reference.owner) {
                Some(Owner::Jig) => match view.first.object(slot.reference.token) {
                    Some(key) => match root.jig().objects().find(key) {
                        Some(id) => jig_client_view::cards::card(
                            &mut fragment,
                            &root.jig().borrow(root.client()),
                            id,
                            &jig_client_view::cards::Place::Elsewhere,
                            view.words,
                            &mut view.jig,
                        ),
                        None => false,
                    },
                    None => match view.first.section(slot.reference.token) {
                        Some(section) => jig_client_view::sections::section(
                            &mut fragment,
                            &root.jig().borrow(root.client()),
                            view.words,
                            &mut view.jig,
                            section,
                        ),
                        None => false,
                    },
                },
                Some(Owner::Application { number }) => match Connector::of(number) {
                    Some(Connector::First) => {
                        if slot.reference == family::NAV {
                            family::navigation(&mut fragment, view.navigation, view.first_words, &mut view.first);
                            true
                        } else {
                            match view.jig.part(slot.reference.token) {
                                Some(part) => {
                                    family::part(
                                        &mut fragment,
                                        root.first(),
                                        part,
                                        view.first_words,
                                        view.words,
                                        &mut view.first,
                                    );
                                    true
                                }
                                None => match view.jig.owner_part(slot.reference.token) {
                                    Some(owner) => {
                                        let part = match jig_client_test_domain::family::owner_hold(owner) {
                                            Some(part) => Some(part),
                                            None => jig_client_test_domain::family::owner_effect(owner),
                                        };
                                        match part {
                                            Some(part) => {
                                                family::part(
                                                    &mut fragment,
                                                    root.first(),
                                                    &part,
                                                    view.first_words,
                                                    view.words,
                                                    &mut view.first,
                                                );
                                                true
                                            }
                                            None => false,
                                        }
                                    }
                                    None => false,
                                },
                            }
                        }
                    }
                    Some(Connector::Second) => match view.jig.part(slot.reference.token) {
                        Some(part) => {
                            second(&mut fragment, root, part, view.first_words, view.words);
                            true
                        }
                        None => false,
                    },
                    None => false,
                },
                None => false,
            };
            if filled {
                b.splice(slot.id, fragment);
            } else {
                b.unshowable(slot.id, &view.first_words.say(words::Word::Unshowable, &[]));
            }
        }
    }
}
fn second(
    b: &mut Builder,
    root: &Domain,
    part: &jig_client_domain::contract::Part,
    words: &words::Words,
    jig: &jig_client_view::words::Words,
) {
    use jig_client_domain::contract::Part;
    use skein_ui::phrases::Hole;
    if !root.second().contains(part) {
        words.status(b, words::Word::Unshowable, &[]);
        return;
    }
    match part {
        Part::Connector { connector } => words.text(b, words::Word::Connector, &[Hole::Number(u64::from(*connector))]),
        Part::Resource { path, .. } => {
            jig.text(b, jig_client_view::phrases::jig::JigPhrase::ResourceSegments, &[]);
            for segment in path {
                jig_client_view::literal_segment::render(b, segment);
            }
        }
        Part::Kind { kind, .. } => words.text(b, words::Word::Kind, &[Hole::Number(u64::from(*kind))]),
        Part::Panel { .. }
        | Part::Result { .. }
        | Part::Effect { .. }
        | Part::OwnerEffect { .. }
        | Part::Outbox { .. }
        | Part::Hold { .. }
        | Part::Project { .. }
        | Part::Deployment { .. } => words.status(b, words::Word::Unshowable, &[]),
    }
}
#[must_use]
#[expect(clippy::manual_map, reason = "step code matches optional owner actions explicitly")]
pub fn decode(view: &mut View<'_>, node: NodeId, act: Act) -> Option<Action> {
    let bound = view.render.act(node, act)?;
    if bound.binding.owner == (Owner::Application { number: jig_client_test_domain::FIRST }).opaque()
        && view.first.sign_in(bound.binding.token)
    {
        return match bound.act {
            Act::Press | Act::Submit | Act::Toggle => Some(Action::SignIn),
            Act::Input { .. } | Act::Drawing { .. } => None,
        };
    }
    match Owner::from_opaque(bound.binding.owner) {
        Some(Owner::Jig) => match jig_client_view::decode(&mut view.jig, bound.binding.token, bound.act) {
            Some(action) => Some(Action::Jig(action)),
            None => None,
        },
        Some(Owner::Application { number }) => match Connector::of(number) {
            Some(Connector::First) => match bindings::decode(&view.first, bound.binding.token, bound.act) {
                Some(action) => Some(Action::First(action)),
                None => None,
            },
            Some(Connector::Second) | None => None,
        },
        None => None,
    }
}

fn sign_in_slot(view: &mut View<'_>, fragment: &mut Builder) {
    let label =
        view.words.say(jig_client_view::phrases::jig::JigPhrase::SignInWith, &[jig_client_view::words::name(b"Test")]);
    fragment.open(Role::Button);
    fragment.name(&label);
    fragment.bind(view.first.insert(bindings::Meaning::SignIn));
    fragment.close();
}
