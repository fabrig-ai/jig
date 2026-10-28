use super::*;
pub fn lacks() -> [(p::Lack, w::Lacking); 22] {
    let limits = wire();
    [
        (p::Lack::Approver, w::Lacking::Approver),
        (p::Lack::Funding, w::Lacking::Funding),
        (p::Lack::Tool, w::Lacking::Tool),
        (p::Lack::Scope, w::Lacking::Scope),
        (p::Lack::Models, w::Lacking::Models),
        (p::Lack::Reference, w::Lacking::Reference),
        (p::Lack::ResourceAccess, w::Lacking::ResourceAccess),
        (p::Lack::ResourceUnavailable, w::Lacking::ResourceUnavailable),
        (
            p::Lack::Ceiling(p::Ceiling {
                holder: p::CeilingHolder::Rules,
                role: None,
                dimension: p::CeilingDimension::PeriodSpend,
                value: Some(449_696),
                edit: Some(449_697),
            }),
            w::Lacking::Ceiling(
                w::Ceiling::new(
                    &limits,
                    w::CeilingParts {
                        holder: w::CeilingHolder::Rules,
                        role: None,
                        dimension: w::CeilingDimension::PeriodSpend,
                        value: Some(449_696),
                        edit: Some(449_697),
                    },
                )
                .expect("ceiling scalars"),
            ),
        ),
        (p::Lack::Shape, w::Lacking::Shape),
        (p::Lack::Account, w::Lacking::Account),
        (p::Lack::Writer, w::Lacking::Writer),
        (
            p::Lack::Budget { needs: 11, left: 7 },
            w::Lacking::Budget(
                w::LackingBudget::new(&limits, w::LackingBudgetParts { needed: 11, left: 7 }).expect("lack"),
            ),
        ),
        (
            p::Lack::Grant { kind: 9 },
            w::Lacking::GrantKind(
                w::LackingGrantKind::new(&limits, w::LackingGrantKindParts { kind: 9 }).expect("lack"),
            ),
        ),
        (
            p::Lack::TaskKind { executor: None },
            w::Lacking::TaskKind(
                w::LackingTaskKind::new(&limits, w::LackingTaskKindParts { executor: None }).expect("lack"),
            ),
        ),
        (
            p::Lack::Depth { most: 3 },
            w::Lacking::Depth(
                w::LackingDepth::new(&limits, w::LackingDepthParts { needed: None, allowed: 3 }).expect("lack"),
            ),
        ),
        (
            p::Lack::Deadline,
            w::Lacking::Deadline(
                w::LackingDeadline::new(&limits, w::LackingDeadlineParts { asked: None, latest: None }).expect("lack"),
            ),
        ),
        (
            p::Lack::Tasks { left: 4 },
            w::Lacking::Tasks(
                w::LackingTasks::new(&limits, w::LackingTasksParts { needed: None, left: 4 }).expect("lack"),
            ),
        ),
        (
            p::Lack::Again { required: true },
            w::Lacking::Again(w::LackingAgain::new(&limits, w::LackingAgainParts { required: true }).expect("lack")),
        ),
        (
            p::Lack::Required { connector: 9, kind: 10 },
            w::Lacking::Required(
                w::LackingRequired::new(&limits, w::LackingRequiredParts { connector: 9, kind: 10 }).expect("lack"),
            ),
        ),
        (
            p::Lack::Failed { connector: 9, kind: 10 },
            w::Lacking::Failed(
                w::LackingFailed::new(&limits, w::LackingFailedParts { connector: 9, kind: 10 }).expect("lack"),
            ),
        ),
        (
            p::Lack::Unguarded { connector: 9, kind: 10 },
            w::Lacking::Unguarded(
                w::LackingUnguarded::new(&limits, w::LackingUnguardedParts { connector: 9, kind: 10 }).expect("lack"),
            ),
        ),
    ]
}
