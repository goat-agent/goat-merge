use goat_merge::github::webhook::{sign, signature_holds};

const SECRET: &str = "a shared secret";

#[test]
fn a_body_signed_with_the_secret_is_accepted() {
    let body = br#"{"action":"labeled"}"#;

    assert!(signature_holds(SECRET, body, &sign(SECRET, body)));
}

#[test]
fn a_body_signed_with_another_secret_is_refused() {
    let body = br#"{"action":"labeled"}"#;

    assert!(!signature_holds(
        SECRET,
        body,
        &sign("some other secret", body)
    ));
}

#[test]
fn changing_one_byte_of_the_body_breaks_the_signature() {
    let signed = sign(SECRET, br#"{"action":"labeled"}"#);

    assert!(
        !signature_holds(SECRET, br#"{"action":"unlabeled"}"#, &signed),
        "a webhook body that was edited on the way here must not be acted on"
    );
}

#[test]
fn a_signature_without_the_algorithm_is_refused() {
    let body = br#"{}"#;
    let signed = sign(SECRET, body);
    let bare = signed.strip_prefix("sha256=").unwrap_or_default();

    assert!(!signature_holds(SECRET, body, bare));
}

#[test]
fn nonsense_in_the_signature_header_is_refused() {
    for claimed in ["", "sha256=", "sha256=zz", "sha1=abcdef", "sha256=abc"] {
        assert!(
            !signature_holds(SECRET, br#"{}"#, claimed),
            "{claimed:?} should not be treated as a signature"
        );
    }
}
