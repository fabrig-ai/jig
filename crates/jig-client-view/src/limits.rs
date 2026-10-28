//! jig's view limits (client/domain/view.md, section 10).

use skein_ui::Limits;

/// Whole jig composition in one enclosing page, with English's phrase
/// envelope (256 literal bytes, at most three holes). Connector
/// fragments and application pages use `compose`. Custom
/// languages use `of_words` to include their checked saying capacities.
#[must_use]
pub fn of(client: &crate::Limits, arrangement: &crate::page::Arrangement) -> Option<Limits> {
    let phrase = client.text.max(64).checked_mul(3)?.checked_add(256)?;
    with_phrase_bytes(client, phrase, arrangement, 0, 0, 0)
}

/// The same renderer envelope using the largest checked saying across
/// every loaded language; the root may switch language without resizing.
#[must_use]
pub fn of_words(client: &crate::Limits, words: &crate::words::Words) -> Option<Limits> {
    let phrase = words.jig_capacity()?;
    with_phrase_bytes(client, phrase, &crate::page::Arrangement::none(), 0, 0, 0)
}

/// Compose shared siblings around the larger jig or application page.
/// `slot_nodes` sums all owners' fragment bounds; `slot_depth` bounds their
/// additional nesting. Roots enlarge text/drawing/style bounds for their
/// own fragments when those exceed jig's checked saying envelope.
#[must_use]
pub fn compose(
    client: &crate::Limits,
    words: &crate::words::Words,
    arrangement: &crate::page::Arrangement,
    application_nodes: u32,
    slot_nodes: u32,
    slot_depth: u32,
) -> Option<Limits> {
    let phrase = words.jig_capacity()?;
    with_phrase_bytes(client, phrase, arrangement, application_nodes, slot_nodes, slot_depth)
}
fn with_phrase_bytes(
    client: &crate::Limits,
    phrase: u32,
    arrangement: &crate::page::Arrangement,
    application_nodes: u32,
    slot_nodes: u32,
    slot_depth: u32,
) -> Option<Limits> {
    let mut limits = envelope(client, phrase, arrangement, application_nodes, slot_nodes, slot_depth)?;
    // Whole manage addresses and configured names have independent byte bounds.
    // Names cover deployment hosts, two committed answer positions per retained
    // object, goal-row answers, and each visible notice, up to 128 hosts apiece.
    limits.text_bytes = limits
        .nodes
        .checked_mul(node_text(client.text, phrase)?)?
        .checked_add(client.accounts.accounts.checked_mul(2)?.checked_mul(jig_client::contract::ACCOUNT_MANAGE_USAGE)?)?
        .checked_add(client.rows.checked_mul(jig_client::contract::PROGRAM_HOST_NAME)?)?
        .checked_add(
            client
                .notices
                .checked_add(client.objects.checked_mul(2)?)?
                .checked_add(client.rows)?
                .checked_mul(128)?
                .checked_mul(jig_client::contract::PROGRAM_HOST_NAME)?,
        )?;
    limits.patches = limits.nodes.checked_mul(5)?;
    Some(limits)
}

/// Node and attribute envelope before storage products. A bounded world
/// supplies its own node, binding, slot, text and patch capacities.
#[must_use]
pub fn envelope(
    client: &crate::Limits,
    phrase: u32,
    arrangement: &crate::page::Arrangement,
    application_nodes: u32,
    slot_nodes: u32,
    slot_depth: u32,
) -> Option<Limits> {
    let nodes = crate::arranged_nodes(client, 0, arrangement)?
        .max(application_nodes)
        .checked_add(crate::banner_nodes(client)?)?
        .checked_add(crate::navigation_nodes(client)?)?
        .checked_add(crate::confirmation_nodes(client)?)?
        .checked_add(crate::notices_nodes(client)?)?
        .checked_add(slot_nodes)?
        .checked_add(3)?;
    // A field carries a saying as its name and the full domain value.
    // Markdown links may carry both label and external target.
    // Public manage addresses and configured host names stay whole below the display-text bound.
    // skein-ui shares this bound with every text node's run capacity, so this
    // still over-reserves runs when the external-target floor dominates.
    let text_per_node = node_text(client.text, phrase)?
        .max(jig_client::contract::ACCOUNT_MANAGE_USAGE.checked_add(phrase)?)
        .max(jig_client::contract::PROGRAM_HOST_NAME);
    Some(Limits {
        nodes,
        // Page, card/input result, verdict proposal, and its deepest
        // Markdown list path. Dependency regions are siblings, not nested.
        depth: client.rows.checked_add(33)?.checked_add(slot_depth)?,
        text_per_node,
        text_bytes: 0,
        drawing_bytes: 0,
        styles_per_node: 2,
        // At most one remove per old node and one structural plus three
        // attribute edits per new node (diff.rs).
        patches: 0,
        // Each actionable node or part slot issues at most one meaning.
        bindings: nodes,
        slots: nodes,
        markdown_depth: 3,
        scene_marks: 0,
        scene_bytes: 0,
    })
}

pub mod ui;

/// Room for a path's preceding words, omitted-segment mark and one escaped segment.
#[must_use]
pub fn node_text(text: u32, phrase: u32) -> Option<u32> {
    let segment = crate::literal_segment::max_bytes(text)?;
    Some(phrase.checked_mul(2)?.checked_add(segment)?.max(segment.checked_mul(2)?))
}
