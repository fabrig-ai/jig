//! Checked composition includes both slot passes (view.md, section 10).
use crate::{drawing, family, words};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub tree: skein_ui::Limits,
    pub text: u32,
}
impl Limits {
    /// Storage declared by a world, rather than every combination admitted by
    /// the domain. Builder checks reject fixtures that exceed this profile;
    /// `of` remains the complete composition envelope.
    #[must_use]
    pub fn for_world(
        domain: &jig_client_test_domain::Limits,
        jig: &jig_client_view::words::Words,
        first: &words::Words,
        nodes: u32,
        slots: u32,
    ) -> Option<Self> {
        if nodes == 0 || slots > nodes {
            return None;
        }
        let owner = crate::connector_values::requirements(&domain.first.connector, jig)?;
        let fragment_nodes =
            family::PART_NODES.max(owner.nodes.checked_add(16)?).max(domain.first.rows.checked_add(2)?);
        let cards = domain.first.rows.checked_add(1)?;
        let application_nodes = fragment_nodes.checked_add(cards.checked_mul(3)?)?.checked_add(9)?;
        let mut tree = jig_client_view::limits::envelope(
            &domain.jig,
            jig.jig_capacity()?,
            &jig_client_view::page::Arrangement::none(),
            0,
            0,
            0,
        )?;
        let jig_nodes = tree.nodes;
        let sections = jig_client_view::task_section_nodes(&domain.jig, jig_client_view::address::TaskSection::Plan)?
            .checked_add(4)?
            .checked_mul(2)?;
        let jig_cards = u64::from(jig_client_view::card_nodes(&domain.jig)?)
            .checked_mul(u64::from(cards))?
            .checked_add(u64::from(sections))?;
        // The complete node ceiling is checked without constructing its storage.
        // Only the world's declared counts need text and patch capacities in u32.
        let slot_nodes = u64::from(jig_nodes)
            .checked_add(jig_cards)?
            .checked_mul(u64::from(fragment_nodes))?
            .checked_add(jig_cards)?
            .checked_add(u64::from(family::NAV_NODES))?;
        let jig_page = jig_client_view::page_nodes(&domain.jig, 0)?;
        let full_nodes = u64::from(jig_nodes.checked_sub(jig_page)?)
            .checked_add(u64::from(jig_page.max(application_nodes)))?
            .checked_add(slot_nodes)?;
        if u64::from(nodes) > full_nodes {
            return None;
        }
        let text = jig_client_view::limits::node_text(domain.jig.text, jig.jig_capacity()?)?
            .max(first.capacity()?.checked_mul(2)?.checked_add(20)?)
            .max(owner.text_per_node)
            .max(jig_client_view::literal_segment::max_bytes(domain.first.text)?);
        tree.text_per_node = tree.text_per_node.max(text);
        tree.nodes = nodes;
        tree.depth = tree.depth.checked_add(owner.depth.checked_add(4)?.max(8))?;
        tree.bindings = nodes;
        tree.slots = slots;
        tree.patches = nodes.checked_mul(5)?;
        tree.text_bytes = nodes
            .checked_mul(text)?
            .checked_add(domain.jig.rows.checked_mul(2)?.checked_mul(jig_client::contract::ACCOUNT_MANAGE_USAGE)?)?;
        tree.drawing_bytes = drawing::BYTES;
        let limits = Self { tree, text: domain.first.text };
        if !valid(&limits, domain, jig, first) {
            return None;
        }
        Some(limits)
    }
    #[must_use]
    pub fn of(
        domain: &jig_client_test_domain::Limits,
        jig: &jig_client_view::words::Words,
        first: &words::Words,
    ) -> Option<Self> {
        let owner = crate::connector_values::requirements(&domain.first.connector, jig)?;
        let fragment_nodes =
            family::PART_NODES.max(owner.nodes.checked_add(16)?).max(domain.first.rows.checked_add(2)?);
        let cards = domain.first.rows.checked_add(1)?;
        let application_nodes = fragment_nodes.checked_add(cards.checked_mul(3)?)?.checked_add(9)?;
        // All nodes bound the possible slots in the public jig envelope.
        // This conservative bound also covers duplicate references to one part.
        let jig_nodes = jig_client_view::limits::of_words(&domain.jig, jig)?.nodes;
        let sections = jig_client_view::task_section_nodes(&domain.jig, jig_client_view::address::TaskSection::Plan)?
            .checked_add(4)?
            .checked_mul(2)?;
        let jig_cards = jig_client_view::card_nodes(&domain.jig)?.checked_mul(cards)?.checked_add(sections)?;
        let part_nodes = jig_nodes.checked_add(jig_cards)?.checked_mul(fragment_nodes)?;
        let slot_nodes = part_nodes.checked_add(jig_cards)?.checked_add(family::NAV_NODES)?;
        let mut tree = jig_client_view::limits::compose(
            &domain.jig,
            jig,
            &jig_client_view::page::Arrangement::none(),
            application_nodes,
            slot_nodes,
            owner.depth.checked_add(4)?.max(8),
        )?;
        let jig_text = jig_client_view::limits::node_text(domain.jig.text, jig.jig_capacity()?)?;
        let text = jig_text
            .max(first.capacity()?.checked_mul(2)?.checked_add(20)?)
            .max(owner.text_per_node)
            .max(jig_client_view::literal_segment::max_bytes(domain.first.text)?);
        // Keep jig's whole-address reserve separate from ordinary node text.
        tree.text_bytes = tree.text_bytes.checked_add(tree.nodes.checked_mul(text.checked_sub(jig_text)?)?)?;
        tree.text_per_node = tree.text_per_node.max(text);
        tree.drawing_bytes = drawing::BYTES;
        let limits = Self { tree, text: domain.first.text };
        if !valid(&limits, domain, jig, first) {
            return None;
        }
        Some(limits)
    }
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(
    _limits: &Limits,
    _domain: &jig_client_test_domain::Limits,
    jig: &jig_client_view::words::Words,
    first: &words::Words,
) -> bool {
    jig.book().capacity(jig_client_view::phrases::jig::JigPhrase::LoadingPart.id()).is_some()
        && first.capacity().is_some()
}
