//! A versioned turn with lengths and CRC-32C (protocol/store.md, 8.1).
mod patch;
use crate::codec;
use alloc::boxed::Box;
use jig_domain_core as core;
pub(crate) use patch::{Decode, Patch, Progress};
use skein_lib::{Reader, Writer};
// Leading length, CRC-32C and trailing length, each four bytes (store.md 8.1).
pub(crate) const WRAPPER_BYTES: u32 = 12;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unread {
    Short,
    Lengths,
    Checksum,
    Record,
}
#[must_use]
pub fn encode(turn: &core::TurnRecord, limits: &codec::Limits) -> Option<Box<[u8]>> {
    let body = codec::core::turn(turn, &codec::core_limits(limits)).ok()?;
    let length = u32::try_from(body.len()).ok()?.checked_add(WRAPPER_BYTES)?;
    let mut writer = Writer::new(usize::try_from(length).ok()?);
    writer.put(&length.to_be_bytes()).ok()?;
    writer.put(&body).ok()?;
    writer.put(&crc32c(&body).to_be_bytes()).ok()?;
    writer.put(&length.to_be_bytes()).ok()?;
    Some(writer.finish())
}
/// Decode exactly one whole frame, refusing trailing bytes as well.
pub fn decode(bytes: &[u8], limits: &codec::Limits) -> Result<core::TurnRecord, Unread> {
    let body = body(bytes)?;
    match codec::core::read_turn(body, &codec::core_limits(limits)) {
        Ok(turn) => Ok(turn),
        Err(_) => Err(Unread::Record),
    }
}
pub(crate) fn body(bytes: &[u8]) -> Result<&[u8], Unread> {
    if u32::try_from(bytes.len()).is_err() {
        return Err(Unread::Lengths);
    }
    let mut reader = Reader::new(bytes);
    let length = reader.u32().ok_or(Unread::Short)?;
    let body_length = length.checked_sub(WRAPPER_BYTES).ok_or(Unread::Lengths)?;
    let body = reader.bytes(body_length).ok_or(Unread::Short)?;
    let checksum = reader.u32().ok_or(Unread::Short)?;
    let trailing = reader.u32().ok_or(Unread::Short)?;
    if trailing != length || !reader.is_empty() {
        return Err(Unread::Lengths);
    }
    if crc32c(body) != checksum {
        return Err(Unread::Checksum);
    }
    Ok(body)
}
/// CRC-32C's reflected polynomial, `0x82f6_3b78`, advanced eight bits per entry.
#[rustfmt::skip]
const TABLE: [u32; 256] = [
    0x0000_0000, 0xf26b_8303, 0xe13b_70f7, 0x1350_f3f4, 0xc79a_971f, 0x35f1_141c, 0x26a1_e7e8, 0xd4ca_64eb,
    0x8ad9_58cf, 0x78b2_dbcc, 0x6be2_2838, 0x9989_ab3b, 0x4d43_cfd0, 0xbf28_4cd3, 0xac78_bf27, 0x5e13_3c24,
    0x105e_c76f, 0xe235_446c, 0xf165_b798, 0x030e_349b, 0xd7c4_5070, 0x25af_d373, 0x36ff_2087, 0xc494_a384,
    0x9a87_9fa0, 0x68ec_1ca3, 0x7bbc_ef57, 0x89d7_6c54, 0x5d1d_08bf, 0xaf76_8bbc, 0xbc26_7848, 0x4e4d_fb4b,
    0x20bd_8ede, 0xd2d6_0ddd, 0xc186_fe29, 0x33ed_7d2a, 0xe727_19c1, 0x154c_9ac2, 0x061c_6936, 0xf477_ea35,
    0xaa64_d611, 0x580f_5512, 0x4b5f_a6e6, 0xb934_25e5, 0x6dfe_410e, 0x9f95_c20d, 0x8cc5_31f9, 0x7eae_b2fa,
    0x30e3_49b1, 0xc288_cab2, 0xd1d8_3946, 0x23b3_ba45, 0xf779_deae, 0x0512_5dad, 0x1642_ae59, 0xe429_2d5a,
    0xba3a_117e, 0x4851_927d, 0x5b01_6189, 0xa96a_e28a, 0x7da0_8661, 0x8fcb_0562, 0x9c9b_f696, 0x6ef0_7595,
    0x417b_1dbc, 0xb310_9ebf, 0xa040_6d4b, 0x522b_ee48, 0x86e1_8aa3, 0x748a_09a0, 0x67da_fa54, 0x95b1_7957,
    0xcba2_4573, 0x39c9_c670, 0x2a99_3584, 0xd8f2_b687, 0x0c38_d26c, 0xfe53_516f, 0xed03_a29b, 0x1f68_2198,
    0x5125_dad3, 0xa34e_59d0, 0xb01e_aa24, 0x4275_2927, 0x96bf_4dcc, 0x64d4_cecf, 0x7784_3d3b, 0x85ef_be38,
    0xdbfc_821c, 0x2997_011f, 0x3ac7_f2eb, 0xc8ac_71e8, 0x1c66_1503, 0xee0d_9600, 0xfd5d_65f4, 0x0f36_e6f7,
    0x61c6_9362, 0x93ad_1061, 0x80fd_e395, 0x7296_6096, 0xa65c_047d, 0x5437_877e, 0x4767_748a, 0xb50c_f789,
    0xeb1f_cbad, 0x1974_48ae, 0x0a24_bb5a, 0xf84f_3859, 0x2c85_5cb2, 0xdeee_dfb1, 0xcdbe_2c45, 0x3fd5_af46,
    0x7198_540d, 0x83f3_d70e, 0x90a3_24fa, 0x62c8_a7f9, 0xb602_c312, 0x4469_4011, 0x5739_b3e5, 0xa552_30e6,
    0xfb41_0cc2, 0x092a_8fc1, 0x1a7a_7c35, 0xe811_ff36, 0x3cdb_9bdd, 0xceb0_18de, 0xdde0_eb2a, 0x2f8b_6829,
    0x82f6_3b78, 0x709d_b87b, 0x63cd_4b8f, 0x91a6_c88c, 0x456c_ac67, 0xb707_2f64, 0xa457_dc90, 0x563c_5f93,
    0x082f_63b7, 0xfa44_e0b4, 0xe914_1340, 0x1b7f_9043, 0xcfb5_f4a8, 0x3dde_77ab, 0x2e8e_845f, 0xdce5_075c,
    0x92a8_fc17, 0x60c3_7f14, 0x7393_8ce0, 0x81f8_0fe3, 0x5532_6b08, 0xa759_e80b, 0xb409_1bff, 0x4662_98fc,
    0x1871_a4d8, 0xea1a_27db, 0xf94a_d42f, 0x0b21_572c, 0xdfeb_33c7, 0x2d80_b0c4, 0x3ed0_4330, 0xccbb_c033,
    0xa24b_b5a6, 0x5020_36a5, 0x4370_c551, 0xb11b_4652, 0x65d1_22b9, 0x97ba_a1ba, 0x84ea_524e, 0x7681_d14d,
    0x2892_ed69, 0xdaf9_6e6a, 0xc9a9_9d9e, 0x3bc2_1e9d, 0xef08_7a76, 0x1d63_f975, 0x0e33_0a81, 0xfc58_8982,
    0xb215_72c9, 0x407e_f1ca, 0x532e_023e, 0xa145_813d, 0x758f_e5d6, 0x87e4_66d5, 0x94b4_9521, 0x66df_1622,
    0x38cc_2a06, 0xcaa7_a905, 0xd9f7_5af1, 0x2b9c_d9f2, 0xff56_bd19, 0x0d3d_3e1a, 0x1e6d_cdee, 0xec06_4eed,
    0xc38d_26c4, 0x31e6_a5c7, 0x22b6_5633, 0xd0dd_d530, 0x0417_b1db, 0xf67c_32d8, 0xe52c_c12c, 0x1747_422f,
    0x4954_7e0b, 0xbb3f_fd08, 0xa86f_0efc, 0x5a04_8dff, 0x8ece_e914, 0x7ca5_6a17, 0x6ff5_99e3, 0x9d9e_1ae0,
    0xd3d3_e1ab, 0x21b8_62a8, 0x32e8_915c, 0xc083_125f, 0x1449_76b4, 0xe622_f5b7, 0xf572_0643, 0x0719_8540,
    0x590a_b964, 0xab61_3a67, 0xb831_c993, 0x4a5a_4a90, 0x9e90_2e7b, 0x6cfb_ad78, 0x7fab_5e8c, 0x8dc0_dd8f,
    0xe330_a81a, 0x115b_2b19, 0x020b_d8ed, 0xf060_5bee, 0x24aa_3f05, 0xd6c1_bc06, 0xc591_4ff2, 0x37fa_ccf1,
    0x69e9_f0d5, 0x9b82_73d6, 0x88d2_8022, 0x7ab9_0321, 0xae73_67ca, 0x5c18_e4c9, 0x4f48_173d, 0xbd23_943e,
    0xf36e_6f75, 0x0105_ec76, 0x1255_1f82, 0xe03e_9c81, 0x34f4_f86a, 0xc69f_7b69, 0xd5cf_889d, 0x27a4_0b9e,
    0x79b7_37ba, 0x8bdc_b4b9, 0x988c_474d, 0x6ae7_c44e, 0xbe2d_a0a5, 0x4c46_23a6, 0x5f16_d052, 0xad7d_5351,
];
#[must_use]
pub fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        let index = usize::try_from((crc ^ u32::from(*byte)) & 0xff).expect("one byte fits usize");
        crc = (crc >> 8_u32) ^ *TABLE.get(index).expect("one table entry per byte");
    }
    !crc
}
