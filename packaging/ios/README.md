# Horizon on iPhone and iPad (experimental)

An early, unofficial iOS build. It starts the game and shows its menus. A tap clicks and a drag
moves the cursor, so menu buttons work. Playing in a world needs touch controls that do not exist
yet (see "What is missing" below).

## Get the .ipa

`.github/workflows/ios-build.yml` builds it on GitHub's macOS runners, so you don't need a Mac.
On a fork, open **Actions → iOS build → Run workflow**. When it finishes, download the
`Horizon-ios` artifact from the run's page and unzip it to get `Horizon.ipa`.

## Install it from Windows or Linux

1. Install [Sideloadly](https://sideloadly.io) (Windows/macOS) or AltStore, and iTunes on
   Windows (it supplies the iPhone's USB driver).
2. Connect the iPhone by USB, drop `Horizon.ipa` on Sideloadly and sign in with your Apple ID.
3. On the iPhone: **Settings → General → VPN & Device Management**, then trust your Apple ID.
   On iOS 16 and later, also turn on **Settings → Privacy & Security → Developer Mode**.

A free Apple ID's signature lasts 7 days: sideload again (your saves stay) to keep playing.

## Give it the game's resources

Horizon ships none of Mojang's files. Prepare them on a PC from your own copy of Java Edition
26.3 with `horizon-prepare` (or the launcher). Then copy the prepared folder, the one that holds
`horizon-resources.json`, onto the iPhone with the Files app or Finder/iTunes file sharing, as
**On My iPhone → Horizon → resources**. Saves and `options.txt` are kept in the same Horizon folder.

## What is missing

- Touch controls in the world: a movement stick, looking around by dragging, jump, sneak,
  attack, use and hotbar buttons, and a way to open the inventory and chat.
- An on-screen keyboard for text fields (world names, chat, server addresses).
- The system clipboard (UIPasteboard). Copied text stays in the game for now.
- Controllers: gilrs has no iOS backend (GameController.framework would be one).
- Preparing the resources on the device itself, without a PC and Java.
- Memory: an iPhone gives an app far less RAM than a PC, so render distance and the worker
  threads' share need iOS defaults.
- The main thread's stack is 1 MB on iOS and 8 MB elsewhere. If the game crashes at startup with
  a stack overflow, its startup has to move to a thread with a larger stack.
