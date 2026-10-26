use crate::Limits;
pub const TARGET: u64 = 3;
pub const PROPOSAL: u64 = 2;
pub const LIMITS: Limits = Limits { input_bytes: 8192, words: 4096, batch: 8, depth: 16 };
pub const DELEGATE: &[u8] = br#"{"batch":[{"executor":{"kind":"agent","charter":1},"spec":{"words":"check"},"contract":{"kind":"report","words":100},"authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":100},"notes":0}},{"executor":{"kind":"agent","charter":1},"spec":{"words":"report"},"contract":{"kind":"report","words":100},"authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":100},"notes":0}}]}"#;
pub const MESSAGE: &[u8] = br#"{"target":3,"form":"words","words":"is the build green?"}"#;
pub const CANCEL: &[u8] = br#"{"target":3,"reason":"no longer needed"}"#;
pub const RELEASE: &[u8] = br#"{"target":3,"revision":1,"again":false}"#;
pub const INTRODUCE: &[u8] = br#"{"left":3,"right":4}"#;
pub const DECIDE: &[u8] = br#"{"kind":"proposal","proposer":3,"proposal":2,"decision":"accept"}"#;
pub const WITHDRAW: &[u8] = br#"{"proposal":2}"#;
pub const SUBSCRIBE: &[u8] = br#"{"kind":"task","target":3}"#;
pub const UNSUBSCRIBE: &[u8] = br#"{"subscription":1}"#;
pub const AMEND: &[u8] = br#"{"target":3,"revision":1,"amendment":{"reason":"check again","instructions":"repeat"}}"#;
pub const DECIDE_ESCALATION: &[u8] = br#"{"kind":"escalation","task":3,"revision":1,"decision":"release"}"#;
pub const PROPOSE: &[u8] = br#"{"action":"batch","reason":"help needed","batch":[{"executor":{"kind":"agent","charter":1},"spec":{"words":"check"},"contract":{"kind":"report","words":100},"authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":100},"notes":0}}]}"#;
pub const NOTE: &[u8] = br#"{"scope":{"kind":"deployment"},"description":"status","body":"green"}"#;
pub const RECALL: &[u8] = br#"{"kind":"name","name":1}"#;

pub const FIELDS: &[(&[u8], &[&[u8]])] = &[
    (b"settings", &[b"kind", b"setting", b"revision", b"number", b"before"]),
    (b"propose_settings", &[b"against", b"set", b"reason"]),
    (b"delegate", &[b"batch"]),
    (b"message", &[b"target", b"form", b"words", b"question"]),
    (b"cancel", &[b"target", b"reason", b"grace"]),
    (b"release", &[b"target", b"revision", b"again"]),
    (b"introduce", &[b"left", b"right"]),
    (b"decide", &[b"kind", b"proposer", b"proposal", b"task", b"revision", b"decision", b"reason"]),
    (b"withdraw", &[b"proposal"]),
    (b"subscribe", &[b"kind", b"target", b"held", b"at", b"period"]),
    (b"unsubscribe", &[b"subscription"]),
    (b"amend", &[b"target", b"revision", b"amendment"]),
    (b"propose", &[b"action", b"reason", b"batch", b"target", b"revision", b"amendment", b"authority"]),
    (b"note", &[b"name", b"recalled", b"scope", b"description", b"body", b"references"]),
    (b"recall", &[b"kind", b"name", b"page", b"scopes", b"query"]),
];

pub(crate) const WORD_NUMBER: u64 = 1;
pub(crate) const SUBSCRIPTION: u64 = 4;
pub(crate) fn word(kind: jig_domain_tasks::MessageKind) -> jig_domain_tasks::Word {
    jig_domain_tasks::Word {
        number: WORD_NUMBER,
        from: jig_domain_tasks::Party::Task(TARGET),
        kind,
        words: b"Hello".as_slice().into(),
        at: skein_lib::Wall::EPOCH,
        hits: 1,
        eligible: true,
    }
}

pub const DELEGATE_NAMED: &[u8] = br#"{"batch":[{"executor":{"kind":"agent","charter":1},"spec":{"words":"check"},"contract":{"kind":"report","words":100},"authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":100},"notes":0},"label":7,"flags":{"conversation":true,"listed":true,"retention":"forgotten_once_read"},"tracked":4,"budget":{"as_allowed":{"cap":40}}},{"executor":{"kind":"agent","charter":1},"spec":{"words":"report"},"contract":{"kind":"report","words":100},"authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":100},"notes":0}}]}"#;
pub const TWO_AS_ALLOWED: &[u8] = br#"{"batch":[{"executor":{"kind":"agent","charter":1},"spec":{"words":"check"},"contract":{"kind":"report","words":100},"authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":100},"notes":0},"label":7,"flags":{"conversation":true,"listed":true,"retention":"forgotten_once_read"},"tracked":4,"budget":{"as_allowed":{"cap":40}}},{"executor":{"kind":"agent","charter":1},"spec":{"words":"report"},"contract":{"kind":"report","words":100},"authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":100},"notes":0},"budget":{"as_allowed":{}}}]}"#;
