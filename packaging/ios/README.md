# Horizon on iPhone and iPad (experimental)

An early, unofficial iOS build. Over menus a tap clicks and a drag moves the cursor. In a world,
on-screen controls play: a stick on the left (push it all the way forward to sprint), drag
anywhere else to look, tap to use or place, hold still to break or attack, buttons for jump,
sneak (a toggle), hit, use, drop, chat, view, pause and inventory, and a tap on a hotbar slot
picks it. They press the buttons bound in Controls > Controller, so rebinding moves them too.

In a world, the Esc button (top left) is the keyboard's Escape: playing, it opens the pause menu
(Save and Quit leaves the world); over an inventory, a crafting table or chat, it closes it. The F3
button shows the debug screen. A tap away from the on-screen keyboard puts it away.

The pencil button (top right) opens the layout editor: drag any control, the stick and the hotbar
included, to move it; tap one, then Size, Opacity or Hide; Reset puts it back (with nothing picked,
the buttons change every control); Done keeps the layout in `touch-controls.txt` beside
`options.txt`.

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

Horizon ships none of Mojang's files. They are prepared from your own copy of Java Edition 26.3,
then copied into **On My iPhone → Horizon → resources** (the folder that holds
`horizon-resources.json`). Saves, `options.txt` and `logs/` are kept in the same Horizon folder.
Without them the app shows how to get them, with a Check again button.

**On a PC:** prepare them with the launcher or `horizon-prepare`, and copy the folder over with
the Files app or Finder/iTunes file sharing.

**With only a phone:** let GitHub prepare them, in a repository of your own that is **private**
(a public one would share Mojang's files):

1. Make a new private repository and add `packaging/ios/prepare/prepare-game-files.yml` to it as
   `.github/workflows/prepare-game-files.yml`.
2. Open **Actions → Prepare game files → Run workflow**, tick that you own Java Edition, run it.
   It downloads 26.3 from Mojang, checks it against `research-lock.json` and prepares it
   (about 20 minutes).
3. Open the release it makes in Safari, download `horizon-resources.zip`, tap it in the Files
   app to unzip it, and move the `resources` folder into **On My iPhone → Horizon**.
4. Open Horizon (close it first if it was showing the files screen).

## What is missing

- The PC launcher's pages: Microsoft sign-in (online servers), instances, the mod browser,
  modpacks, servers and the wardrobe. Single player and LAN play without them.
- An on-screen keyboard for text fields (world names, chat, server addresses).
- The system clipboard (UIPasteboard). Copied text stays in the game for now.
- Controllers: gilrs has no iOS backend (GameController.framework would be one).
- Preparing the resources on the device itself, without a PC and Java.
- Memory: an iPhone gives an app far less RAM than a PC, so render distance and the worker
  threads' share need iOS defaults.
- The main thread's stack is 1 MB on iOS and 8 MB elsewhere. If the game crashes at startup with
  a stack overflow, its startup has to move to a thread with a larger stack.
