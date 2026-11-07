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

    private static func wait(_ seconds: Double) async {
        try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
    }
}
