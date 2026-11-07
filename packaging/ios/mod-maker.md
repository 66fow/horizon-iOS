# The mod maker (Horizon Mods, iOS)

Horizon Mods' **Create** tab writes mods with an AI and has GitHub check them, so a mod can be made on an iPhone.

- **The AI.** Sign in with ChatGPT (OpenAI's "Sign in with ChatGPT": the player's Go, Plus or Pro plan, OAuth 2
  with PKCE, the client registered on first sign-in, the redirect caught by a server on `127.0.0.1:1455` inside the
  app), or an API key for Claude, OpenAI or Gemini. Claude and Gemini subscriptions are not offered: Anthropic's terms
  forbid using a Claude.ai login in other apps, and Google has no allowed way for apps to use a Gemini plan. Keys and
  tokens stay in the keychain. **GitHub Copilot** uses the player's Copilot plan through GitHub's Copilot CLI, which
  can't run on an iPhone, so it runs in `ios-copilot.yml`: the app commits the turn to the user-mods branch as
  `.chats/<chat>/<turn>-request.md`, Copilot reads it and Horizon's docs and source (read-only tools), and the answer
  is committed beside it as `<turn>-answer.md`. It needs the repository secret `COPILOT_TOKEN` (a fine-grained token
  with the Copilot Requests permission). GitHub Models, the old free API, was retired on 2026-07-30.
- **What it's told.** Horizon's own documentation and source from this fork's `ios-build` branch: for a content mod
  the manifest and content sections of `docs/mods.md`, `docs/modpacks.md`'s Registries 0.2 and the
  `cargo horizon-mod new` template; for a native mod `docs/native-mods.md`'s Trait mods, the native template and
  `crates/horizon-client/src/mod_api.rs`. It answers with the whole mod as one ` ```horizon-mod ` JSON block
  (`id`, `name`, `version`, `summary`, `files`); a texture can be a palette and rows of characters, drawn into a PNG
  by the app.
- **Checking.** Upload commits the files to `user-mods/<id>/` on the fork's `user-mods` branch (an orphan branch
  with only mods) and runs `ios-user-mod.yml`: a content mod is packed and checked with `cargo horizon-mod build`
  and `check` (the registry's checks); a native mod gets its `horizon-native.json`, is staged onto `ios-build` by the
  launcher's `native-stage`, and the game is type-checked with it. A pass is published as the pre-release
  `mod-<id>-v<version>` (`<id>.hmod` or `<id>.hnmod`, a `<!-- horizon-user-mod {…} -->` line in its notes). A
  failure's errors are read from the job's log and sent back to the AI to fix.
- **Using it.** A content mod installs from Horizon's Get mods → My mods. A native mod shows in Horizon Mods' Mods
  tab under "Yours", and native builds (`ios-native-build.yml`) fetch it from the fork's release by its id.
- **Sharing it.** Submit to registry makes the mod its own public repository (`horizon-mod-<id>`, MIT, with a
  release holding the checked file), as Horizon's registry lists projects, and opens playhorizon.dev/mods/submit,
  where the player signs in with GitHub and sends it for review. The token needs All repositories access with
  Administration and Contents: Read and write for that.
