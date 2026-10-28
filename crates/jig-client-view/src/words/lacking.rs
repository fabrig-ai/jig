//! Only supplied authority facts are said; client/domain/contract.md, 4.
use super::Words;
use crate::phrases::jig::JigPhrase;
use jig_client::contract::Lacking;
use skein_ui::Builder;
use skein_ui::phrases::Hole;
impl Words {
    /// Say missing authority, keeping unavailable requested facts absent.
    pub fn lacking(&self, b: &mut Builder, lacking: &Lacking, text_limit: u32) {
        match lacking {
            Lacking::Approver => self.text(b, JigPhrase::ApprovalAuthorityMissing, &[]),
            Lacking::Budget { needed, left } => {
                self.text(b, JigPhrase::BudgetMissing, &[Hole::Amount(*left), Hole::Amount(*needed)]);
            }
            Lacking::Grant(grant) => {
                self.text(
                    b,
                    JigPhrase::LiteralGrantMissing,
                    &[Hole::Number(u64::from(grant.kind)), Hole::Number(u64::from(grant.connector))],
                );
                crate::owner_authority::pattern(b, self, &grant.pattern, text_limit);
            }
            Lacking::GrantKind { kind } => self.text(b, JigPhrase::GrantKindMissing, &[Hole::Number(u64::from(*kind))]),
            Lacking::TaskKind { executor } => {
                self.text(b, JigPhrase::KindMissing, &[]);
                if let Some(executor) = executor {
                    self.executor(b, *executor);
                }
            }
            Lacking::Depth { needed, allowed } => match needed {
                Some(needed) => self.text(
                    b,
                    JigPhrase::DepthMissing,
                    &[Hole::Number(u64::from(*needed)), Hole::Number(u64::from(*allowed))],
                ),
                None => self.text(b, JigPhrase::DepthMaximum, &[Hole::Number(u64::from(*allowed))]),
            },
            Lacking::Deadline { asked, latest } => match asked {
                Some(asked) => match latest {
                    Some(latest) => self.text(
                        b,
                        JigPhrase::DeadlineMissing,
                        &[Hole::Wall(self.adjusted_wall(*asked)), Hole::Wall(self.adjusted_wall(*latest))],
                    ),
                    None => {
                        self.text(b, JigPhrase::DeadlineAuthorityMissing, &[]);
                        self.wall(b, *asked);
                    }
                },
                None => {
                    self.text(b, JigPhrase::DeadlineAuthorityMissing, &[]);
                    if let Some(latest) = latest {
                        self.deadline(b, *latest);
                    }
                }
            },
            Lacking::Tasks { needed, left } => match needed {
                Some(needed) => self.text(
                    b,
                    JigPhrase::TasksMissing,
                    &[Hole::Number(u64::from(*needed)), Hole::Number(u64::from(*left))],
                ),
                None => self.text(b, JigPhrase::TasksLeft, &[Hole::Number(u64::from(*left))]),
            },
            Lacking::Funding => self.text(b, JigPhrase::FundingMissing, &[]),
            Lacking::Tool => self.text(b, JigPhrase::ToolMissing, &[]),
            Lacking::Scope => self.text(b, JigPhrase::ScopeMissing, &[]),
            Lacking::Ceiling(_) | Lacking::HoldOwner { .. } => self.text(
                b,
                JigPhrase::Word(jig_client::words::Word::Refusal(jig_client::words::RefusalWord::Authority)),
                &[],
            ),
            Lacking::Models => self.text(b, JigPhrase::ModelMissing, &[]),
            Lacking::Reference => self.text(b, JigPhrase::ReferenceMissing, &[]),
            Lacking::ResourceAccess => self.text(b, JigPhrase::ResourceAccessMissing, &[]),
            Lacking::ResourceUnavailable => self.text(b, JigPhrase::ResourceUnavailable, &[]),
            Lacking::Shape => self.text(b, JigPhrase::AuthorityShapeMissing, &[]),
            Lacking::Again { required } => {
                let phrase = if *required { JigPhrase::AgainRequired } else { JigPhrase::AgainUnexpected };
                self.text(b, phrase, &[]);
            }
            Lacking::Account => self.text(b, JigPhrase::AccountMissing, &[]),
            Lacking::Writer => self.text(b, JigPhrase::WriterMissing, &[]),
            Lacking::Required { connector, kind } => self.text(
                b,
                JigPhrase::RequiredJudgeMissing,
                &[Hole::Number(u64::from(*connector)), Hole::Number(u64::from(*kind))],
            ),
            Lacking::Failed { connector, kind } => self.text(
                b,
                JigPhrase::FailedJudge,
                &[Hole::Number(u64::from(*connector)), Hole::Number(u64::from(*kind))],
            ),
            Lacking::Unguarded { connector, kind } => self.text(
                b,
                JigPhrase::UnguardedJudge,
                &[Hole::Number(u64::from(*connector)), Hole::Number(u64::from(*kind))],
            ),
        }
    }
}
