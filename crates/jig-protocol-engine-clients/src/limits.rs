//! Connection and exchange admission (protocol/client.md, sections 13 and 18).
use skein_lib::Duration;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub streams: u32,
    pub watches_per_stream: u32,
    pub queued_bytes: u32,
    pub identity_bytes: u32,
    pub families: u32,
    pub providers: u32,
    pub exchanges: u32,
    pub mints: u32,
    /// Maximum volatile authorization codes.
    pub provider_codes: u32,
    /// Maximum volatile provider access tokens.
    pub provider_tokens: u32,
    pub exchanges_per_client: u32,
    /// A message's bytes, owned by the component (protocol/client.md, 4.2 and 13).
    pub message_bytes: u32,
    pub wire: jig_client_protocol_wire::v19::Limits,
    pub link_span: Duration,
}
pub const MAX_OUT_DOWN: u32 = 4;
pub const MAX_OUT_UP: u32 = 3;
pub const MAX_OUT_FIRE: u32 = 1;
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.wire.signed_in_token_some < 40
        || limits.wire.made_service_credential < 40
        || limits.wire.link_sign_in_secret < 32
        || limits.identity_bytes < 8
        || limits.streams == 0
        || limits.watches_per_stream == 0
        || limits.queued_bytes < 128
        || limits.provider_codes == 0
        || limits.provider_tokens == 0
        || limits.mints == 0
        || limits.message_bytes == 0
        || limits.exchanges == 0
        || limits.exchanges_per_client == 0
        || limits.link_span == Duration::ZERO
    {
        return false;
    }
    jig_client_protocol_wire::v19::check_limits(&limits.wire).is_ok()
        && jig_client_protocol_wire::v19::KeyedAnswer::worst_case_bytes(&limits.wire).is_some()
        && jig_client_protocol_wire::v19::ReadAnswer::worst_case_bytes(&limits.wire).is_some()
        && jig_client_protocol_wire::v19::AuthorizeAnswer::worst_case_bytes(&limits.wire).is_some()
        && limits.exchanges.checked_add(limits.exchanges).is_some()
        && provider_bounds(&provider_limits(limits)).is_some()
}

// The issuer has no validity API at this pin. Its document encoder uses u32
// lengths, and form parsing derives one slot beyond the request's byte cap.
fn provider_bounds(limits: &skein_oauth_issuer::Limits) -> Option<u32> {
    let claims = u64::from(limits.claim_bytes)
        .checked_mul(4)?
        .checked_add(u64::from(limits.uri_bytes))?
        .checked_add(u64::from(limits.client_bytes))?
        .checked_add(u64::from(limits.state_bytes))?
        .checked_mul(6)?
        .checked_add(1024)?;
    let jwt = claims.checked_add(2)?.checked_div(3)?.checked_mul(4)?.checked_add(1024)?;
    let answer = jwt.checked_add(u64::from(limits.token_bytes).checked_mul(6)?)?.checked_add(128)?;
    u32::try_from(answer).ok()?;
    u32::try_from(u64::from(limits.uri_bytes).checked_mul(30)?.checked_add(2048)?).ok()?;
    limits.request_bytes.checked_add(1)
}

/// Issuer bounds shared with configuration's startup helper (client.md, 18).
#[must_use]
pub fn provider_limits(limits: &Limits) -> skein_oauth_issuer::Limits {
    skein_oauth_issuer::Limits {
        clients: 256,
        uri_bytes: 2048,
        client_bytes: 255,
        request_bytes: limits.message_bytes.min(limits.wire.authorize_query),
        state_bytes: 1024,
        token_bytes: 43,
        claim_bytes: limits.identity_bytes.saturating_add(6),
    }
}
