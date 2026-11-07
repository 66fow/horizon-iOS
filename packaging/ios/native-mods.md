# Native mods on iOS

Native mods change Horizon's own Rust source before it is compiled (docs/native-mods.md). An iPhone
can't compile, so GitHub does it, into a custom `.ipa`:

1. **Pick the mods.** In the fork's **Native mods for iOS** issue, tick the mods, then tick
   **Build now**. (Or run **Actions → iOS build with native mods** and type their ids.) The issue's
   list is refreshed from the registry every day (`ios-native-list.yml`); run that workflow once to
   make the issue. Issues must be on in the fork (Settings → General → Features → Issues).
2. **GitHub builds it** (`ios-native-build.yml`): the mods are downloaded from Horizon's registry and
   checked against its SHA-256 and size, then the launcher's own native staging
   (`horizon-launcher native-stage`, the steps `native-build` runs before it compiles: horizon-mixins,
   trait injections, the mods' crates) applies them to the `ios-build` branch's source, and a Mac
   compiles it for iOS. Every mod is built on this branch's commit, whatever Horizon it was made for:
   one that no longer applies fails the build, and the issue says so.
3. **Sideload** the `.ipa` from the release linked in the issue, over Horizon: worlds and files stay.

Native mods are not sandboxed: their code runs with full access to the app. The registry reviews each
one before listing it.
