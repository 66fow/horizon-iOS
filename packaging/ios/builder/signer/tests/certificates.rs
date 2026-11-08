//! Both kinds of .p12 a player brings: the classic one Keychain Access writes (SHA-1 MAC, 3DES and
//! RC2) and OpenSSL 3's (SHA-256 MAC, AES-256), which certificate sellers usually hand out. Made
//! with `openssl pkcs12 -export [-legacy] -passout pass:horizon` from a throwaway self-signed key.
use horizon_mods_signer::{certificate, load};

const CLASSIC: &[u8] = include_bytes!("fixtures/classic.p12");
const MODERN: &[u8] = include_bytes!("fixtures/modern.p12");
/// MODERN with its PBKDF2 PRFs' NULL parameters left out (and its MAC made again), as some
/// certificate sellers' .p12s are: pkcs5 alone calls that "malformed ASN.1 DER value for NULL".
const NO_PRF_NULL: &[u8] = include_bytes!("fixtures/no-prf-null.p12");

#[test]
fn both_formats_open_with_their_password() {
    for p12 in [CLASSIC, MODERN, NO_PRF_NULL] {
        let (name, team, _) = certificate(p12, "horizon").unwrap();
        assert_eq!(name, "Apple Development: Horizon Test (ABCDE12345)");
        assert_eq!(team, "ABCDE12345");
        // The private key too, which signing needs.
        load(p12, "horizon").unwrap();
    }
}

#[test]
fn a_wrong_password_says_so() {
    for p12 in [CLASSIC, MODERN, NO_PRF_NULL] {
        let error = certificate(p12, "not it").unwrap_err();
        assert!(error.to_lowercase().contains("password"), "{error}");
    }
}

/// What Horizon Mods sends after iOS opened the .p12: the certificate and the key's PKCS #1 DER.
/// `cert.der` and `key.der` are the fixtures' certificate and key, exported with openssl.
#[test]
fn a_certificate_ios_opened_signs_too() {
    let cert = include_bytes!("fixtures/cert.der");
    let key = include_bytes!("fixtures/key.der");
    let mut blob = b"HMDER1".to_vec();
    for part in [&cert[..], &key[..]] {
        blob.extend((part.len() as u32).to_le_bytes());
        blob.extend_from_slice(part);
    }
    let (name, team, _) = certificate(&blob, "").unwrap();
    assert_eq!(name, "Apple Development: Horizon Test (ABCDE12345)");
    assert_eq!(team, "ABCDE12345");
    load(&blob, "").unwrap();
    assert!(certificate(&blob[..blob.len() - 10], "").is_err());
}
