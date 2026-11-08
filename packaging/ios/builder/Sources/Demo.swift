import SwiftUI

/// A scripted tour of the app for recordings (`-HorizonDemo YES`), and a mod page opened at start
/// for stills (`-HorizonDetail <mod id>` or `auto`). Picks made on the tour are put back after it.
enum Demo {
    @MainActor
    static func start(_ store: Store) async {
        let defaults = UserDefaults.standard
        let detail = defaults.string(forKey: "HorizonDetail")
        guard defaults.bool(forKey: "HorizonDemo") || detail != nil else { return }
        for _ in 0..<40 where store.mods.isEmpty { await wait(0.5) }
        guard !store.mods.isEmpty else { return }
        let pictured = store.mods.first { !$0.screenshots.isEmpty && $0.icon != nil } ?? store.mods[0]
        if let detail {
            await wait(2)
            store.demoDetail = detail == "auto" ? pictured.id : detail
            return
        }
        let before = store.picked
        store.picked = []
        let mods = store.mods
        await wait(3)
        for i in [5, 12, 20] where i < mods.count {
            store.demoScroll = mods[i].id
            await wait(2.2)
        }
        store.demoScroll = "top"
        await wait(2)
        for mod in mods.prefix(3) {
            withAnimation(.easeOut(duration: 0.15)) { store.toggle(mod) }
            await wait(0.8)
        }
        await wait(1.5)
        store.demoDetail = pictured.id
        await wait(5)
        store.demoDetail = nil
        await wait(1.5)
        for tab in [AppTab.create, .builds, .instances, .settings] {
            withAnimation(.easeOut(duration: 0.15)) { store.tab = tab }
            await wait(3)
        }
        withAnimation(.easeOut(duration: 0.15)) { store.tab = .mods }
        await wait(2)
        store.picked = before
    }

    /// One part of the install tutorial (`-HorizonTutorial settings|mods|builds|play`), recorded on its
    /// own in a simulator (.github/workflows/ios-mods-tutorial.yml).
    @MainActor
    static func tutorial(_ store: Store, _ library: Library) async {
        guard let part = Store.tutorial else { return }
        func type(_ text: String, into set: (String) -> Void) async {
            var typed = ""
            for c in text {
                typed.append(c)
                set(typed)
                await wait(0.07)
            }
        }
        switch part {
        case "settings":
            let repo = store.repo, token = store.token
            defer { store.repo = repo; store.token = token; store.demoCertificate = 0 }
            store.tab = .settings
            store.repo = ""
            store.token = ""
            await wait(2.5)
            await type("yourname/horizon-iOS") { store.repo = $0 }
            await wait(1.5)
            store.demoScroll = "maketoken"
            await wait(5)
            store.demoScroll = "token"
            await wait(2)
            await type("github_pat_11ABCDEFG0123456789") { store.token = $0 }
            await wait(2)
            store.demoScroll = "certificate"
            await wait(3)
            store.demoCertificate = 1
            await wait(3.5)
            store.demoCertificate = 2
            await wait(5)
        case "mods":
            let before = store.picked
            defer { store.picked = before }
            store.tab = .mods
            store.picked = []
            for _ in 0..<40 where store.mods.isEmpty { await wait(0.5) }
            await wait(2.5)
            let want = ["baritone", "appleskin", "chop", "mod_menu", "fps_overlay"]
            for id in want {
                guard let mod = store.mods.first(where: { $0.id == id }) else { continue }
                store.demoScroll = mod.id
                await wait(1.4)
                withAnimation(.easeOut(duration: 0.15)) { store.toggle(mod) }
                await wait(0.9)
            }
            store.demoScroll = "top"
            await wait(1.5)
            store.demoDetail = "baritone"
            await wait(5)
            store.demoDetail = nil
            await wait(1.5)
            store.demoBuildSheet = true
            await wait(6)
            store.demoBuildSheet = false
            await wait(2)
        case "builds":
            store.tab = .builds
            await wait(12)
        case "play":
            store.tab = .instances
            for _ in 0..<40 where library.remote.isEmpty { await wait(0.5) }
            await wait(3)
            let fresh = library.remote.filter { !library.isLocal($0) }
            if let build = fresh.first(where: { $0.tag.hasPrefix("ios-native") }) ?? fresh.first {
                await library.get(build)
            }
            await wait(5)
        default:
            break
        }
    }

    private static func wait(_ seconds: Double) async {
        try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
    }
}
