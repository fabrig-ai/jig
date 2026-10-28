//! Keep: checked child sums and both slot passes (view.md, 10).
//! Replace: application family and connector node bounds.
use crate::{drawing, words::Words};
/// Every task section is visible together (examples.md, 8).
#[must_use]
pub fn arrangement() -> jig_client_view::page::Arrangement {
    use jig_client_view::address::TaskSection::{Activity, Budget, History, Overview, Panels, Plan};
    let mut task = skein_lib::List::with_capacity(6);
    for section in [Overview, Activity, Plan, History, Budget, Panels] {
        task.push(section).expect("six task sections fit");
    }
    jig_client_view::page::Arrangement { task, project: skein_lib::List::with_capacity(0) }
}
/// Bounds for the composed tree and owner bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub tree: skein_ui::Limits,
}
impl Limits {
    #[must_use]
    pub fn of(
        domain: &jig_ops_client_domain::Limits,
        jig: &jig_client_view::words::Words,
        observability: &Words,
        infrastructure: &Words,
    ) -> Option<Self> {
        let rows = domain.observability.rows.max(domain.infrastructure.rows);
        // Each nested row contributes at most eighty fact, heading, form or fallback nodes.
        // The fixed part reserve includes both deferred Deployment diagnostics.
        let part = rows
            .checked_add(1)?
            .checked_mul(rows.checked_add(1)?)?
            .checked_mul(80)?
            .checked_add(34)?
            .checked_add(jig_client_view::refusal_nodes(&domain.jig)?)?;
        let application_nodes = part.checked_mul(rows.checked_add(1)?)?;
        // A full connector document is reached through a chip phase, runtime,
        // effect, hold, approval, project or deployment slot. Nested rows bound
        // result/verdict/proposal and chip dependencies; object counts bound
        // repeated cards and their dependency chips. Each resource slot adds a
        // label and one literal node per separately supplied bounded segment.
        let documents = domain
            .jig
            .rows
            .checked_add(1)?
            .checked_pow(3)?
            .checked_mul(domain.jig.objects.checked_add(1)?.checked_pow(2)?)?
            .checked_mul(8)?;
        let jig_nodes = jig_client_view::limits::of_words(&domain.jig, jig)?.nodes;
        let jig_cards = jig_client_view::card_nodes(&domain.jig)?.checked_mul(rows.checked_add(1)?)?.checked_add(
            jig_client_view::task_section_nodes(&domain.jig, jig_client_view::address::TaskSection::Plan)?
                .checked_add(4)?,
        )?;
        let slot_nodes = documents
            .checked_mul(part)?
            .checked_add(jig_nodes.checked_mul(rows.checked_add(3)?)?)?
            .checked_add(jig_cards)?
            .checked_add(8)?;
        let mut tree =
            jig_client_view::limits::compose(&domain.jig, jig, &arrangement(), application_nodes, slot_nodes, 8)?;
        let jig_text = jig_client_view::limits::node_text(domain.jig.text, jig.jig_capacity()?)?;
        let text = jig_text
            .max(observability.capacity()?.max(infrastructure.capacity()?).checked_mul(2)?.checked_add(32)?)
            .max(jig_client_view::literal_segment::max_bytes(
                domain.observability.text.max(domain.infrastructure.text),
            )?);
        // Preserve jig's separately priced whole addresses; ordinary connector
        // text only enlarges the shared display envelope by its difference.
        tree.text_bytes = tree.text_bytes.checked_add(tree.nodes.checked_mul(text.checked_sub(jig_text)?)?)?;
        tree.text_per_node = tree.text_per_node.max(text);
        tree.drawing_bytes = drawing::BYTES;
        let limits = Self { tree };
        if !valid(&limits, domain, jig, observability, infrastructure) {
            return None;
        }
        Some(limits)
    }
}
/// Whether the composed phrase capacities are representable.
#[must_use]
pub fn valid(
    _limits: &Limits,
    _domain: &jig_ops_client_domain::Limits,
    jig: &jig_client_view::words::Words,
    observability: &Words,
    infrastructure: &Words,
) -> bool {
    jig.book().capacity(jig_client_view::phrases::jig::JigPhrase::LoadingPart.id()).is_some()
        && observability.capacity().is_some()
        && infrastructure.capacity().is_some()
}
