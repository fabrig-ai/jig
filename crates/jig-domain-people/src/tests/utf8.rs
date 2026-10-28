use super::*;

#[test]
fn non_utf8_words_are_refused_before_any_request_is_kept_or_routed() {
    let mut test = Test::new(LIMITS);
    test.member();
    for words in [Box::from([255]), Box::from([0xc3]), Box::from([0xc3, b'a'])] {
        let requests = test.request(10, 1, create_request(1, words));
        assert_eq!(requests.len(), 1);
        assert_eq!(reply(&requests), Reply::Refused(Refusal::Limit { which: LimitKind::Words, bound: None }));
    }
    let requests = test.request(10, 1, create_request(1, Box::from("é".as_bytes())));
    assert_eq!(requests.len(), 1);
    let request = route(&requests);
    assert_ne!(request, Token::new(u64::MAX));
}
