import SwiftUI

@main
struct HorizonModsApp: App {
    @StateObject private var store = Store()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(store)
                .tint(.horizon)
        }
    }
}

extension Color {
    /// Horizon's purple, as on the game's icon.
    static let horizon = Color(red: 0.61, green: 0.55, blue: 1.0)
}

struct RootView: View {
    @EnvironmentObject var store: Store
    @Environment(\.scenePhase) private var phase

    var body: some View {
        TabView(selection: $store.tab) {
            ModsView()
                .tabItem { Label("Mods", systemImage: "puzzlepiece.extension") }
                .tag(AppTab.mods)
            BuildsView()
                .tabItem { Label("Builds", systemImage: "hammer") }
                .badge(store.activeCount)
                .tag(AppTab.builds)
            SettingsView()
                .tabItem { Label("Settings", systemImage: "gearshape") }
                .tag(AppTab.settings)
        }
        .task { await store.loadMods() }
        // Builds are checked while the app is open: often while one runs, rarely otherwise.
        .task(id: phase) {
            guard phase == .active else { return }
            while !Task.isCancelled {
                await store.refreshRuns()
                let seconds: UInt64 = store.activeCount > 0 ? 15 : 90
                try? await Task.sleep(nanoseconds: seconds * 1_000_000_000)
            }
        }
    }
}

struct ErrorRow: View {
    let message: String
    let retry: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label(message, systemImage: "exclamationmark.triangle.fill")
                .font(.subheadline)
                .foregroundStyle(.orange)
            Button("Try again", action: retry)
                .buttonStyle(.bordered)
        }
        .padding(.vertical, 4)
    }
}
