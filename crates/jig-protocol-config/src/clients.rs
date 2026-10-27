//! Client heartbeat and local sign-in link (configuration.md, 3).
use crate::{Directories, Path, Problems, check, read};
mod issuer;
mod provider;
use alloc::boxed::Box;
pub use issuer::{ProviderUrls, provider_issuer};
pub use provider::{Provider, ProviderClient, provider_key};
use skein_json::Document;
use skein_lib::{Duration, List};
#[derive(Debug)]
pub struct Clients {
    pub heartbeat: Duration,
    pub link: bool,
    pub provider: Option<Provider>,
}
pub(crate) fn parse(
    document: &Document,
    at: u32,
    path: &Path,
    directories: &Directories,
    problems: &mut Problems,
) -> Option<Clients> {
    let mut object = check::object(document, at, path, problems)?;
    let heartbeat = check::duration(&mut object, document, b"heartbeat_ms", path, problems);
    let mut link = None;
    if let Some(at) = object.required(document, b"link", path, problems) {
        match read::boolean(document, at) {
            Ok(value) => link = Some(value),
            Err(what) => problems.push(path.member(b"link"), what),
        }
    }
    let provider = provider::parse(&mut object, document, path, directories, problems);
    object.finish(document, path, problems);
    Some(Clients { heartbeat: heartbeat?, link: link?, provider })
}
/// The configured models as the client protocol names and shows them, in configuration's order,
/// each with the number of the LLM provider listing it (contract.md, 5.1).
#[must_use]
pub fn models(llm: &crate::Llm) -> Box<[jig_protocol_engine_clients::Model]> {
    let mut models = List::with_capacity(u32::try_from(llm.models.len()).expect("checked models"));
    for (at, model) in llm.models.iter().enumerate() {
        let mut provider = None;
        for item in &llm.providers {
            for name in &item.models {
                if *name == model.name {
                    provider = Some(item.number);
                }
            }
        }
        models
            .push(jig_protocol_engine_clients::Model {
                name: model.name.clone(),
                provider,
                window: model.window,
                output: model.output,
                efforts: model.efforts.clone(),
                prices: jig_protocol_engine_clients::Prices {
                    input: model.input_price,
                    cached: model.cached_price,
                    cache_write: model.cache_write_price,
                    output: model.output_price,
                    unit: model.price_unit,
                },
                description: llm.descriptions.get(at).expect("one description per configured model").clone(),
            })
            .expect("one row per model");
    }
    models.into_boxed()
}

pub(crate) fn check_provider(engine: &crate::Engine, problems: &mut Problems) {
    let Some(provider) = &engine.clients.provider else { return };
    let path = Path::root().member(b"clients").member(b"provider").member(b"addresses");
    for (index, number) in provider.addresses.iter().enumerate() {
        let mut application = false;
        for configured in &engine.providers {
            application |= configured.number == *number && configured.kind == crate::ProviderKind::Application;
        }
        if !application {
            problems.push(check::index(&path, index), crate::What::Refers);
        }
    }
}

/// Refuse configurations whose admitted settings exceed client wire 19.
pub(crate) fn check_settings(limits: &crate::Limits, problems: &mut Problems) {
    let core = &limits.core;
    let wire = &limits.clients.wire;
    let path = Path::root().member(b"limits");
    // settings-v1 bounds a proposal to 128 item heads; its review shares the document envelope.
    if wire.owner_proposal_action_settings_items < 128 || wire.owner_retained_action_settings_items < 128 {
        problems.push(path.member(b"clients.wire.proposal_heads"), crate::What::Bound);
    }
    if wire.settings_review_items < wire.owned_record {
        problems.push(path.member(b"clients.wire.settings_review"), crate::What::Bound);
    }
    if core.settings.proposals > wire.settings_document_pending {
        problems.push(path.member(b"core.settings.proposals"), crate::What::Bound);
    }
    // Defaults and changes may name disjoint settings; both belong to the inventory.
    let inventory = match jig_domain_core::settings::capacity(core) {
        Some(count) => double_settings(count),
        None => None,
    };
    if match inventory {
        Some(count) => count > wire.settings_document_settings,
        None => true,
    } {
        problems.push(path.member(b"tasks.charters"), crate::What::Bound);
        problems.push(path.member(b"core.templates"), crate::What::Bound);
        problems.push(path.member(b"authority.projects"), crate::What::Bound);
    }
    for (name, value, maximum) in [
        (b"tasks.charter_documents".as_slice(), core.tasks.charter_documents, wire.charter_documents),
        (b"authority.segments".as_slice(), core.authority.segments, wire.charter_document_name),
        (b"authority.segment_bytes".as_slice(), core.authority.segment_bytes, wire.charter_document_name_item),
        (b"authority.roles".as_slice(), core.authority.roles, wire.setting_policy_roles),
        (
            b"authority.requirements".as_slice(),
            core.authority.requirements,
            wire.setting_policy_requirements.min(wire.rules_requirements),
        ),
        (b"authority.grants".as_slice(), core.authority.grants, wire.setting_policy_projections),
        (b"authority.implications".as_slice(), core.authority.implications, wire.rules_implies),
        (b"core.templates".as_slice(), core.templates, wire.setting_policy_templates),
        (b"core.settings.permissions".as_slice(), core.settings.permissions, wire.setting_policy_permissions),
        (b"core.settings.alternatives".as_slice(), core.settings.alternatives, wire.charter_alternatives),
        (b"core.run_bytes".as_slice(), core.run_bytes, wire.charter_instructions),
        (b"documents.text_bytes".as_slice(), core.documents.text_bytes, wire.document_text_bytes),
        (b"documents.rows".as_slice(), core.documents.rows, wire.revisions_page_rows),
        (b"documents.lines".as_slice(), core.documents.lines, wire.revision_row_items),
    ] {
        if value > maximum {
            problems.push(path.member(name), crate::What::Bound);
        }
    }
}
fn double_settings(count: u32) -> Option<u32> {
    count.checked_mul(2)
}

#[cfg(test)]
mod settings_tests {
    use super::*;
    fn ceiling(name: &[u8], maximum: u32) {
        let mut limits = crate::profile(crate::Profile::Small);
        crate::overrides::apply(&mut limits, name, u64::from(maximum)).unwrap();
        let mut problems = Problems::empty();
        check_settings(&limits, &mut problems);
        assert!(problems.is_empty(), "at ceiling: {name:?}: {problems:?}");
        crate::overrides::apply(&mut limits, name, u64::from(maximum) + 1).unwrap();
        check_settings(&limits, &mut problems);
        assert_eq!(problems.problems.len(), 1, "one past: {name:?}: {problems:?}");
        assert_eq!(problems.problems.get(0).unwrap().path, Path::root().member(b"limits").member(name));
    }
    #[test]
    fn pending_settings_fit_the_wire_inventory() {
        let maximum = crate::profile(crate::Profile::Small).clients.wire.settings_document_pending;
        ceiling(b"core.settings.proposals", maximum);
    }
    #[test]
    fn policy_roles_fit_the_wire() {
        ceiling(b"authority.roles", 32);
    }
    #[test]
    fn policy_and_rule_requirements_fit_the_wire() {
        ceiling(b"authority.requirements", 32);
    }
    #[test]
    fn policy_projections_fit_the_wire() {
        ceiling(b"authority.grants", 32);
    }
    #[test]
    fn rule_implications_fit_the_wire() {
        ceiling(b"authority.implications", 32);
    }
    #[test]
    fn template_overrides_fit_the_wire() {
        ceiling(b"core.templates", 16);
    }
    #[test]
    fn policy_permissions_fit_the_wire() {
        ceiling(b"core.settings.permissions", 32);
    }
    #[test]
    fn charter_alternatives_fit_the_wire() {
        ceiling(b"core.settings.alternatives", 16);
    }
    #[test]
    fn charter_instructions_fit_the_wire() {
        ceiling(b"core.run_bytes", 1_114_240);
    }
    #[test]
    fn setting_names_fit_the_wire() {
        ceiling(b"documents.text_bytes", 65_536);
    }
    #[test]
    fn revision_page_rows_fit_the_wire() {
        ceiling(b"documents.rows", 32);
    }
    #[test]
    fn revision_item_previews_fit_the_wire() {
        ceiling(b"documents.lines", 32);
    }
    #[test]
    fn inventory_counts_both_defaults_and_changes() {
        let mut limits = crate::profile(crate::Profile::Small);
        // Small has eight templates, two policies and rules: eleven other slots.
        limits.core.tasks.charters = 117;
        let mut problems = Problems::empty();
        check_settings(&limits, &mut problems);
        assert!(problems.is_empty());
        limits.core.tasks.charters = 118;
        check_settings(&limits, &mut problems);
        assert_eq!(problems.problems.len(), 3);
    }
    #[test]
    fn profile_core_shares_remain_representable() {
        for profile in [crate::Profile::Small, crate::Profile::Standard] {
            let limits = crate::profile(profile);
            assert!(jig_domain_core::worst_case(&limits.core).is_some(), "a profile's core share is representable");
        }
    }
}

#[cfg(test)]
mod proposal_settings_tests {
    use super::*;
    #[test]
    fn proposal_heads_and_review_fit_their_wire_ceilings() {
        // settings-v1's proposal holds 128 heads; the review shares Owned.record's document ceiling.
        for (proposal, retained, review, expected) in [
            (128, 128, jig_client_protocol_wire::v19::CEILINGS.owned_record, None),
            (
                127,
                128,
                jig_client_protocol_wire::v19::CEILINGS.owned_record,
                Some(b"clients.wire.proposal_heads".as_slice()),
            ),
            (
                128,
                127,
                jig_client_protocol_wire::v19::CEILINGS.owned_record,
                Some(b"clients.wire.proposal_heads".as_slice()),
            ),
            (
                128,
                128,
                jig_client_protocol_wire::v19::CEILINGS.owned_record - 1,
                Some(b"clients.wire.settings_review".as_slice()),
            ),
        ] {
            let mut limits = crate::profile(crate::Profile::Small);
            limits.clients.wire.owner_proposal_action_settings_items = proposal;
            limits.clients.wire.owner_retained_action_settings_items = retained;
            limits.clients.wire.settings_review_items = review;
            let mut problems = Problems::empty();
            check_settings(&limits, &mut problems);
            match expected {
                None => assert!(problems.is_empty(), "{problems:?}"),
                Some(name) => {
                    assert_eq!(problems.problems.len(), 1);
                    assert_eq!(
                        problems.problems.get(0).expect("one problem").path,
                        Path::root().member(b"limits").member(name)
                    );
                }
            }
        }
    }
}
