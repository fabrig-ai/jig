use super::*;
pub fn admission_0() -> [(p::LimitKind, w::AdmissionLimit); 21] {
    [
        (p::LimitKind::Words, w::AdmissionLimit::Words),
        (p::LimitKind::Amendment, w::AdmissionLimit::Amendment),
        (p::LimitKind::Key, w::AdmissionLimit::Key),
        (p::LimitKind::Name, w::AdmissionLimit::Name),
        (p::LimitKind::Identity, w::AdmissionLimit::Identity),
        (p::LimitKind::Roles, w::AdmissionLimit::Roles),
        (p::LimitKind::Arithmetic, w::AdmissionLimit::Arithmetic),
        (p::LimitKind::Duplicate, w::AdmissionLimit::Duplicate),
        (p::LimitKind::Empty, w::AdmissionLimit::Empty),
        (p::LimitKind::Batch, w::AdmissionLimit::Batch),
        (p::LimitKind::ProjectLiveTasks, w::AdmissionLimit::ProjectLiveTasks),
        (p::LimitKind::LiveTasks, w::AdmissionLimit::LiveTasks),
        (p::LimitKind::Tree, w::AdmissionLimit::Tree),
        (p::LimitKind::Depth, w::AdmissionLimit::Depth),
        (p::LimitKind::Delegates, w::AdmissionLimit::Delegates),
        (p::LimitKind::Subscription, w::AdmissionLimit::Subscription),
        (p::LimitKind::Dependencies, w::AdmissionLimit::Dependencies),
        (p::LimitKind::Cycle, w::AdmissionLimit::Cycle),
        (p::LimitKind::Spec, w::AdmissionLimit::Spec),
        (p::LimitKind::Contract, w::AdmissionLimit::Contract),
        (p::LimitKind::Authority, w::AdmissionLimit::Authority),
    ]
}
pub fn admission_1() -> [(p::LimitKind, w::AdmissionLimit); 23] {
    [
        (p::LimitKind::Inputs, w::AdmissionLimit::Inputs),
        (p::LimitKind::Attempts, w::AdmissionLimit::Attempts),
        (p::LimitKind::LiveDelegates, w::AdmissionLimit::LiveDelegates),
        (p::LimitKind::Restore, w::AdmissionLimit::Restore),
        (p::LimitKind::Read, w::AdmissionLimit::Read),
        (p::LimitKind::Turn, w::AdmissionLimit::Turn),
        (p::LimitKind::HoldKind, w::AdmissionLimit::HoldKind),
        (p::LimitKind::HoldTaken, w::AdmissionLimit::HoldTaken),
        (p::LimitKind::Holds, w::AdmissionLimit::Holds),
        (p::LimitKind::InboxFull, w::AdmissionLimit::InboxFull),
        (p::LimitKind::Notes, w::AdmissionLimit::Notes),
        (p::LimitKind::Revision, w::AdmissionLimit::Revision),
        (p::LimitKind::Question, w::AdmissionLimit::Question),
        (p::LimitKind::Priority, w::AdmissionLimit::Priority),
        (p::LimitKind::Policy, w::AdmissionLimit::Policy),
        (p::LimitKind::Resource, w::AdmissionLimit::Resource),
        (p::LimitKind::Service, w::AdmissionLimit::Service),
        (p::LimitKind::Accounts, w::AdmissionLimit::Accounts),
        (p::LimitKind::HolderAccounts, w::AdmissionLimit::HolderAccounts),
        (p::LimitKind::Sources, w::AdmissionLimit::Sources),
        (p::LimitKind::SignIns, w::AdmissionLimit::SignIns),
        (p::LimitKind::Grace, w::AdmissionLimit::Grace),
        (p::LimitKind::Goals, w::AdmissionLimit::Goals),
    ]
}
pub fn admission_refusal(which: w::AdmissionLimit) -> w::Refusal {
    let limits = wire();
    let which = w::Limit::Admission(
        w::LimitAdmission::new(&limits, w::LimitAdmissionParts { which, bound: None }).expect("limit"),
    );
    w::Refusal::Limit(w::RefusalLimit::new(&limits, w::RefusalLimitParts { which }).expect("refusal"))
}
