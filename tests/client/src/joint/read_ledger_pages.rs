//! Whole historical pages precede consuming their original opaque reply.
use super::*;
use crate::harness::ClientId;

fn retained(source: reads::Page, what: &client::Read, wrong: client::Read) {
    let mut transport = super::tests::transport();
    let native = Token::new(u64::MAX);
    let l = wire::ledger_pages::tests::limits();
    let other = transport.reply_to(Reply::Read { client: ClientId(1), read: Token::new(0), what: wrong }).into_token();
    let to = transport.reply_to(Reply::Read { client: ClientId(0), read: native, what: what.clone() }).into_token();
    let original = source.clone();
    let source = reads::Answer::Page(Box::new(source));
    assert_eq!(read(&mut transport, other, &source, &l).err(), Some(Invalid::Page(wire::pages::Invalid::Identity)));
    assert!(transport.reply(other).is_some());
    for limits in [
        Limits { document_bytes: 0, turn_text: l.turn_text, ..l },
        Limits { rows: 7, turn_text: l.turn_text, ..l },
        Limits { text: 1, turn_text: 1, ..l },
    ] {
        assert!(read(&mut transport, to, &source, &limits).is_err());
        assert!(transport.reply(to).is_some());
        assert!(transport.reply(other).is_some());
    }
    let Incoming::Callback {
        client: actual_client,
        event:
            Event::Client {
                event:
                    jig_client::Event::Up {
                        up: jig_client::Up::ReadDone { read: actual, result: jig_client::Readout::Jig(result) },
                    },
            },
    } = read(&mut transport, to, &source, &l).expect("whole exact historical page")
    else {
        panic!("original native callback")
    };
    assert_eq!(actual_client, ClientId(0));
    assert_eq!(actual, native);
    let expected = match what {
        client::Read::Ended { task, .. } => wire::ledger_pages::ended(*task, &original, &l),
        client::Read::Spend { project, .. } => wire::ledger_pages::spend(*project, &original, &l),
        client::Read::Holds { .. }
        | client::Read::Tasks { .. }
        | client::Read::Me
        | client::Read::Turn { .. }
        | client::Read::Task { .. }
        | client::Read::Proposal { .. }
        | client::Read::Escalation { .. }
        | client::Read::Question { .. }
        | client::Read::PersonTask { .. }
        | client::Read::Result { .. }
        | client::Read::History { .. }
        | client::Read::Activity { .. }
        | client::Read::Inbox { .. }
        | client::Read::Project { .. }
        | client::Read::Notes { .. }
        | client::Read::Settings
        | client::Read::Setting(_)
        | client::Read::Revision(_)
        | client::Read::Revisions(_)
        | client::Read::Deployment => panic!("one of the two original histories"),
    }
    .expect("all exact owner facts");
    assert_eq!(result, client::ReadResult::Value(expected));
    assert!(transport.reply(to).is_none());
    assert!(transport.reply(other).is_some());
    assert_eq!(read(&mut transport, to, &source, &l).err(), Some(Invalid::MissingRoute));
}

#[test]
fn ended_read_keeps_original_task_cursor_token_and_route_until_whole_native_admission() {
    let source = wire::ledger_pages::tests::ended_source();
    let l = wire::ledger_pages::tests::limits();
    let after =
        wire::cursor::encode(source.before.as_ref().expect("original query cursor"), l.text).expect("whole query edge");
    let what = client::Read::Ended { task: u64::MAX, after: Some(after) };
    assert_eq!(
        wire::reads::read(&what, l.text),
        Ok(reads::Read::EndedDelegates { task: u64::MAX, from: source.before.clone() })
    );
    retained(source, &what, client::Read::Ended { task: 0, after: None });
}

#[test]
fn spend_read_keeps_original_project_cursor_token_and_route_until_whole_native_admission() {
    let source = wire::ledger_pages::tests::spend_source();
    let l = wire::ledger_pages::tests::limits();
    let after =
        wire::cursor::encode(source.before.as_ref().expect("original query cursor"), l.text).expect("whole query edge");
    let what = client::Read::Spend { project: u32::MAX, after: Some(after) };
    assert_eq!(
        wire::reads::read(&what, l.text),
        Ok(reads::Read::Spend { project: u32::MAX, from: source.before.clone() })
    );
    retained(source, &what, client::Read::Spend { project: 0, after: None });
}
