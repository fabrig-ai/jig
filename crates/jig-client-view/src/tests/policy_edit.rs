#![expect(clippy::disallowed_methods, reason = "ordinary tests inspect full semantic leaves")]
use super::cards::{contains, words};
use crate::policy_edit;
use jig_client::contract::*;
use skein_lib::{Duration, List};
use skein_ui::{Builder, Role};
fn pattern(last: OwnerPatternEnd) -> OwnerPattern {
    let mut segments = List::with_capacity(2);
    segments.push(Box::from(b"literal/slash".as_slice())).unwrap();
    segments.push(Box::from(b"\xff".as_slice())).unwrap();
    OwnerPattern { segments, last }
}
fn shown(value: &PolicyEdit) -> skein_ui::Tree {
    let words = words();
    let required = policy_edit::required(value, words, super::cards::CLIENT.rows).unwrap();
    let mut b = Builder::new(&skein_ui::Limits {
        nodes: required.nodes.checked_add(1).unwrap(),
        depth: required.depth.checked_add(1).unwrap(),
        text_per_node: required.text_per_node,
        text_bytes: required.text_bytes,
        drawing_bytes: 0,
        styles_per_node: 0,
        patches: 0,
        bindings: 0,
        slots: 0,
        markdown_depth: 0,
        scene_marks: 0,
        scene_bytes: 0,
    });
    b.open(Role::Page);
    policy_edit::render(&mut b, value, words, super::cards::CLIENT.text, super::cards::CLIENT.rows);
    b.close();
    let tree = b.finish();
    assert_eq!(tree.len(), required.nodes.checked_add(1).unwrap());
    tree
}
#[test]
fn policy_requirements_keep_guard_freshness_judge_and_segment_boundaries_in_the_public_fragment() {
    for (guard, must_be_guarded) in
        [(PolicyGuard::Guarded, true), (PolicyGuard::Observed { freshness: Duration::from_nanos(u64::MAX) }, false)]
    {
        for last in
            [OwnerPatternEnd::Exact(Box::from(b"end".as_slice())), OwnerPatternEnd::Open(Box::from(b"end".as_slice()))]
        {
            let exact = match &last {
                OwnerPatternEnd::Exact(_) => true,
                OwnerPatternEnd::Open(_) => false,
            };
            let mut requirements = List::with_capacity(1);
            requirements
                .push(PolicyRequirement {
                    connector: u16::MAX,
                    kind: u16::MAX,
                    pattern: pattern(last),
                    judge: PolicyJudge { connector: u16::MAX, requirement: u16::MAX, parameters: u32::MAX },
                    guard,
                    must_be_guarded,
                })
                .unwrap();
            let tree = shown(&PolicyEdit::Requirements { requirements });
            for text in [b"literal/slash".as_slice(), b"\\xFF", b"65535", b"4294967295", b"end"] {
                assert!(contains(&tree, text));
            }
            assert!(tree.nodes().iter().any(|node| node.text.iter().any(|run| run.text.as_ref() == b"literal/slash")));
            assert_eq!(contains(&tree, b"Exact ending"), exact);
            assert_eq!(contains(&tree, b"Open ending"), !exact);
            assert_eq!(contains(&tree, b"Guard is required"), must_be_guarded);
            assert_eq!(contains(&tree, b"Guard is not required"), !must_be_guarded);
            match guard {
                PolicyGuard::Guarded => {
                    assert!(contains(&tree, b"Verdict checked at application"));
                    assert!(!contains(&tree, b"Observed verdict freshness"));
                }
                PolicyGuard::Observed { .. } => assert!(contains(&tree, b"18446744073709551615")),
            }
        }
    }
}
#[test]
fn policy_role_projection_permission_and_empty_replacement_reviews_keep_every_supplied_field() {
    let mut mappings = List::with_capacity(1);
    mappings.push(PolicyPermission { connector: u16::MAX, permission: u16::MAX, role: u32::MAX }).unwrap();
    let tree = shown(&PolicyEdit::Permissions { mappings });
    assert!(contains(&tree, b"Connector 65535 permission 65535 maps to role 4294967295"));
    let mut grants = List::with_capacity(1);
    grants
        .push(OwnerGrant {
            connector: 7,
            kind: 9,
            pattern: pattern(OwnerPatternEnd::Open(Box::from(b"end".as_slice()))),
        })
        .unwrap();
    assert!(contains(&shown(&PolicyEdit::Projections { grants }), b"Effect connector 7 kind 9"));
    let tree = shown(&PolicyEdit::Role(PolicyEditRole {
        number: u32::MAX,
        authority: OwnerAuthority {
            models: None,
            tools: u64::MAX,
            grants: List::with_capacity(0),
            delegation: OwnerDelegation { kinds: List::with_capacity(0), tasks: u32::MAX, depth: u32::MAX },
            spend: u64::MAX,
            deadline: None,
            notes: u8::MAX,
            note_resources: List::with_capacity(0),
        },
        period_spend: u64::MAX,
        requests: u16::MAX,
        decides: u8::MAX,
    }));
    for text in [
        b"Replace role 4294967295".as_slice(),
        b"Request flags: 65535",
        b"Proposal decision flags: 255",
        b"Period spend:",
        b"18446744073709551615",
    ] {
        assert!(contains(&tree, text));
    }
    assert!(contains(&shown(&PolicyEdit::ProjectSpend { period_spend: u64::MAX }), b"Period spend:"));
    let tree = shown(&PolicyEdit::RoleSpend { role: 4, period_spend: u64::MAX });
    for text in [b"Role 4".as_slice(), b"Period spend:", b"18446744073709551615"] {
        assert!(contains(&tree, text));
    }
    for edit in [
        PolicyEdit::Requirements { requirements: List::with_capacity(0) },
        PolicyEdit::Projections { grants: List::with_capacity(0) },
        PolicyEdit::Permissions { mappings: List::with_capacity(0) },
    ] {
        assert_eq!(shown(&edit).len(), 3);
    }
}

#[test]
fn a_ceiling_review_keeps_the_whole_authority_and_its_label() {
    let ceiling = OwnerAuthority {
        models: None,
        tools: u64::MAX,
        grants: List::with_capacity(0),
        delegation: OwnerDelegation { kinds: List::with_capacity(0), tasks: u32::MAX, depth: u32::MAX },
        spend: u64::MAX,
        deadline: None,
        notes: u8::MAX,
        note_resources: List::with_capacity(0),
    };
    let tree = shown(&PolicyEdit::Ceiling { ceiling });
    assert!(tree.nodes().iter().any(|node| node.name.as_deref() == Some(b"Project ceiling".as_slice())));
    assert!(contains(&tree, b"18446744073709551615"));
    assert!(contains(&tree, b"4294967295"));
}
