//! Makes a Horizon build's program loadable inside Horizon Mods (packaging/ios/builder/play.md).
//!
//! An iPhone app can load a library signed by its own team, not another program. So the build's
//! `MH_EXECUTE` program becomes an `MH_DYLIB`: not position-independent-executable any more, with a
//! library id, and its 4 GiB `__PAGEZERO` cut to one page (a library can't reserve the bottom
//! 4 GiB). Then it is signed with the player's certificate (the one their sideloading app signs
//! Horizon Mods with), so the iPhone lets Horizon Mods load it, and Horizon Mods jumps to its
//! `main` (its `LC_MAIN` entry), the way an app launcher like LiveContainer runs apps.
use apple_codesign::cryptography::{parse_pfx_data, InMemoryPrivateKey};
use apple_codesign::{AppleCertificate, MachOSigner, SettingsScope, SigningSettings};
use x509_certificate::CapturedX509Certificate;
use std::ffi::{c_char, CStr};

const MH_MAGIC_64: u32 = 0xfeed_facf;
const MH_EXECUTE: u32 = 2;
const MH_DYLIB: u32 = 6;
const MH_PIE: u32 = 0x0020_0000;
const LC_SEGMENT_64: u32 = 0x19;
const LC_ID_DYLIB: u32 = 0x0d;
const LC_LOAD_DYLINKER: u32 = 0x0e;
const LC_CODE_SIGNATURE: u32 = 0x1d;
const LC_MAIN: u32 = 0x8000_0028;
/// One 16 KiB page, just under 4 GiB, as the iPhone's dyld maps a library's zero page.
const PAGEZERO_ADDR: u64 = 0x1_0000_0000 - 0x4000;
const PAGEZERO_SIZE: u64 = 0x4000;
/// The library id's name.
const ID_NAME: &[u8] = b"@rpath/HorizonGame.dylib\0";

fn u32_at(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| "the program is cut short".into())
}

fn put_u32(d: &mut [u8], at: usize, v: u32) {
    d[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_u64(d: &mut [u8], at: usize, v: u64) {
    d[at..at + 8].copy_from_slice(&v.to_le_bytes());
}

/// An `LC_ID_DYLIB` command of `size` bytes.
fn id_dylib(size: usize) -> Vec<u8> {
    let mut c = vec![0u8; size];
    put_u32(&mut c, 0, LC_ID_DYLIB);
    put_u32(&mut c, 4, size as u32);
    put_u32(&mut c, 8, 24); // the name's offset, after the dylib struct
    put_u32(&mut c, 12, 2); // timestamp
    put_u32(&mut c, 16, 0x1_0000); // current version 1.0.0
    put_u32(&mut c, 20, 0x1_0000); // compatibility version 1.0.0
    c[24..24 + ID_NAME.len()].copy_from_slice(ID_NAME);
    c
}

/// The program `data` (a thin arm64 Mach-O) as a library, unsigned. A library already is left as
/// it is.
pub fn patch(mut d: Vec<u8>) -> Result<Vec<u8>, String> {
    if u32_at(&d, 0)? != MH_MAGIC_64 {
        return Err("not a 64-bit iPhone program (a universal one isn't supported)".into());
    }
    let filetype = u32_at(&d, 12)?;
    if filetype == MH_DYLIB {
        return Ok(d);
    }
    if filetype != MH_EXECUTE {
        return Err(format!("not a program (Mach-O type {filetype})"));
    }
    let ncmds = u32_at(&d, 16)? as usize;
    let sizeofcmds = u32_at(&d, 20)? as usize;
    let flags = u32_at(&d, 24)?;
    put_u32(&mut d, 12, MH_DYLIB);
    put_u32(&mut d, 24, flags & !MH_PIE);
    let mut at = 32;
    let mut first_data = usize::MAX;
    let mut dylinker = None;
    let mut has_main = false;
    for _ in 0..ncmds {
        let cmd = u32_at(&d, at)?;
        let size = u32_at(&d, at + 4)? as usize;
        if size < 8 || at + size > 32 + sizeofcmds {
            return Err("the program's load commands are damaged".into());
        }
        match cmd {
            LC_SEGMENT_64 => {
                let name = &d[at + 8..at + 24];
                if name.starts_with(b"__PAGEZERO\0") {
                    put_u64(&mut d, at + 24, PAGEZERO_ADDR);
                    put_u64(&mut d, at + 32, PAGEZERO_SIZE);
                }
                // Where the first section's bytes start: the load commands can grow up to it.
                let nsects = u32_at(&d, at + 64)? as usize;
                for s in 0..nsects {
                    let sect = at + 72 + s * 80;
                    let offset = u32_at(&d, sect + 48)? as usize;
                    if offset > 0 {
                        first_data = first_data.min(offset);
                    }
                }
            }
            LC_LOAD_DYLINKER => dylinker = Some((at, size)),
            LC_ID_DYLIB => return Err("the program already names itself a library".into()),
            LC_MAIN => has_main = true,
            LC_CODE_SIGNATURE | _ => {}
        }
        at += size;
    }
    if !has_main {
        return Err("the program has no main entry point".into());
    }
    let need = ID_NAME.len() + 24;
    let need = (need + 7) & !7;
    if first_data != usize::MAX && at + need <= first_data {
        // Room after the load commands: add the library id there.
        d[at..at + need].copy_from_slice(&id_dylib(need));
        put_u32(&mut d, 16, (ncmds + 1) as u32);
        put_u32(&mut d, 20, (sizeofcmds + need) as u32);
    } else if let Some((at, size)) = dylinker.filter(|&(_, size)| size >= need) {
        // No room: a library has no dynamic linker command, so the id takes its place.
        d[at..at + size].copy_from_slice(&id_dylib(size));
    } else {
        return Err("there's no room in the program for a library id".into());
    }
    Ok(d)
}

/// The certificate and private key in `p12`. p12-keystore reads both the classic format (SHA-1
/// MAC, 3DES/RC2), as Keychain Access writes, and OpenSSL 3's (SHA-256 MAC, AES), as most certificate
/// sellers hand out. apple-codesign's own reader knows only the classic one, and stops the program
/// on any other MAC, so it is only tried for an encryption p12-keystore doesn't know.
pub fn load(p12: &[u8], password: &str) -> Result<(CapturedX509Certificate, InMemoryPrivateKey), String> {
    if let Some(rest) = p12.strip_prefix(b"HMDER1") {
        return from_der(rest);
    }
    use p12_keystore::error::Error;
    let store = match p12_keystore::KeyStore::from_pkcs12(p12, password) {
        Ok(store) => store,
        Err(Error::MacError(_)) => return Err("The password is wrong for this .p12.".into()),
        Err(e @ (Error::UnsupportedEncryptionScheme | Error::UnsupportedContentType)) => {
            return parse_pfx_data(p12, password).map_err(|classic| format!("Couldn't read this .p12 ({e}; {classic})"));
        }
        Err(e) => return Err(format!("Couldn't read this .p12 ({e})")),
    };
    let (_, chain) = store
        .private_key_chain()
        .ok_or("This .p12 has no private key in it: export the certificate with its key.")?;
    let leaf = chain.chain().first().ok_or("This .p12 has no certificate in it.")?;
    let cert = CapturedX509Certificate::from_der(leaf.as_der().to_vec()).map_err(|e| e.to_string())?;
    let key = InMemoryPrivateKey::from_pkcs8_der(chain.key()).map_err(|e| e.to_string())?;
    Ok((cert, key))
}

/// What Horizon Mods sends once iOS has opened the .p12 itself (Signer.signingData): "HMDER1", then
/// the certificate's DER and the RSA key's PKCS #1 DER, each after its length (u32, little-endian).
fn from_der(d: &[u8]) -> Result<(CapturedX509Certificate, InMemoryPrivateKey), String> {
    fn part<'a>(d: &mut &'a [u8]) -> Result<&'a [u8], String> {
        let bad = || "the certificate from iOS is cut short".to_string();
        let n = u32::from_le_bytes(d.get(..4).ok_or_else(bad)?.try_into().unwrap()) as usize;
        let out = d.get(4..4 + n).ok_or_else(bad)?;
        *d = &d[4 + n..];
        Ok(out)
    }
    let mut d = d;
    let cert = part(&mut d)?;
    let key = part(&mut d)?;
    let cert = CapturedX509Certificate::from_der(cert.to_vec()).map_err(|e| e.to_string())?;
    let key = InMemoryPrivateKey::from_pkcs8_der(pkcs8_rsa(key)).map_err(|e| e.to_string())?;
    Ok((cert, key))
}

/// An RSA key's PKCS #1 DER (what iOS exports) wrapped as PKCS #8: version 0, rsaEncryption, the key.
fn pkcs8_rsa(pkcs1: &[u8]) -> Vec<u8> {
    fn tlv(tag: u8, body: &[u8]) -> Vec<u8> {
        let mut out = vec![tag];
        let n = body.len();
        if n < 0x80 {
            out.push(n as u8);
        } else {
            let bytes: Vec<u8> = n.to_be_bytes().into_iter().skip_while(|b| *b == 0).collect();
            out.push(0x80 | bytes.len() as u8);
            out.extend(bytes);
        }
        out.extend_from_slice(body);
        out
    }
    let algorithm = [
        0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00,
    ];
    let mut body = vec![0x02, 0x01, 0x00];
    body.extend_from_slice(&algorithm);
    body.extend(tlv(0x04, pkcs1));
    tlv(0x30, &body)
}

/// The certificate in `p12`: its subject's common name, its team and when it stops working.
pub fn certificate(p12: &[u8], password: &str) -> Result<(String, String, String), String> {
    let (cert, _) = load(p12, password)?;
    let name = cert.subject_common_name().unwrap_or_default();
    let team = cert.apple_team_id().unwrap_or_default();
    let until = cert.validity_not_after().to_rfc3339();
    Ok((name, team, until))
}

/// `program` as a library signed with the certificate in `p12`, for `identifier`.
pub fn prepare(program: Vec<u8>, p12: &[u8], password: &str, identifier: &str) -> Result<Vec<u8>, String> {
    let library = patch(program)?;
    let (cert, key) = load(p12, password)?;
    let team = cert.apple_team_id();
    let mut settings = SigningSettings::default();
    settings.set_signing_key(&key, cert);
    settings.chain_apple_certificates();
    // apple-codesign only reads the team from certificates Apple issued; the iPhone loads a library
    // only for the team that signed the app, so take it from the certificate's OU in any case.
    if settings.set_team_id_from_signing_certificate().is_none() {
        if let Some(team) = team {
            settings.set_team_id(team);
        }
    }
    settings.set_binary_identifier(SettingsScope::Main, identifier);
    let signer = MachOSigner::new(&library).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(library.len() + 1024 * 1024);
    signer
        .write_signed_binary(&settings, &mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// Copies `text` into the caller's buffer `out` of `len` bytes, ending it with a NUL.
unsafe fn write_text(text: &str, out: *mut c_char, len: usize) {
    if out.is_null() || len == 0 {
        return;
    }
    let bytes = text.as_bytes();
    let n = bytes.len().min(len - 1);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, n);
    *out.add(n) = 0;
}

unsafe fn text<'a>(p: *const c_char) -> &'a str {
    if p.is_null() {
        ""
    } else {
        CStr::from_ptr(p).to_str().unwrap_or("")
    }
}

/// For Horizon Mods (Swift): reads the program at `input`, writes the signed library to `output`.
/// 0 on success; otherwise the reason is in `error`.
///
/// # Safety
/// The strings are NUL-terminated, `p12` holds `p12_len` bytes, `error` has room for `error_len`.
#[no_mangle]
pub unsafe extern "C" fn hm_prepare(
    input: *const c_char,
    output: *const c_char,
    p12: *const u8,
    p12_len: usize,
    password: *const c_char,
    identifier: *const c_char,
    error: *mut c_char,
    error_len: usize,
) -> i32 {
    let p12 = if p12.is_null() { &[][..] } else { std::slice::from_raw_parts(p12, p12_len) };
    let result = std::fs::read(text(input))
        .map_err(|e| format!("reading the build: {e}"))
        .and_then(|program| prepare(program, p12, text(password), text(identifier)))
        .and_then(|signed| std::fs::write(text(output), signed).map_err(|e| format!("writing the library: {e}")));
    match result {
        Ok(()) => 0,
        Err(e) => {
            write_text(&e, error, error_len);
            1
        }
    }
}

/// For Horizon Mods (Swift): the certificate's name, team and end date, one per line, in `out`.
///
/// # Safety
/// As [`hm_prepare`].
#[no_mangle]
pub unsafe extern "C" fn hm_certificate(
    p12: *const u8,
    p12_len: usize,
    password: *const c_char,
    out: *mut c_char,
    out_len: usize,
) -> i32 {
    let p12 = if p12.is_null() { &[][..] } else { std::slice::from_raw_parts(p12, p12_len) };
    match certificate(p12, text(password)) {
        Ok((name, team, until)) => {
            write_text(&format!("{name}\n{team}\n{until}"), out, out_len);
            0
        }
        Err(e) => {
            write_text(&e, out, out_len);
            1
        }
    }
}
