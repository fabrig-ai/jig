//! Immutable activity, whole admitted words and page-carried archives
//! (client/domain/contract.md, 5 and 7; domain/engine.md, 5.4 and 7.2).
use super::{chips, owner, results, turns};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_client_scripted_engine as admission;
use jig_domain_tasks as tasks;
use skein_lib::List;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    Owner(owner::Invalid),
    Payload,
    Identity,
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
fn text(value: &[u8], limits: &Limits) -> Result<client::Text, owner::Invalid> {
    client::Text::new(owner::bytes(value, limits)?, limits.text).map_err(|_| owner::Invalid::TooLarge)
}
fn result(value: &tasks::ResultKind, limits: &Limits) -> Result<client::OwnerWordResult, owner::Invalid> {
    Ok(match value {
        tasks::ResultKind::Approved { state, words_present } => {
            client::OwnerWordResult::Approved { state: text(state, limits)?, words_present: *words_present }
        }
        tasks::ResultKind::Report => client::OwnerWordResult::Report,
        tasks::ResultKind::Verdict { code, fields } => {
            let capacity = u32::try_from(fields.len()).map_err(|_| owner::Invalid::TooLarge)?;
            if capacity > limits.rows {
                return Err(owner::Invalid::TooLarge);
            }
            let mut copied = List::with_capacity(capacity);
            for field in fields {
                copied
                    .push(client::ResultField {
                        name: owner::bytes(&field.name, limits)?,
                        value: owner::bytes(&field.value, limits)?,
                    })
                    .map_err(|_| owner::Invalid::TooLarge)?;
            }
            client::OwnerWordResult::Verdict { code: *code, fields: copied }
        }
        tasks::ResultKind::Change { connector, kind, resource } => {
            client::OwnerWordResult::Change { connector: *connector, kind: *kind, resource: *resource }
        }
        tasks::ResultKind::Failed => client::OwnerWordResult::Failed,
        tasks::ResultKind::Cancelled => client::OwnerWordResult::Cancelled,
    })
}
/// # Errors
/// Refuses mismatched source identities, invalid whole message metadata, archive decoding failures or a complete payload above the receiving bounds.
pub fn kind(value: &tasks::MessageKind, limits: &Limits) -> Result<client::OwnerMessageKind, owner::Invalid> {
    Ok(match value {
        tasks::MessageKind::Escalation { task, revision } => {
            client::OwnerMessageKind::Escalation { task: *task, revision: *revision }
        }
        tasks::MessageKind::Proposal { proposer, proposal, kind } => client::OwnerMessageKind::Proposal {
            proposer: *proposer,
            proposal: *proposal,
            kind: match kind {
                tasks::ProposalKind::Batch => client::OwnerMessageProposalKind::Batch,
                tasks::ProposalKind::Effect => client::OwnerMessageProposalKind::Effect,
                tasks::ProposalKind::Amend => client::OwnerMessageProposalKind::Amend,
                tasks::ProposalKind::Widen => client::OwnerMessageProposalKind::Widen,
                tasks::ProposalKind::Release => client::OwnerMessageProposalKind::Release,
                tasks::ProposalKind::Settings => client::OwnerMessageProposalKind::Settings,
            },
        },
        tasks::MessageKind::PersonStalled { task } => client::OwnerMessageKind::PersonStalled { task: *task },
        tasks::MessageKind::ProposalDecision { proposal, accepted } => {
            client::OwnerMessageKind::ProposalDecision { proposal: *proposal, accepted: *accepted }
        }
        tasks::MessageKind::SettingsDecision { proposal, outcome } => client::OwnerMessageKind::SettingsDecision {
            proposal: *proposal,
            outcome: match outcome {
                tasks::SettingsOutcome::Accepted { revision } => {
                    client::SettingsOutcome::Accepted { revision: *revision }
                }
                tasks::SettingsOutcome::Rejected => client::SettingsOutcome::Rejected,
                tasks::SettingsOutcome::Stale { newest } => client::SettingsOutcome::Stale { newest: *newest },
            },
        },
        tasks::MessageKind::QuestionAnswered { question } => {
            client::OwnerMessageKind::QuestionAnswered { question: *question }
        }
        tasks::MessageKind::QuestionEnded { question } => {
            client::OwnerMessageKind::QuestionEnded { question: *question }
        }
        tasks::MessageKind::Words => client::OwnerMessageKind::Words,
        tasks::MessageKind::EffectNotMade { entry } => client::OwnerMessageKind::EffectNotMade { entry: *entry },
        tasks::MessageKind::Amendment { revision } | tasks::MessageKind::FinishRequested { revision } => {
            client::OwnerMessageKind::Amendment { revision: *revision }
        }
        tasks::MessageKind::Met { dependency, removed } => {
            client::OwnerMessageKind::Met { dependency: *dependency, removed: *removed }
        }
        tasks::MessageKind::Question => client::OwnerMessageKind::Question,
        tasks::MessageKind::Answer { question } => client::OwnerMessageKind::Answer { question: *question },
        tasks::MessageKind::Notice { subscription, target, state } => client::OwnerMessageKind::Notice {
            subscription: *subscription,
            target: *target,
            state: match state {
                tasks::NoticeState::Held { .. } => client::OwnerNoticeState::Held,
                tasks::NoticeState::Done => client::OwnerNoticeState::Done,
                tasks::NoticeState::Failed => client::OwnerNoticeState::Failed,
                tasks::NoticeState::Cancelled => client::OwnerNoticeState::Cancelled,
            },
        },
        tasks::MessageKind::Timer { subscription } => client::OwnerMessageKind::Timer { subscription: *subscription },
        tasks::MessageKind::News { subscription, class } => client::OwnerMessageKind::News {
            subscription: *subscription,
            class: match class {
                tasks::NewsClass::Wakes => client::OwnerNewsClass::Wakes,
                tasks::NewsClass::Kept => client::OwnerNewsClass::Kept,
                tasks::NewsClass::Dropped => client::OwnerNewsClass::Dropped,
            },
        },
        tasks::MessageKind::Result(value) => client::OwnerMessageKind::Result(result(value, limits)?),
    })
}
/// # Errors
/// Refuses mismatched source identities, invalid whole message metadata, archive decoding failures or a complete payload above the receiving bounds.
pub fn word(value: &tasks::Word, limits: &Limits) -> Result<client::OwnerWord, Invalid> {
    let copied = word_fields(value, limits).map_err(Invalid::Owner)?;
    if !admission::owner_activity::word_fits(&copied, &bounds(limits)) {
        return Err(Invalid::Payload);
    }
    Ok(copied)
}
fn word_fields(value: &tasks::Word, limits: &Limits) -> Result<client::OwnerWord, owner::Invalid> {
    Ok(client::OwnerWord {
        number: value.number,
        from: results::requester(value.from),
        kind: kind(&value.kind, limits)?,
        words: text(&value.words, limits)?,
        at: value.at,
        hits: value.hits,
        eligible: value.eligible,
    })
}
fn why(value: tasks::Why) -> client::OwnerRunWhy {
    match value {
        tasks::Why::Made => client::OwnerRunWhy::Made,
        tasks::Why::Ready => client::OwnerRunWhy::Ready,
        tasks::Why::Message { message } => client::OwnerRunWhy::Message { message },
        tasks::Why::Released => client::OwnerRunWhy::Released,
        tasks::Why::Retry { tries } => client::OwnerRunWhy::Retry {
            tries: client::OwnerFailureTries {
                transient: tries.transient,
                permanent: tries.permanent,
                run: tries.run,
                agent: tries.agent,
                lost: tries.lost,
                invalid: tries.invalid,
            },
        },
        tasks::Why::Narrowed => client::OwnerRunWhy::Narrowed,
        tasks::Why::Changed => client::OwnerRunWhy::Changed,
    }
}
/// # Errors
/// Refuses mismatched source identities, invalid whole message metadata, archive decoding failures or a complete payload above the receiving bounds.
pub fn line(task: u64, value: &tasks::ActivityLine, limits: &Limits) -> Result<client::ActivityLine, Invalid> {
    if value.task != task {
        return Err(Invalid::Identity);
    }
    let line = match &value.line {
        tasks::Line::Words { message, by, kind: value_kind, words } => client::OwnerActivity::Words {
            message: *message,
            by: results::requester(*by),
            kind: kind(value_kind, limits).map_err(Invalid::Owner)?,
            words: text(words, limits).map_err(Invalid::Owner)?,
        },
        tasks::Line::Heard(value) => client::OwnerActivity::Heard(word_fields(value, limits).map_err(Invalid::Owner)?),
        tasks::Line::Notified { recipient, word } => client::OwnerActivity::Notified {
            recipient: results::requester(*recipient),
            word: word_fields(word, limits).map_err(Invalid::Owner)?,
        },
        tasks::Line::RunBegan { attempt, why: cause, resumed } => {
            client::OwnerActivity::RunBegan { attempt: *attempt, why: why(*cause), resumed: *resumed }
        }
        tasks::Line::RunEnded { attempt, end, spent } => client::OwnerActivity::RunEnded {
            attempt: *attempt,
            end: match end {
                tasks::EndKind::Parked => client::OwnerRunEnd::Parked,
                tasks::EndKind::Finished => client::OwnerRunEnd::Finished,
                tasks::EndKind::Cancelled => client::OwnerRunEnd::Cancelled,
                tasks::EndKind::Failed { class } => client::OwnerRunEnd::Failed { class: chips::class(*class) },
                tasks::EndKind::Refused => client::OwnerRunEnd::Refused,
            },
            spent: *spent,
        },
        tasks::Line::Turn { reply, attempt, turn, read } => {
            client::OwnerActivity::Turn { reply: *reply, attempt: *attempt, turn: *turn, read: *read }
        }
        tasks::Line::Decided { revision } => client::OwnerActivity::Decided { revision: *revision },
        tasks::Line::Kept { attempt, pending, question } => {
            let mut rows = List::with_capacity(u32::try_from(pending.len()).expect("pending count"));
            for number in pending {
                rows.push(*number).expect("same pending count");
            }
            client::OwnerActivity::Kept { attempt: *attempt, pending: rows, question: *question }
        }
        tasks::Line::Milestone { position } => client::OwnerActivity::Milestone { position: *position },
    };
    let copied = client::ActivityLine {
        position: value.position,
        at: value.at,
        event: client::Activity::Owner {
            source: Box::new(client::OwnerActivityLine { task, position: value.position, at: value.at, line }),
            decoded: None,
        },
    };
    admission::admit_activity_line(&copied, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(copied)
}
/// Attach the record carried beside its exact immutable archive identity. The caller
/// retains an undecodable outcome if decoding fails; whole admission remains bounded.
/// # Errors
/// Refuses mismatched source identities, invalid whole message metadata, archive decoding failures or a complete payload above the receiving bounds.
pub fn with_archive(
    value: &client::ActivityLine,
    archive: &jig_domain_core::TurnRecord,
    codec: &jig_protocol_transcript::Limits,
    limits: &Limits,
) -> Result<client::ActivityLine, Invalid> {
    let client::Activity::Owner { source, .. } = &value.event else {
        return Err(Invalid::Identity);
    };
    if !matches!(source.line, client::OwnerActivity::Turn { attempt, turn, .. } if archive.task == source.task && archive.attempt == attempt && archive.turn == turn)
    {
        return Err(Invalid::Identity);
    }
    let mut copied = value.clone();
    let client::Activity::Owner { decoded, .. } = &mut copied.event else {
        unreachable!("checked owner archive reference");
    };
    *decoded = Some(client::ArchivedTurn {
        task: archive.task,
        name: client::TurnName { attempt: archive.attempt, number: archive.turn },
        cumulative: archive.spent,
        read: archive.read,
        at: archive.at,
        turn: match turns::turn(&archive.transcript, codec) {
            Ok(turn) => client::TurnRead::Decoded(turn),
            Err(problem) => client::TurnRead::Undecodable {
                why: match problem {
                    jig_protocol_transcript::Undecodable::Version { found } => client::TurnProblem::Version { found },
                    jig_protocol_transcript::Undecodable::Malformed | jig_protocol_transcript::Undecodable::Unknown => {
                        client::TurnProblem::Malformed
                    }
                    jig_protocol_transcript::Undecodable::TooLarge => client::TurnProblem::TooLarge {
                        bytes: u64::try_from(archive.transcript.len()).expect("bounded transcript"),
                    },
                },
            },
        },
    });
    admission::admit_activity_line(&copied, &bounds(limits)).map_err(|_| Invalid::Payload)?;
    Ok(copied)
}

/// Translate the turn supplied by the store's page, without another read.
/// # Errors
/// Refuses a mismatched line or a whole payload past the client's bounds.
pub fn page_turn(
    task: u64,
    source: &tasks::ActivityLine,
    turn: Option<&jig_domain_core::reads::PageTurn>,
    codec: &jig_protocol_transcript::Limits,
    limits: &Limits,
) -> Result<client::ActivityLine, Invalid> {
    let value = line(task, source, limits)?;
    let Some(turn) = turn else {
        if let tasks::Line::Turn { .. } = source.line {
            return Err(Invalid::Identity);
        }
        return Ok(value);
    };
    match turn {
        jig_domain_core::reads::PageTurn::Carried(record) => with_archive(&value, record, codec, limits),
        jig_domain_core::reads::PageTurn::TooLarge { spent, read, at, bytes } => {
            let tasks::Line::Turn { attempt, turn, .. } = source.line else { return Err(Invalid::Identity) };
            let mut value = value;
            let client::Activity::Owner { decoded, .. } = &mut value.event else { return Err(Invalid::Identity) };
            *decoded = Some(client::ArchivedTurn {
                task,
                name: client::TurnName { attempt, number: turn },
                cumulative: *spent,
                read: *read,
                at: *at,
                turn: client::TurnRead::Undecodable { why: client::TurnProblem::TooLarge { bytes: *bytes } },
            });
            admission::admit_activity_line(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
            Ok(value)
        }
        jig_domain_core::reads::PageTurn::Unread => {
            let tasks::Line::Turn { attempt, turn, .. } = source.line else { return Err(Invalid::Identity) };
            let mut value = value;
            value.event = client::Activity::Turn {
                name: client::TurnName { attempt, number: turn },
                turn: client::TurnRead::Undecodable { why: client::TurnProblem::Malformed },
            };
            admission::admit_activity_line(&value, &bounds(limits)).map_err(|_| Invalid::Payload)?;
            Ok(value)
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    const UTF8_TEXTS: [&[u8]; 2] = [b"~\0/", b"~"];

    use super::*;
    use skein_lib::Wall;
    fn limits() -> Limits {
        Limits { text: 100, turn_text: 100, rows: 4, document_bytes: 100_000, ..crate::test::limits().client.domain }
    }
    fn source(line: tasks::Line) -> tasks::ActivityLine {
        tasks::ActivityLine { task: u64::MAX, position: 7, at: Wall::from_nanos(9), line }
    }
    fn word_source(kind: tasks::MessageKind) -> tasks::Word {
        tasks::Word {
            number: u64::MAX,
            from: tasks::Party::Deployment { project: u32::MAX },
            kind,
            words: Box::from(UTF8_TEXTS[0]),
            at: Wall::EPOCH,
            hits: u32::MAX,
            eligible: false,
        }
    }
    #[test]
    fn immutable_activity_keeps_all_source_kinds_raw_makers_and_independent_message_metadata() {
        let kinds = [
            tasks::MessageKind::Escalation { task: u64::MAX, revision: 9 },
            tasks::MessageKind::Proposal { proposer: u64::MAX, proposal: 0, kind: tasks::ProposalKind::Amend },
            tasks::MessageKind::PersonStalled { task: 0 },
            tasks::MessageKind::ProposalDecision { proposal: 0, accepted: false },
            tasks::MessageKind::Words,
            tasks::MessageKind::EffectNotMade { entry: u64::MAX },
            tasks::MessageKind::Amendment { revision: u64::MAX },
            tasks::MessageKind::Question,
            tasks::MessageKind::Answer { question: 0 },
            tasks::MessageKind::Notice { subscription: u64::MAX, target: 0, state: tasks::NoticeState::Cancelled },
            tasks::MessageKind::Timer { subscription: 0 },
            tasks::MessageKind::News { subscription: 0, class: tasks::NewsClass::Dropped },
            tasks::MessageKind::Result(tasks::ResultKind::Verdict {
                code: u32::MAX,
                fields: Box::new([tasks::ResultField { name: Box::from([255, b'/']), value: Box::from([0]) }]),
            }),
        ];
        for kind in kinds {
            let source_word = word_source(kind);
            let copied_word = word(&source_word, &limits()).expect("whole word");
            assert_eq!(copied_word.number, u64::MAX);
            assert_eq!(copied_word.from, client::OwnerRequester::Deployment { project: u32::MAX });
            assert_eq!(copied_word.words.bytes(), source_word.words.as_ref());
            assert_eq!((copied_word.at, copied_word.hits, copied_word.eligible), (Wall::EPOCH, u32::MAX, false));
            let copied = line(u64::MAX, &source(tasks::Line::Heard(source_word)), &limits()).expect("whole heard line");
            let client::Activity::Owner { source, decoded } = copied.event else {
                panic!("owner line");
            };
            assert!(decoded.is_none());
            assert_eq!(source.line, client::OwnerActivity::Heard(copied_word));
            assert_eq!((source.task, source.position, source.at), (u64::MAX, 7, copied.at));
        }
        let variants = [
            tasks::Line::Words {
                message: 0,
                by: tasks::Party::Person(u64::MAX),
                kind: tasks::MessageKind::Question,
                words: Box::from([]),
            },
            tasks::Line::Notified {
                recipient: tasks::Party::Task(u64::MAX),
                word: word_source(tasks::MessageKind::Words),
            },
            tasks::Line::RunBegan {
                attempt: u64::MAX,
                why: tasks::Why::Retry {
                    tries: tasks::Tries { transient: 1, permanent: 2, run: 3, agent: 4, lost: 5, invalid: 6 },
                },
                resumed: false,
            },
            tasks::Line::RunEnded { attempt: 0, end: tasks::EndKind::Refused, spent: u64::MAX },
            tasks::Line::Turn { reply: u64::MAX, attempt: u64::MAX, turn: u32::MAX, read: Some(0) },
            tasks::Line::Decided { revision: u64::MAX },
            tasks::Line::Milestone { position: 0 },
        ];
        for original in variants {
            let original = source(original);
            let copied = line(u64::MAX, &original, &limits()).expect("whole source line");
            assert_eq!((copied.position, copied.at), (original.position, original.at));
            assert_eq!(line(0, &original, &limits()), Err(Invalid::Identity));
        }
    }
    fn record() -> &'static [u8] {
        include_bytes!("../../../../../crates/jig-protocol-transcript/tests/data/record_turn_full.bin")
    }
    pub(crate) fn codec() -> jig_protocol_transcript::Limits {
        jig_protocol_transcript::Limits {
            smith: smith_transcript::v6::CEILINGS,
            bytes: 146,
            turn: client::TurnLimits { messages: 1, blocks: 1, text: 3 },
        }
    }
    pub(crate) fn archive_fixture()
    -> (client::ActivityLine, jig_domain_core::TurnRecord, jig_protocol_transcript::Limits, Limits) {
        let limits = limits();
        let codec = codec();
        let original = line(
            u64::MAX,
            &source(tasks::Line::Turn { reply: 9, attempt: u64::MAX, turn: u32::MAX, read: Some(0) }),
            &limits,
        )
        .expect("supplied archive");
        let archive = jig_domain_core::TurnRecord {
            opens_window: false,
            task: u64::MAX,
            attempt: u64::MAX,
            turn: u32::MAX,
            spent: 19,
            read: Some(1),
            at: Wall::from_nanos(42),
            transcript: record().into(),
        };
        (original, archive, codec, limits)
    }
    fn assert_archive_metadata(original: &client::ActivityLine, decoded: &client::ActivityLine) {
        let (name, turn) = decoded.event.decoded_turn().expect("decoded archive");
        assert_eq!(name, client::TurnName { attempt: u64::MAX, number: u32::MAX });
        assert_eq!(turn.read_through, None);
        assert_eq!(decoded.event.read_position(), Some(0));
        let client::Activity::Owner { source: before, .. } = &original.event else {
            panic!("source");
        };
        let client::Activity::Owner { source: after, decoded: archive_value } = &decoded.event else {
            panic!("source");
        };
        assert_eq!(before, after);
        let metadata = archive_value.as_ref().expect("fetched metadata");
        assert_eq!((metadata.cumulative, metadata.read, metadata.at), (19, Some(1), Wall::from_nanos(42)));
        let decoded = metadata.turn.decoded().expect("decoded");
        assert_eq!(decoded.spent, u64::MAX);
        assert_eq!(decoded.read_through, None);
    }
    #[test]
    fn fetched_archive_content_retains_the_exact_reference_and_rejects_another_reply_or_whole_overflow() {
        let (original, archive, codec, limits) = archive_fixture();
        assert!(original.event.decoded_turn().is_none());
        assert_eq!(original.event.read_position(), Some(0));
        assert_eq!(
            with_archive(&original, &jig_domain_core::TurnRecord { attempt: 8, ..archive.clone() }, &codec, &limits),
            Err(Invalid::Identity)
        );
        let decoded = with_archive(&original, &archive, &codec, &limits).expect("actual fetched record");
        assert_archive_metadata(&original, &decoded);
        assert!(
            with_archive(
                &original,
                &archive,
                &codec,
                &Limits { document_bytes: 1, turn_text: limits.turn_text, ..limits }
            )
            .is_err()
        );
        let unread = with_archive(
            &original,
            &jig_domain_core::TurnRecord { transcript: Box::from([255, 255]), ..archive.clone() },
            &codec,
            &limits,
        )
        .expect("undecodable archive stays");
        let client::Activity::Owner { decoded: Some(unread), .. } = unread.event else { panic!("archive") };
        assert_eq!(
            unread.turn,
            client::TurnRead::Undecodable { why: client::TurnProblem::Version { found: u16::MAX } }
        );
        assert!(original.event.decoded_turn().is_none());
    }
    #[test]
    fn whole_activity_admission_rejects_invalid_presence_and_preserves_absent_versus_zero_archive_read() {
        let mut invalid = word_source(tasks::MessageKind::Result(tasks::ResultKind::Approved {
            state: Box::from(UTF8_TEXTS[1]),
            words_present: false,
        }));
        assert_eq!(word(&invalid, &limits()), Err(Invalid::Payload));
        invalid.words = Box::from([]);
        assert!(word(&invalid, &limits()).is_ok());
        for read in [None, Some(0), Some(u64::MAX)] {
            let copied = line(u64::MAX, &source(tasks::Line::Turn { reply: 0, attempt: 1, turn: 2, read }), &limits())
                .expect("archive reference");
            assert_eq!(copied.event.read_position(), read);
            let mut mismatched = copied.clone();
            mismatched.position = 8;
            assert!(admission::admit_activity_line(&mismatched, &bounds(&limits())).is_err());
            let mut malformed = copied.clone();
            if let client::Activity::Owner { source, decoded } = &mut malformed.event {
                source.line = client::OwnerActivity::Decided { revision: 0 };
                *decoded = Some(client::ArchivedTurn {
                    task: u64::MAX,
                    name: client::TurnName { attempt: 1, number: 2 },
                    cumulative: 0,
                    read: None,
                    at: Wall::EPOCH,
                    turn: client::TurnRead::Decoded(client::Turn {
                        messages: List::with_capacity(0),
                        used: client::Usage { input: 0, output: 0, cached: 0, cache_written: 0 },
                        spent: 0,
                        read_through: None,
                        cut: false,
                    }),
                });
            }
            assert!(admission::admit_activity_line(&malformed, &bounds(&limits())).is_err());
        }
    }
}
