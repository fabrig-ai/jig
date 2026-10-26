//! The seventh v19 policy edit keeps its tag and whole authority in the golden bytes.
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Reader, Writer};
#[test]
fn the_ceiling_edit_goldens_round_trip() {
    for bytes in [
        include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_policy_edit_ceiling_smallest.bin")
            .as_slice(),
        include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_policy_edit_ceiling_full.bin").as_slice(),
    ] {
        assert_eq!(bytes[0], 6, "the seventh edit appends without changing the old tags");
        let mut reader = Reader::new(bytes);
        let edit = w::PolicyEdit::decode(&w::CEILINGS, &mut reader).expect("committed ceiling golden");
        assert!(matches!(&edit, w::PolicyEdit::Ceiling(_)));
        assert!(reader.is_empty());
        let mut writer = Writer::new(usize::try_from(edit.measure()).expect("golden fits usize"));
        edit.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
}
