//! Whole document envelopes at the real client boundary (contract.md, 5–7).
//! Normal Run state remains independent of provisional text and its Continue
//! cursor. Receiving admission prices the complete envelope before it is fed.
use super::{chips, cursor, inbox, offers, owner, queries, results, rows};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_client_scripted_engine as admission;
use jig_domain_core::documents;
use jig_domain_views as views;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    Owner(owner::Invalid),
    Payload,
    Identity,
    Cursor,
}

fn bounds(limits: &Limits) -> admission::Bounds {
    admission::Bounds {
        text: limits.text,
        rows: limits.rows,
        authority: crate::test::AUTHORITY_ENTRIES,
        segments: limits.segments,
        document_bytes: limits.document_bytes,
        ..admission::Bounds::default()
    }
}

/// # Errors
/// Refuses any owner fact or complete boxed document above the receiving bounds.
pub fn frame_read(
    value: &documents::Frame,
    limits: &Limits,
    models: &[jig_domain_core::Model],
    providers: &jig_domain_accounts::Providers,
) -> Result<client::ReadValue, Invalid> {
    let value =
        client::ReadValue::Me(Box::new(owner::frame(value, limits, models, providers).map_err(Invalid::Owner)?));
    admission::admit_read_value(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses any owner fact or complete boxed document above the receiving bounds.
pub fn frame_delivery(
    value: &documents::Frame,
    limits: &Limits,
    models: &[jig_domain_core::Model],
    providers: &jig_domain_accounts::Providers,
) -> Result<client::Delivery, Invalid> {
    let value = client::Delivery::Me(Box::new(owner::frame(value, limits, models, providers).map_err(Invalid::Owner)?));
    admission::admit_delivery(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses inconsistent facts or any complete Task payload above bounds.
pub fn task_read(
    source: &documents::Task,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadValue, Invalid> {
    let value = client::ReadValue::Task(Box::new(super::task::document(source, limits, models)?));
    admission::admit_read_value(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses inconsistent facts or any complete Task payload above bounds.
pub fn task_delivery(
    source: &documents::Task,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::Delivery, Invalid> {
    let value = client::Delivery::Task(Box::new(super::task::document(source, limits, models)?));
    admission::admit_delivery(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses any whole pending proposal or reserved nested allocation above bounds.
pub fn proposal_read(
    source: &documents::Proposal,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadValue, Invalid> {
    let value = client::ReadValue::OwnerProposal(Box::new(
        super::proposals::document(source, limits, models).map_err(Invalid::Owner)?,
    ));
    admission::admit_read_value(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses any whole held Task or independent escalation allocation above bounds.
pub fn escalation_read(
    source: &documents::Escalation,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadValue, Invalid> {
    let value = client::ReadValue::OwnerEscalation(Box::new(super::escalations::document(source, limits, models)?));
    admission::admit_read_value(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses any owner fact, reserved allocation or complete Project envelope above bounds.
pub fn project_read(
    source: &documents::Project,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadValue, Invalid> {
    let value = client::ReadValue::OwnerProject(Box::new(
        super::projects::document(source, limits, models).map_err(Invalid::Owner)?,
    ));
    admission::admit_read_value(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses any owner fact, reserved allocation or complete Deployment read envelope above bounds.
pub fn deployment_read(source: &documents::Deployment, limits: &Limits) -> Result<client::ReadValue, Invalid> {
    let value = client::ReadValue::OwnerDeployment(Box::new(
        super::deployment::document(source, limits).map_err(Invalid::Owner)?,
    ));
    admission::admit_read_value(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses any owner fact, reserved allocation or complete Deployment delivery envelope above bounds.
pub fn deployment_delivery(source: &documents::Deployment, limits: &Limits) -> Result<client::Delivery, Invalid> {
    let value = client::Delivery::OwnerDeployment(Box::new(
        super::deployment::document(source, limits).map_err(Invalid::Owner)?,
    ));
    admission::admit_delivery(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses any whole pending person work or optional complete shown result above bounds.
pub fn person_task_read(
    source: &documents::PersonTask,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadValue, Invalid> {
    let value = client::ReadValue::OwnerPersonTask(Box::new(
        super::person_tasks::document(source, limits, models).map_err(Invalid::Owner)?,
    ));
    admission::admit_read_value(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// Admit every whole Chip and the allocated named collection before delivery.
/// # Errors
/// Refuses any owner fact or complete boxed document above the receiving bounds.
pub fn chips_delivery(value: &skein_lib::List<documents::Chip>, limits: &Limits) -> Result<client::Delivery, Invalid> {
    let value = client::Delivery::Chips(
        owner::list(value, limits, |value| chips::chip(value, limits)).map_err(Invalid::Owner)?,
    );
    admission::admit_delivery(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses whole Inbox groups, selection allocations or document admission.
pub fn inbox_delivery(value: &documents::Inbox, limits: &Limits) -> Result<client::Delivery, Invalid> {
    let value = client::Delivery::OwnerInbox(Box::new(inbox::document(value, limits).map_err(Invalid::Owner)?));
    admission::admit_delivery(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// The original watch supplies the project and complete selected filter.
/// Source rows are already selected by their owner; this boundary never refilters them.
/// # Errors
/// Refuses mismatched source identity or any whole reserved payload above bounds.
pub fn tasks_delivery(
    query: &client::TaskQuery,
    value: &documents::Lines,
    limits: &Limits,
) -> Result<client::Delivery, Invalid> {
    let expected = queries::query(query).map_err(|_| Invalid::Identity)?;
    if value.query != expected
        || value.lines.iter().any(|row| Some(row.project) != query.project && query.project.is_some())
    {
        return Err(Invalid::Identity);
    }
    let value = client::Delivery::Tasks(Box::new(client::Tasks {
        rows: owner::list(&value.lines, limits, |row| queries::line(row, limits)).map_err(Invalid::Owner)?,
        more: value.next.is_some(),
    }));
    admission::admit_delivery(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// Retain the complete source subtree under the original Tree watch.
/// Supplied parent, depth and folded counts never reconstruct omitted rows.
/// # Errors
/// Refuses mismatched root/window, a complete cursor or any whole allocation above bounds.
pub fn tree_delivery(
    task: u64,
    window: u32,
    source: &documents::Tree,
    limits: &Limits,
) -> Result<client::Delivery, Invalid> {
    if source.root.number != task || source.rows.capacity() > window {
        return Err(Invalid::Identity);
    }
    let value = client::Delivery::OwnerTree(Box::new(client::OwnerTree {
        root: chips::chip(&source.root, limits).map_err(Invalid::Owner)?,
        rows: owner::list(&source.rows, limits, |row| {
            Ok(client::OwnerTreeRow {
                chip: chips::chip(&row.chip, limits)?,
                parent: row.parent,
                depth: row.depth,
                proposals: owner::list(&row.proposals, limits, |proposal| {
                    Ok(client::ProposalKey {
                        proposer: match proposal.proposer {
                            documents::Proposer::Task(number) => client::Proposer::Task(number),
                            documents::Proposer::Party(number) => client::Proposer::Party(number),
                        },
                        number: proposal.number,
                    })
                })?,
                spent: row.spent,
                dependencies: owner::list(&row.dependencies, limits, |link| results::link(link, limits))?,
                cut: row.cut,
            })
        })
        .map_err(Invalid::Owner)?,
        folded: owner::list(&source.folded, limits, |fold| {
            Ok(client::OwnerTreeFold {
                parent: fold.parent,
                phases: rows::phases(fold.phases),
                held_below: fold.held_below,
            })
        })
        .map_err(Invalid::Owner)?,
        held_below: source.held_below,
        next: source
            .next
            .as_ref()
            .map(|value| cursor::encode_tree(value, task, window, limits.text).map_err(|_| Invalid::Cursor))
            .transpose()?,
    }));
    admission::admit_delivery(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses any whole result metadata, terminal fact or reserved allocation above bounds.
pub fn result_read(
    source: &documents::ResultDoc,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadValue, Invalid> {
    let value = client::ReadValue::Result(Box::new(results::document(source, limits, models).map_err(Invalid::Owner)?));
    admission::admit_read_value(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// An admitted live Question supplies bounded words and no retained ending.
/// # Errors
/// Refuses any source text, chip, offer or complete receiving envelope above bounds.
pub fn question_read(
    source: &documents::Question,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ReadValue, Invalid> {
    let value = client::ReadValue::Question(Box::new(client::Question {
        task: chips::chip(&source.task, limits).map_err(Invalid::Owner)?,
        from: Some(results::requester(source.from)),
        message: source.message,
        words: owner::text(&source.words, limits).map_err(Invalid::Owner)?,
        since: source.at,
        acts: offers::offered(&source.acts, limits, models).map_err(Invalid::Owner)?,
        ending: None,
    }));
    admission::admit_read_value(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

fn raw_text(value: &[u8], limits: &Limits) -> Result<client::Text, Invalid> {
    if value.len() > usize::try_from(limits.text).map_err(|_| Invalid::Owner(owner::Invalid::TooLarge))? {
        return Err(Invalid::Owner(owner::Invalid::TooLarge));
    }
    client::Text::new(value.into(), limits.text).map_err(|_| Invalid::Owner(owner::Invalid::TooLarge))
}

/// # Errors
/// Refuses any owner fact or complete boxed document above the receiving bounds.
pub fn current_call(value: &views::Call, limits: &Limits) -> Result<client::CurrentCall, Invalid> {
    Ok(client::CurrentCall {
        identity: client::CallIdentity {
            owner: value.identity.owner,
            sequence: value.identity.sequence,
            retry: value.identity.retry,
            id: raw_text(&value.identity.id, limits)?,
        },
        tool: raw_text(&value.tool, limits)?,
        input: raw_text(&value.input, limits)?,
        cut: value.cut,
        began: value.began,
    })
}

/// Copy the owner's retained full writing value. The Writing document supplies
/// no start wall, so that fact remains absent. Completion sequence is not a turn.
/// # Errors
/// Refuses any owner fact or complete boxed document above the receiving bounds.
pub fn writing(value: &views::Writing, limits: &Limits) -> Result<client::RunText, Invalid> {
    Ok(client::RunText {
        writing: value
            .writing
            .as_ref()
            .map(|call| {
                Ok(client::WritingCall {
                    id: raw_text(&call.id, limits)?,
                    tool: raw_text(&call.tool, limits)?,
                    from: call.from,
                    through: call.through,
                    input: raw_text(&call.input, limits)?,
                    cut: call.cut,
                })
            })
            .transpose()?,
        attempt: value.turn.attempt,
        began: None,
        turn: client::ProvisionalTurnName {
            attempt: value.turn.attempt,
            number: value.turn.turn,
            retry: value.turn.retry,
        },
        from: value.from,
        through: value.to,
        text: raw_text(&value.text, limits)?,
        cut: value.cut,
        call: value.call.as_ref().map(|call| current_call(call, limits).map(client::LiveCall::Raw)).transpose()?,
    })
}

/// Admit an actual provisional suffix as a stream envelope. The normal `RunState`
/// envelope has a separate Taken path even when both concern the same watch.
/// # Errors
/// Refuses any owner fact or complete boxed document above the receiving bounds.
pub fn stream_delivery(value: &views::Writing, limits: &Limits) -> Result<client::Delivery, Invalid> {
    let value = client::Delivery::Run(Box::new(writing(value, limits)?));
    admission::admit_delivery(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// This normal paced document needs transport Taken, never a synthetic Continue.
/// # Errors
/// Refuses any owner fact or complete boxed document above the receiving bounds.
pub fn run_delivery(value: &documents::Run, limits: &Limits) -> Result<client::Delivery, Invalid> {
    let value = client::Delivery::RunState(Box::new(client::RunState {
        task: value.task,
        attempt: value.attempt,
        committed_turn: value.committed_turn,
        writing: value.writing.as_ref().map(|value| writing(value, limits)).transpose()?,
        call: value.call.as_ref().map(|value| current_call(value, limits)).transpose()?,
    }));
    admission::admit_delivery(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

/// # Errors
/// Refuses any whole account allocation or complete delivery above the receiving bounds.
pub fn accounts_delivery(source: &documents::Accounts, limits: &Limits) -> Result<client::Delivery, Invalid> {
    let value =
        client::Delivery::OwnerAccounts(Box::new(super::accounts::document(source, limits).map_err(Invalid::Owner)?));
    admission::admit_delivery(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(value)
}

#[cfg(test)]
pub(crate) mod tests {
    const UTF8_TEXTS: [&[u8]; 4] = [b"~\0", b"\0~", b"~\0/", b"/~"];

    use super::*;
    use skein_lib::{Duration, List, Token, Wall};

    pub(crate) fn tree_source() -> documents::Tree {
        let mut proposals = List::with_capacity(4);
        for proposer in [documents::Proposer::Task(u64::MAX), documents::Proposer::Party(u64::MAX)] {
            proposals.push(documents::ProposalRef { proposer, number: 0 }).expect("source proposal");
        }
        let mut dependencies = List::with_capacity(4);
        dependencies
            .push(documents::TaskLink::Stored { number: u64::MAX, status: None })
            .expect("source absent status");
        dependencies
            .push(documents::TaskLink::Retained { chip: chips::tests::source_chip(), at: None })
            .expect("source absent time");
        let mut rows = List::with_capacity(4);
        let mut chip = chips::tests::source_chip();
        chip.number = 23;
        rows.push(documents::TreeRow {
            chip,
            parent: 0,
            depth: u32::MAX,
            proposals,
            spent: u64::MAX,
            dependencies,
            cut: true,
        })
        .expect("source descendant");
        let mut folded = List::with_capacity(4);
        folded
            .push(documents::FoldedCount { parent: u64::MAX, phases: rows::tests::phase_source(), held_below: 17 })
            .expect("source independent fold");
        folded
            .push(documents::FoldedCount { parent: 0, phases: documents::PhaseCounts::ZERO, held_below: u32::MAX })
            .expect("source independent empty fold");
        documents::Tree {
            root: chips::tests::source_chip(),
            rows,
            folded,
            held_below: 0,
            next: Some(documents::Cursor::Number { number: u64::MAX }),
        }
    }

    #[test]
    fn whole_tree_keeps_root_descendants_fold_counts_links_loss_and_original_opaque_continuation_without_clipping() {
        let mut source = tree_source();
        let limits = rows::tests::limits();
        let before = source.clone();
        let client::Delivery::OwnerTree(value) =
            tree_delivery(u64::MAX, 4, &source, &limits).expect("whole source tree")
        else {
            panic!("owner tree")
        };
        assert_eq!(value.root, chips::chip(&source.root, &limits).expect("whole root"));
        assert_eq!(
            (value.rows.len(), value.rows.capacity(), value.folded.len(), value.folded.capacity()),
            (1, 4, 2, 4)
        );
        assert_eq!(value.held_below, 0);
        let row = value.rows.get(0).expect("whole descendant");
        assert_eq!((row.chip.task, row.parent, row.depth, row.spent, row.cut), (23, 0, u32::MAX, u64::MAX, true));
        assert_eq!((row.proposals.capacity(), row.dependencies.capacity()), (4, 4));
        assert_eq!(
            row.proposals.as_slice(),
            &[
                client::ProposalKey { proposer: client::Proposer::Task(u64::MAX), number: 0 },
                client::ProposalKey { proposer: client::Proposer::Party(u64::MAX), number: 0 }
            ]
        );
        assert_eq!(row.dependencies.get(0), Some(&client::OwnerTaskLink::Stored { number: u64::MAX, status: None }));
        assert!(matches!(row.dependencies.get(1), Some(client::OwnerTaskLink::Retained { at: None, .. })));
        let fold = value.folded.get(0).expect("all ten counts");
        assert_eq!((fold.parent, fold.held_below), (u64::MAX, 17));
        assert_eq!(fold.phases, rows::phases(rows::tests::phase_source()));
        assert_eq!(value.folded.get(1).expect("independent fold").held_below, u32::MAX);
        assert_eq!(
            cursor::decode_tree(value.next.as_ref().expect("supplied continuation"), u64::MAX, 4, limits.text),
            source.next
        );
        assert_eq!(tree_delivery(0, 4, &source, &limits), Err(Invalid::Identity));
        assert_eq!(tree_delivery(u64::MAX, 3, &source, &limits), Err(Invalid::Identity));
        assert_eq!(
            tree_delivery(u64::MAX, 4, &source, &Limits { rows: 3, turn_text: limits.turn_text, ..limits }),
            Err(Invalid::Owner(owner::Invalid::TooLarge))
        );
        assert_eq!(
            tree_delivery(u64::MAX, 4, &source, &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits }),
            Err(Invalid::Payload)
        );
        assert_eq!(source, before);
        source.next = None;
        source.rows.clear();
        let client::Delivery::OwnerTree(value) =
            tree_delivery(u64::MAX, 4, &source, &limits).expect("whole reserved empty source")
        else {
            panic!("empty owner tree")
        };
        assert_eq!(value.next, None);
        assert_eq!((value.rows.len(), value.rows.capacity(), value.folded.len()), (0, 4, 2));
    }

    #[test]
    fn tree_cursor_namespace_keeps_task_window_full_owner_position_and_refuses_read_or_other_watch_identity() {
        for source in [
            documents::Cursor::Number { number: u64::MAX },
            documents::Cursor::Inbox {
                task: 0,
                object: documents::InboxObject::Proposal { proposer: documents::Proposer::Party(u64::MAX), number: 0 },
                at: Some(Wall::EPOCH),
            },
        ] {
            let value = cursor::encode_tree(&source, u64::MAX, u32::MAX, 200).expect("whole opaque cursor");
            assert_eq!(cursor::decode_tree(&value, u64::MAX, u32::MAX, 200), Some(source));
            assert_eq!(cursor::decode_tree(&value, 0, u32::MAX, 200), None);
            assert_eq!(cursor::decode_tree(&value, u64::MAX, 0, 200), None);
            assert_eq!(cursor::decode(&value, 200), None);
            let exact = u32::try_from(value.value.bytes().len()).expect("bounded cursor bytes");
            assert_eq!(cursor::encode_tree(&source, u64::MAX, u32::MAX, exact), Ok(value.clone()));
            assert!(cursor::encode_tree(&source, u64::MAX, u32::MAX, exact - 1).is_err());
            let mut bytes = value.value.bytes().to_vec();
            bytes.push(0);
            assert_eq!(
                cursor::decode_tree(
                    &client::Cursor::new(bytes.into_boxed_slice(), 200).expect("trailing fixture"),
                    u64::MAX,
                    u32::MAX,
                    200
                ),
                None
            );
        }
    }

    pub(crate) fn question_source() -> documents::Question {
        let mut acts = List::with_capacity(4);
        acts.push(jig_domain_core::acts::Offered {
            effect: None,
            act: jig_domain_core::acts::Act::Answer,
            admission: jig_domain_core::acts::Admission::Allowed,
        })
        .expect("source offer");
        documents::Question {
            acts,
            task: chips::tests::source_chip(),
            message: u64::MAX,
            from: jig_domain_tasks::Party::Task(0),
            at: Wall::EPOCH,
            words: documents::Text { bytes: Box::from(UTF8_TEXTS[0]), cut: true },
        }
    }
    #[test]
    fn bounded_question_reads_keep_exact_sender_identity_words_clock_chip_and_actual_answer_offer() {
        let mut source = question_source();
        let limits = rows::tests::limits();
        for sender in [
            jig_domain_tasks::Party::Task(u64::MAX),
            jig_domain_tasks::Party::Person(0),
            jig_domain_tasks::Party::Deployment { project: u32::MAX },
        ] {
            source.from = sender;
            let original = source.clone();
            let client::ReadValue::Question(value) =
                question_read(&source, &limits, &[jig_core_world::world::scripted_model()])
                    .expect("whole admitted question")
            else {
                panic!("question read")
            };
            assert_eq!(value.from, Some(results::requester(sender)));
            assert_eq!(value.message, u64::MAX);
            assert_eq!(value.words.bytes(), UTF8_TEXTS[0]);
            assert!(value.words.cut);
            assert_eq!(value.since, Wall::EPOCH);
            assert_eq!(value.ending, None);
            assert!(value.task.title.cut);
            assert_eq!((value.acts.capacity(), value.acts.len()), (4, 1));
            assert_eq!(
                value.acts.get(0).expect("actual offer"),
                &client::Offered { effect: None, act: client::ActKind::Answer, admission: client::Admission::Allowed }
            );
            assert_eq!(source, original);
            assert_eq!(
                question_read(
                    &source,
                    &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits },
                    &[jig_core_world::world::scripted_model()]
                ),
                Err(Invalid::Payload)
            );
            assert_eq!(
                question_read(
                    &source,
                    &Limits { rows: 3, turn_text: limits.turn_text, ..limits },
                    &[jig_core_world::world::scripted_model()]
                ),
                Err(Invalid::Owner(owner::Invalid::TooLarge))
            );
            assert_eq!(
                question_read(
                    &source,
                    &Limits { text: 1, turn_text: 1, ..limits },
                    &[jig_core_world::world::scripted_model()]
                ),
                Err(Invalid::Owner(owner::Invalid::TooLarge))
            );
        }
        source.words = documents::Text { bytes: Box::new([]), cut: false };
        let client::ReadValue::Question(value) =
            question_read(&source, &limits, &[jig_core_world::world::scripted_model()])
                .expect("whole empty source words")
        else {
            panic!("empty question")
        };
        assert!(value.words.bytes().is_empty());
    }

    #[test]
    fn task_lines_keep_the_query_requester_clock_and_reserved_rows() {
        let limits = rows::tests::limits();
        let query = client::TaskQuery {
            project: Some(u32::MAX),
            requester: None,
            root: Some(u64::MAX),
            parent: None,
            label: None,
            phase: Some(client::PhaseClass::Ended),
            executor: Some(client::ExecutorKind::Person),
            tracked: None,
            order: client::QueryOrder::Changed,
        };
        let mut lines = List::with_capacity(3);
        lines
            .push(documents::TaskLine {
                chip: chips::tests::source_chip(),
                project: u32::MAX,
                requester: jig_domain_tasks::Party::Deployment { project: u32::MAX },
                tracked: None,
                label: 0,
                flags: jig_domain_tasks::Flags::PLAIN,
                changed: Wall::EPOCH,
                last_activity: Wall::EPOCH,
                ended_at: None,
                budget: chips::tests::source_chip().details.budget,
                waits_on: documents::WaitsOn::Nothing,
                phases: documents::PhaseCounts::ZERO,
                held_below: 0,
                conversation: None,
            })
            .expect("owner row");
        let source = documents::Lines {
            query: queries::query(&query).expect("query"),
            lines,
            next: Some(documents::Cursor::Number { number: 1 }),
        };
        let before = source.clone();
        let client::Delivery::Tasks(value) = tasks_delivery(&query, &source, &limits).expect("whole Tasks") else {
            panic!("Tasks")
        };
        assert!(value.more);
        assert_eq!((value.rows.len(), value.rows.capacity()), (1, 3));
        let row = value.rows.get(0).expect("copied row");
        assert_eq!(row.project, u32::MAX);
        assert_eq!(row.requester, client::OwnerRequester::Deployment { project: u32::MAX });
        assert_eq!(row.changed, Wall::EPOCH);
        assert_eq!(row.chip.task, u64::MAX);
        assert!(row.chip.title.cut);
        assert_eq!(
            tasks_delivery(&client::TaskQuery { project: Some(0), ..query }, &source, &limits),
            Err(Invalid::Identity)
        );
        assert_eq!(
            tasks_delivery(&client::TaskQuery { root: None, ..query }, &source, &limits),
            Err(Invalid::Identity)
        );
        assert_eq!(
            tasks_delivery(&query, &source, &Limits { rows: 2, turn_text: limits.turn_text, ..limits }),
            Err(Invalid::Owner(owner::Invalid::TooLarge))
        );
        assert_eq!(
            tasks_delivery(&query, &source, &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits }),
            Err(Invalid::Payload)
        );
        assert_eq!(source, before);
        let empty = documents::Lines { lines: List::with_capacity(3), next: None, ..source };
        let client::Delivery::Tasks(value) = tasks_delivery(&query, &empty, &limits).expect("empty Ended") else {
            panic!("Tasks")
        };
        assert!(value.rows.is_empty());
        assert!(!value.more);
    }

    #[test]
    fn complete_result_documents_keep_whole_terminal_payload_optional_clocks_metadata_and_offers() {
        let limits = Limits {
            rows: 8,
            text: 200,
            turn_text: 200,
            document_bytes: 100_000,
            ..crate::test::limits().client.domain
        };
        for terminal in [
            documents::Ending::Done(documents::ResultValue::Report(documents::Text {
                bytes: Box::from(UTF8_TEXTS[0]),
                cut: true,
            })),
            documents::Ending::Failed(documents::Text { bytes: Box::from(UTF8_TEXTS[1]), cut: true }),
            documents::Ending::Cancelled {
                reason: documents::Text { bytes: Box::new([]), cut: true },
                result: Some(documents::ResultValue::Approved { state: Box::from([255, 0]), words: None }),
                by: jig_domain_tasks::Party::Deployment { project: u32::MAX },
                at: Wall::EPOCH,
            },
        ] {
            for at in [None, Some(Wall::EPOCH)] {
                let mut source = results::tests::document();
                source.ending = terminal.clone();
                source.at = at;
                source.followups = Some(List::with_capacity(4));
                source
                    .acts
                    .push(jig_domain_core::acts::Offered {
                        effect: None,
                        act: jig_domain_core::acts::Act::Message,
                        admission: jig_domain_core::acts::Admission::Allowed,
                    })
                    .expect("original offer");
                let before = source.clone();
                let client::ReadValue::Result(value) =
                    result_read(&source, &limits, &[jig_core_world::world::scripted_model()]).expect("whole Result")
                else {
                    panic!("Result")
                };
                assert_eq!(value.at, at);
                assert_eq!(value.task.task, u64::MAX);
                assert!(value.task.title.cut);
                assert_eq!(value.acts.capacity(), 4);
                assert_eq!(
                    value.acts.as_slice(),
                    &[client::Offered {
                        effect: None,
                        act: client::ActKind::Message,
                        admission: client::Admission::Allowed
                    }]
                );
                let expected = results::ending(&source.ending, &limits).expect("whole source terminal");
                assert_eq!(value.ending, client::Ending::Task(Box::new(expected.clone())));
                match expected {
                    client::OwnerChipEnding::Done(result) => assert_eq!(value.value, Some(result)),
                    client::OwnerChipEnding::Failed(_) => assert_eq!(value.value, None),
                    client::OwnerChipEnding::Cancelled { result, by, at, reason } => {
                        assert_eq!(value.value, result);
                        assert_eq!(by, client::OwnerRequester::Deployment { project: u32::MAX });
                        assert_eq!(at, Wall::EPOCH);
                        assert!(reason.cut);
                    }
                }
                let details = value.details.expect("actual whole metadata");
                assert!(details.cut);
                assert_eq!(details.chosen_by, None);
                assert_eq!(details.delegates.capacity(), 8);
                assert_eq!(details.followups.expect("known empty batch").capacity(), 4);
                assert_eq!(
                    result_read(
                        &source,
                        &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits },
                        &[jig_core_world::world::scripted_model()]
                    ),
                    Err(Invalid::Payload)
                );
                assert_eq!(
                    result_read(
                        &source,
                        &Limits { rows: 7, turn_text: limits.turn_text, ..limits },
                        &[jig_core_world::world::scripted_model()]
                    ),
                    Err(Invalid::Owner(owner::Invalid::TooLarge))
                );
                assert_eq!(source, before);
            }
        }
    }

    pub(crate) fn frame() -> documents::Frame {
        documents::Frame {
            providers: skein_lib::List::with_capacity(0),
            providers_cut: false,
            key_bytes: 256,
            redirect_bytes: 4096,
            words_bytes: 65_536,
            amendment_bytes: 65_536,
            party: u64::MAX,
            kind: jig_domain_people::Kind::Person,
            login: documents::Text { bytes: Box::from(UTF8_TEXTS[0]), cut: true },
            name: documents::Text { bytes: Box::new([]), cut: false },
            deployment_role: None,
            projects: List::with_capacity(2),
            needs: 17,
            need_counts: jig_domain_people::Needs { questions: 1, proposals: 2, escalations: 3, person_tasks: 4 },
            updates: documents::Count::Known(0),
            results_seen: u64::MAX,
            retention: Duration::from_secs(7),
            cut: true,
        }
    }

    fn call(began: Option<Wall>) -> views::Call {
        views::Call {
            identity: views::CallIdentity {
                owner: Token::new(u64::MAX),
                sequence: u32::MAX,
                retry: u32::MAX,
                id: Box::from(UTF8_TEXTS[2]),
            },
            tool: Box::from(UTF8_TEXTS[3]),
            input: Box::new([]),
            cut: true,
            began,
        }
    }

    #[test]
    fn me_read_and_watch_preserve_allocations_and_owner_loss() {
        let source = frame();
        let before = source.clone();
        // This fixture deliberately reserves three unfilled offer rows.
        let limits = Limits {
            projects: 3,
            rows: 3,
            turn_text: crate::test::limits().client.domain.turn_text,
            ..crate::test::limits().client.domain
        };
        let read =
            frame_read(&source, &limits, &[], &jig_domain_accounts::Providers::none()).expect("whole frame read fits");
        let client::ReadValue::Me(read) = read else { panic!("actual Me read envelope") };
        let delivery = frame_delivery(&source, &limits, &[], &jig_domain_accounts::Providers::none())
            .expect("whole frame watch fits");
        let client::Delivery::Me(delivery) = delivery else { panic!("actual Me watch envelope") };
        assert_eq!(read, delivery);
        assert_eq!(read.party.login.bytes.bytes(), UTF8_TEXTS[0]);
        assert!(read.party.login.cut);
        assert!(read.cut);
        assert_eq!(read.projects.capacity(), 2);
        assert_eq!(read.needs_total, 17);
        assert_eq!(
            frame_read(
                &source,
                &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits },
                &[],
                &jig_domain_accounts::Providers::none()
            ),
            Err(Invalid::Payload)
        );
        assert_eq!(
            frame_delivery(
                &source,
                &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits },
                &[],
                &jig_domain_accounts::Providers::none()
            ),
            Err(Invalid::Payload)
        );
        assert_eq!(source, before);
    }

    #[test]
    fn normal_run_keeps_a_whole_call_after_commit_without_writing_or_invented_clock() {
        let limits = crate::test::limits().client.domain;
        for began in [None, Some(Wall::EPOCH)] {
            let source = documents::Run {
                task: u64::MAX,
                attempt: None,
                committed_turn: u32::MAX,
                writing: None,
                call: Some(call(began)),
            };
            let before = source.clone();
            let value = run_delivery(&source, &limits).expect("whole current call fits");
            let client::Delivery::RunState(value) = value else { panic!("normal whole Run envelope") };
            assert_eq!(value.task, u64::MAX);
            assert_eq!(value.attempt, None);
            assert_eq!(value.committed_turn, u32::MAX);
            assert_eq!(value.writing, None);
            let retained = value.call.expect("writing absence preserves supplied call");
            assert_eq!(retained.identity.owner, Token::new(u64::MAX));
            assert_eq!(retained.identity.sequence, u32::MAX);
            assert_eq!(retained.identity.retry, u32::MAX);
            assert_eq!(retained.identity.id.bytes(), UTF8_TEXTS[2]);
            assert_eq!(retained.tool.bytes(), UTF8_TEXTS[3]);
            assert_eq!(retained.input.bytes(), b"");
            assert!(retained.cut);
            assert_eq!(retained.began, began);
            assert_eq!(
                run_delivery(&source, &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits }),
                Err(Invalid::Payload)
            );
            assert_eq!(
                run_delivery(&source, &Limits { text: 2, turn_text: 2, ..limits }),
                Err(Invalid::Owner(owner::Invalid::TooLarge))
            );
            assert_eq!(source, before);
        }
    }

    #[test]
    fn retained_writing_copies_provisional_retry_offsets_and_loss_separately_from_current_call() {
        let source = documents::Run {
            task: 23,
            attempt: Some(u64::MAX),
            committed_turn: 11,
            writing: Some(views::Writing {
                writing: None,
                turn: views::TurnName { attempt: 19, turn: 17, retry: 3 },
                from: 31,
                to: 33,
                text: Box::from(UTF8_TEXTS[0]),
                cut: true,
                call: None,
            }),
            call: Some(call(None)),
        };
        let limits = crate::test::limits().client.domain;
        let translated = run_delivery(&source, &limits).expect("whole retained writing and call fit");
        let client::Delivery::RunState(translated) = translated else { panic!("normal whole Run envelope") };
        assert_eq!(translated.attempt, Some(u64::MAX));
        assert_eq!(translated.committed_turn, 11);
        let writing = translated.writing.expect("supplied writing retained");
        assert_eq!(writing.attempt, 19);
        assert_eq!(writing.turn, client::ProvisionalTurnName { attempt: 19, number: 17, retry: 3 });
        assert_eq!(writing.began, None);
        assert_eq!(writing.from, 31);
        assert_eq!(writing.through, 33);
        assert_eq!(writing.text.bytes(), UTF8_TEXTS[0]);
        assert!(writing.cut);
        assert_eq!(writing.call, None);
        let current = translated.call.expect("independently supplied current call retained");
        assert_eq!(current.identity.sequence, u32::MAX);
        assert_eq!(current.identity.retry, u32::MAX);
        assert_eq!(current.began, None);
    }
}
