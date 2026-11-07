//! A Horizon build's program, made a library and signed, checked as the iPhone's loader reads it.
//! Needs a real build: HORIZON_TEST_PROGRAM (its `horizon-client`) and HORIZON_TEST_P12 (a .p12,
//! password HORIZON_TEST_P12_PASSWORD); skipped without them.
use apple_codesign::{MachFile, verify_macho_data};

const MH_DYLIB: u32 = 6;
const MH_PIE: u32 = 0x0020_0000;

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(d[at..at + 4].try_into().unwrap())
}

fn u64_at(d: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(d[at..at + 8].try_into().unwrap())
}

/// Each load command: (cmd, offset).
fn commands(d: &[u8]) -> Vec<(u32, usize)> {
    let mut at = 32;
    (0..u32_at(d, 16))
        .map(|_| {
            let c = (u32_at(d, at), at);
            at += u32_at(d, at + 4) as usize;
            c
        })
        .collect()
}

#[test]
fn a_build_becomes_a_signed_library_with_one_zero_page_and_its_entry_kept() {
    let (Ok(program), Ok(p12)) = (std::env::var("HORIZON_TEST_PROGRAM"), std::env::var("HORIZON_TEST_P12")) else {
        eprintln!("skipped: no HORIZON_TEST_PROGRAM / HORIZON_TEST_P12");
        return;
    };
    let password = std::env::var("HORIZON_TEST_P12_PASSWORD").unwrap_or_default();
    let original = std::fs::read(program).unwrap();
    let signed =
        horizon_mods_signer::prepare(original.clone(), &std::fs::read(p12).unwrap(), &password, "dev.horizon.game")
            .unwrap();

    assert_eq!(u32_at(&signed, 12), MH_DYLIB);
    assert_eq!(u32_at(&signed, 24) & MH_PIE, 0);
    let cmds = commands(&signed);
    let id = cmds.iter().filter(|(c, _)| *c == 0x0d).count();
    assert_eq!(id, 1, "one library id");
    let (_, zero) = cmds
        .iter()
        .find(|(c, at)| *c == 0x19 && signed[at + 8..at + 24].starts_with(b"__PAGEZERO\0"))
        .copied()
        .unwrap();
    assert_eq!((u64_at(&signed, zero + 24), u64_at(&signed, zero + 32)), (0xFFFF_C000, 0x4000));
    // The entry point the launcher jumps to is the program's own.
    let entry = |d: &[u8]| {
        commands(d).iter().find(|(c, _)| *c == 0x8000_0028).map(|(_, at)| u64_at(d, at + 8)).unwrap()
    };
    assert_eq!(entry(&signed), entry(&original));
    // The signature covers the patched library, with the certificate's CMS blob.
    let problems = verify_macho_data(&signed);
    let real: Vec<_> = problems
        .iter()
        .map(|p| format!("{p:?}"))
        .filter(|p| !p.contains("NoCryptographicSignature") && !p.to_lowercase().contains("certificate"))
        .collect();
    assert!(real.is_empty(), "{real:#?}");
    let macho = MachFile::parse(&signed).unwrap().into_iter().next().unwrap();
    let signature = macho.code_signature().unwrap().unwrap();
    assert!(signature.signature_data().unwrap().is_some(), "a CMS signature, not ad hoc");
    let cd = signature.code_directory().unwrap().unwrap();
    assert_eq!(cd.ident.as_ref(), "dev.horizon.game");
    assert_eq!(cd.team_name.as_deref(), Some("ABCDE12345"));
}
