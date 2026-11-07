import SwiftUI

@main
struct HorizonModsApp: App {
    @StateObject private var store = Store()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(store)
                .tint(Theme.accentSoft)
                .preferredColorScheme(.dark)
        }
    }
}

struct RootView: View {
    @EnvironmentObject var store: Store
    @Environment(\.scenePhase) private var phase

    var body: some View {
        ZStack {
            SceneBackground()
            Group {
                switch store.tab {
                case .mods: ModsView()
                case .builds: BuildsView()
                case .instances: InstancesView()
                case .settings: SettingsView()
                }
            }
            .safeAreaInset(edge: .bottom, spacing: 0) {
                TabBar(tab: $store.tab, building: store.activeCount)
            }
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

/// The pages as tabs on a floating glass bar, the current one on the selected fill.
struct TabBar: View {
    @Binding var tab: AppTab
    let building: Int

    var body: some View {
        HStack(spacing: 4) {
            item(.mods, icon: "puzzle", label: "Mods")
            item(.builds, icon: "package-01", label: "Builds")
            item(.instances, icon: "game-controller-01", label: "Play")
            item(.settings, icon: "sliders-horizontal", label: "Settings")
        }
        .padding(5)
        .glass(Theme.Radius.panel)
        .padding(.horizontal, 20)
        .padding(.bottom, 4)
    }

    private func item(_ page: AppTab, icon: String, label: String) -> some View {
        let on = tab == page
        return Button {
            withAnimation(.easeOut(duration: 0.15)) { tab = page }
        } label: {
            VStack(spacing: 3) {
                HIcon(name: icon, size: 22)
                Text(label).font(.geist(11, .medium))
            }
            .foregroundStyle(Theme.ink(on ? 1 : 0.55))
            .frame(maxWidth: .infinity)
            .padding(.vertical, 7)
            .background(RoundedRectangle(cornerRadius: 15, style: .continuous).fill(Theme.fill(on ? 0.12 : 0)))
            .overlay(alignment: .topTrailing) {
                if page == .builds, building > 0 {
                    Text("\(building)")
                        .font(.geist(10, .semibold))
                        .foregroundStyle(Color.white)
                        .frame(minWidth: 17, minHeight: 17)
                        .background(Circle().fill(Theme.accent))
                        .offset(x: -10, y: 3)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
