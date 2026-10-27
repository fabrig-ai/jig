//! The joint peer preserves each operation's standalone meaning.
use super::{Invalid, List, client, people};
pub(super) fn ask(
    project: u32,
    values: &List<client::Operation>,
    models: &[jig_domain_core::Model],
    names: &mut List<Box<[u8]>>,
) -> Result<people::Ask, Invalid> {
    let mut operations = List::with_capacity(values.len());
    for value in values {
        let ask = match value.clone() {
            client::Operation::Message { project, task, words } => client::Ask::Message { project, task, words },
            client::Operation::Amend { project, task, revision, change, reason } => {
                client::Ask::Amend { project, task, revision, change, reason }
            }
            client::Operation::Cancel { project, task, reason, grace } => {
                client::Ask::Cancel { project, task, reason, grace }
            }
            client::Operation::Release { project, task, revision, again } => {
                client::Ask::Release { project, task, revision, again }
            }
            client::Operation::Stop { project, task } => client::Ask::Stop { project, task },
            client::Operation::Move { project, task, reason } => client::Ask::Move { project, task, reason },
        };
        let operation = match super::translate(&ask, models, names)? {
            people::Ask::Say { project: owner, task, words } if owner == project => {
                people::Operation::Message { task, words }
            }
            people::Ask::Amend { project: owner, task, revision, amendment, release } if owner == project => {
                people::Operation::Amend { task, revision, amendment: Box::new(amendment), release }
            }
            people::Ask::Cancel { project: owner, task, reason, grace } if owner == project => {
                people::Operation::Cancel { task, reason, grace }
            }
            people::Ask::Move { project: owner, task, reason } if owner == project => {
                people::Operation::Move { task, reason }
            }
            people::Ask::Release { project: owner, task, revision, again } if owner == project => {
                people::Operation::Release { task, revision, again }
            }
            people::Ask::Stop { project: owner, task } if owner == project => people::Operation::Stop { task },
            people::Ask::Compound { .. }
            | people::Ask::Create { .. }
            | people::Ask::MarkSeen { .. }
            | people::Ask::Application { .. }
            | people::Ask::EditNote { .. }
            | people::Ask::MakeService { .. }
            | people::Ask::Adopt { .. }
            | people::Ask::Propose { .. }
            | people::Ask::Withdraw { .. }
            | people::Ask::Stop { .. }
            | people::Ask::Cancel { .. }
            | people::Ask::Release { .. }
            | people::Ask::TakePerson { .. }
            | people::Ask::HandBackPerson { .. }
            | people::Ask::AnswerPerson { .. }
            | people::Ask::Move { .. }
            | people::Ask::DecideProposal { .. }
            | people::Ask::Say { .. }
            | people::Ask::AnswerQuestion { .. }
            | people::Ask::Amend { .. }
            | people::Ask::SetRoles { .. }
            | people::Ask::Rename { .. }
            | people::Ask::MakeProject { .. }
            | people::Ask::ChangeSettings { .. }
            | people::Ask::MoveToNewest { .. }
            | people::Ask::ChangePolicy { .. }
            | people::Ask::AccountSignIn { .. }
            | people::Ask::AccountSignOut { .. }
            | people::Ask::SetPool { .. }
            | people::Ask::AccountChange { .. }
            | people::Ask::Hold { .. }
            | people::Ask::Lift { .. }
            | people::Ask::DecideEscalation { .. } => return Err(Invalid::CompoundProject),
        };
        operations.push(operation).expect("same capacity");
    }
    Ok(people::Ask::Compound { project, operations })
}
