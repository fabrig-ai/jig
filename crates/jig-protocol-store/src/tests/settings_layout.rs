//! E changes people 13 in place; pre-E layouts refuse before any header decode (store.md, 5.3).
use super::fixtures;
use crate::{Event, Family, Older, OpenRefusal, Request, keys};
use skein_lib::Token;
// jig 1d93fa987's people-v13 fingerprint, before the settings proposal arms.
const BEFORE_E: [u8; 32] = [
    0x7c, 0xc3, 0xe7, 0xac, 0x32, 0x63, 0x96, 0x6e, 0x53, 0xe0, 0x3e, 0x74, 0xf3, 0xa5, 0x53, 0x84, 0xcc, 0x70, 0x92,
    0x92, 0x4a, 0x6c, 0x89, 0xbe, 0xe9, 0xad, 0x58, 0xf2, 0x02, 0x72, 0x09, 0xcb,
];
#[test]
fn a_pre_settings_proposal_people_layout_refuses_as_older_people() {
    let mut layouts = crate::format::layouts();
    assert_ne!(layouts.people, BEFORE_E, "regenerate the changed current schemas together");
    layouts.people = BEFORE_E;
    let mut stores = fixtures::stores();
    fixtures::plant(
        &mut stores,
        &[
            fixtures::format_put(fixtures::DEPLOYMENT),
            skein_kv::Op::Put {
                key: keys::encode_own(&keys::Own::Layouts, fixtures::KEY_LIMIT).unwrap(),
                value: layouts.encode(),
            },
            fixtures::header_put(fixtures::DEPLOYMENT, 1),
            fixtures::installation_put(fixtures::INSTALLATION),
        ],
    );
    let mut store = fixtures::over(stores);
    let events = fixtures::run(
        &mut store,
        Request::Open { owner: Token::new(1), root: fixtures::ROOT, installation: fixtures::INSTALLATION },
    );
    let [Event::OpenRefused { why: OpenRefusal::OlderBuild { why: Older::Layout(Family::People) }, .. }] =
        events.as_ref()
    else {
        panic!("one older people layout refusal")
    };
}
