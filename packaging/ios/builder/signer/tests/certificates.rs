//! Both kinds of .p12 a player brings: the classic one Keychain Access writes (SHA-1 MAC, 3DES and
//! RC2) and OpenSSL 3's (SHA-256 MAC, AES-256), which certificate sellers usually hand out. Made
//! with `openssl pkcs12 -export [-legacy] -passout pass:horizon` from a throwaway self-signed key.
use horizon_mods_signer::{certificate, load};

const CLASSIC: &[u8] = include_bytes!("fixtures/classic.p12");
const MODERN: &[u8] = include_bytes!("fixtures/modern.p12");

#[test]
fn both_formats_open_with_their_password() {
    for p12 in [CLASSIC, MODERN] {
        let (name, team, _) = certificate(p12, "horizon").unwrap();
        assert_eq!(name, "Apple Development: Horizon Test (ABCDE12345)");
        assert_eq!(team, "ABCDE12345");
        // The private key too, which signing needs.
        load(p12, "horizon").unwrap();
    }
}

#[test]
fn a_wrong_password_says_so() {
    for p12 in [CLASSIC, MODERN] {
        let error = certificate(p12, "not it").unwrap_err();
        assert!(error.to_lowercase().contains("password"), "{error}");
    }
}
