//! The signed-in party document (contract.md, 5.1).
use super::{
    Beyond, Count, DocumentText, HeldRole, Needs, PartyKind, PoolLedger, TaskContractDocument, TaskFlags,
    TemplateBudget, TemplateDeadline, TemplateExecutor,
};
use alloc::boxed::Box;
use skein_lib::{Duration, List, Wall};
/// Who is signed in and what the engine offers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Me {
    pub party: MeParty,
    pub projects: List<MeProject>,
    pub needs: Needs,
    pub needs_total: u32,
    pub updates: Count,
    pub results_seen: u64,
    pub cut: bool,
    pub retention: Duration,
    pub deployment_role: Option<HeldRole>,
    /// Public configured provider offers within the document line bound.
    pub providers: List<super::ProviderOffer>,
    /// Whether additional provider offers were omitted.
    pub providers_cut: bool,
    /// Actual startup key ceiling, including the client carried-value bound.
    pub key_bytes: u32,
    /// The driver's redirect bound: the most bytes a pasted sign-in address may have.
    pub redirect_bytes: u32,
    pub words_bytes: u32,
    pub amendment_bytes: u32,
}

/// The authenticated identity, with independently supplied local text loss.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MeParty {
    pub number: u64,
    pub kind: PartyKind,
    pub login: DocumentText,
    pub name: DocumentText,
}

/// Actual membership and the optional retained pool, independent of charter offers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MeProject {
    pub project: u32,
    pub name: DocumentText,
    pub role: HeldRole,
    pub pool: Option<PoolLedger>,
    pub templates: List<Template>,
    pub creating: Option<Creating>,
    /// The models the party may choose here, as its authority allows them, in configuration's order.
    pub models: List<super::MeModel>,
}

/// One actual procedure alternative supplied for the fixed goal form.
/// See domain/people.md, 5.1, and client/domain/contract.md, 3.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CreateProcedure {
    pub connector: u16,
    pub code: u32,
}

/// The first creating bound and the project ceiling (contract.md, 5.3).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CreateBounds {
    pub authority: Box<super::OwnerAuthorityDocument>,
    pub pool_left: u64,
    pub charters: List<u32>,
    pub procedures: List<CreateProcedure>,
    pub latest: Option<Wall>,
    pub budget: u64,
    pub maximum_budget: u64,
    pub maximum_deadline: Option<Wall>,
}
/// Creating's first bound and what happens beyond it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Creating {
    pub within: CreateBounds,
    pub beyond: Beyond,
}
/// A configured template merged with its project's overrides (protocol/configuration.md, 3.1).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Template {
    pub number: u32,
    pub name: DocumentText,
    pub executor: TemplateExecutor,
    pub contract: TaskContractDocument,
    pub shallower: u32,
    pub budget: TemplateBudget,
    pub deadline: TemplateDeadline,
    pub tracked: Option<u32>,
    pub flags: TaskFlags,
    pub label: u32,
    pub grace: Option<Duration>,
    /// The LLM choice the template gives its task; `None` is its charter's default.
    pub llm: Option<super::LlmChoice>,
}
