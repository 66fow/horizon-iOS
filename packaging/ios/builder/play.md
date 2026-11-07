# Playing builds inside Horizon Mods

Horizon Mods' **Play** tab gets Horizon builds (the fork's `ios-r<N>` and `ios-native-…` releases) into
`Documents/Builds/<build>/` and plays them inside itself, so Horizon Mods is the only app to sideload. It works like
an app launcher such as LiveContainer, without its code (LiveContainer is AGPL-3.0; this is written for Horizon, MIT):

1. **The certificate.** Settings → Signing certificate: *From SideStore* / *From AltStore* opens
   `<app>://certificate?callback_template=horizonmods://certificate?cert=$(BASE64_CERT)&password=$(PASSWORD)`, the
   sideloading apps' way of handing an app the certificate they sign it with; or a `.p12` file and its password.
   It works with a free Apple ID. It's kept in the keychain.
2. **Signing.** On Play, the signer (`signer/`, Rust, `apple-codesign`, MPL-2.0) turns the build's program into a
   library (`MH_EXECUTE` → `MH_DYLIB`, no PIE flag, an `LC_ID_DYLIB`, `__PAGEZERO` cut to one page) and signs it with
   the certificate, into `Builds/<build>/HorizonGame.dylib`. It's signed again when the certificate changes.
3. **Starting.** Horizon Mods writes the one wish "play this build" and opens itself again. At start, before its
   own screens, it reads (and removes) the wish, loads the library, makes the build's app its main bundle (its
   Info.plist, as if installed on its own) and runs the program's `main` (its `LC_MAIN` entry). The build runs in
   Horizon Mods' own folders: every build shares the game files (`resources/`), worlds, mods and packs.
4. **Back.** The wish is used once: close the game (swipe it away) and open Horizon Mods again.

No JIT is needed: everything is signed. A build can only be loaded on an iPhone (not the simulator), signed by the
team whose certificate signed Horizon Mods.
