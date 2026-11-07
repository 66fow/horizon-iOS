import SwiftUI
import UIKit

/// Where a build goes: a new instance, or an update of one already built.
struct BuildSheet: View {
    @EnvironmentObject var store: Store
    @Environment(\.dismiss) private var dismiss
    @State private var target: String?  // nil: a new instance
    @State private var name = ""

    private var suggested: String {
        let first = store.pickedInOrder.first.map { store.names([$0]) } ?? ""
        return first.isEmpty ? "Horizon" : String("Horizon \(first)".prefix(30))
    }

    var body: some View {
        ZStack {
            SceneBackground()
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    HStack {
                        Text("Build").font(.display(34)).foregroundStyle(Theme.ink(1))
                        Spacer()
                        Button { dismiss() } label: {
                            HIcon(name: "cancel-01", size: 18).foregroundStyle(Theme.ink(0.9))
                                .padding(11).glass(Theme.Radius.row)
                        }
                        .buttonStyle(.plain)
                    }
                    Plate("Mods") {
                        Text(store.names(store.pickedInOrder))
                            .font(.geist(14, .medium))
                            .foregroundStyle(Theme.ink(0.85))
                    }
                    Plate("Into") {
                        choice(nil, title: "A new instance",
                               detail: "Its own app on your home screen, beside Horizon and the others.")
                        if target == nil {
                            FieldBox {
                                HIcon(name: "pencil-edit-02", size: 16).foregroundStyle(Theme.ink(0.5))
                                TextField("", text: $name, prompt: Text(suggested).foregroundColor(Theme.ink(0.4)))
                                    .font(.geist(15))
                                    .foregroundStyle(Theme.ink(1))
                            }
                        }
                        ForEach(store.shownInstances) { instance in
                            choice(instance.id, title: "Update “\(instance.name)”",
                                   detail: "Replaces build \(instance.build). Its worlds and files stay.")
                        }
                    }
                    Text("Native mods change Horizon's own code and run with full access to the app. Only build mods you trust."
                         + (store.activeCount > 0 ? " A build is already running: starting this one stops it." : ""))
                        .font(.geist(12))
                        .foregroundStyle(Theme.ink(0.6))
                }
                .padding(16)
            }
            .scrollDismissesKeyboard(.interactively)
            .safeAreaInset(edge: .bottom, spacing: 0) {
                Button {
                    let chosen = store.shownInstances.first { $0.id == target }
                    let title = chosen?.name ?? (name.isEmpty ? suggested : name)
                    dismiss()
                    Task { await store.build(instance: chosen?.id, name: title) }
                } label: {
                    HStack(spacing: 8) {
                        Text(target == nil ? "Build the new instance" : "Build the update")
                        HIcon(name: "arrow-right-01", size: 18)
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(AccentButton(height: 50))
                .padding(.horizontal, 16)
                .padding(.bottom, 8)
            }
        }
        .preferredColorScheme(.dark)
    }

    private func choice(_ id: String?, title: String, detail: String) -> some View {
        let on = target == id
        return Button {
            withAnimation(.easeOut(duration: 0.15)) { target = id }
        } label: {
            HStack(alignment: .top, spacing: 12) {
                ZStack {
                    Circle().strokeBorder(on ? Theme.accentSoft : Theme.ink(0.3), lineWidth: 1.5)
                    if on { Circle().fill(Theme.accentSoft).padding(5) }
                }
                .frame(width: 22, height: 22)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.geist(15, .semibold)).foregroundStyle(Theme.ink(1))
                    Text(detail).font(.geist(12)).foregroundStyle(Theme.ink(0.6))
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
            }
            .padding(12)
            .background(RoundedRectangle(cornerRadius: Theme.Radius.row, style: .continuous)
                .fill(Theme.fill(on ? 0.1 : 0.04)))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

/// Every Horizon on this iPhone: the plain one and each native-mod instance, to launch or update.
struct InstancesView: View {
    @EnvironmentObject var store: Store
    @Environment(\.openURL) private var openURL
    @State private var missing: String?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                PageHeader(title: "Instances", caption: "Each build with mods is its own app. Launch one from here.")
                InstanceCard(name: "Horizon", detail: "No native mods", icon: "mark", asset: nil,
                             play: { launch(URL(string: "horizon://"), "Horizon") }, remove: nil)
                ForEach(store.shownInstances) { instance in
                    InstanceCard(
                        name: instance.name,
                        detail: instance.mods.isEmpty ? "Build \(instance.build)" : store.names(instance.mods),
                        icon: nil,
                        asset: instance.asset,
                        footnote: "Build \(instance.build) · horizon-i\(instance.id)",
                        play: { launch(instance.link, instance.name) },
                        remove: { withAnimation { _ = store.hidden.insert(instance.id) } })
                }
                if store.shownInstances.isEmpty {
                    Plate {
                        Text("No instances yet. Pick mods, tap Build and choose A new instance.")
                            .font(.geist(14))
                            .foregroundStyle(Theme.ink(0.7))
                        Button("Pick mods") { store.tab = .mods }.buttonStyle(QuietButton())
                    }
                }
                Plate {
                    Text("An instance keeps its own worlds and settings, so copy the game files into its folder in the Files app the first time, as for Horizon. A free Apple ID can keep 3 sideloaded apps at once.")
                        .font(.geist(13))
                        .foregroundStyle(Theme.ink(0.7))
                }
                if !store.hidden.isEmpty {
                    Button("Show removed instances (\(store.hidden.count))") { store.hidden = [] }
                        .buttonStyle(QuietButton())
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 16)
        }
        .refreshable { await store.refreshInstances() }
        .alert("Couldn't open \(missing ?? "it")", isPresented: Binding(
            get: { missing != nil }, set: { if !$0 { missing = nil } })
        ) {
            Button("OK", role: .cancel) {}
        } message: {
            Text("It isn't installed on this iPhone, or it's from before builds had a launch link. Install its latest build, then try again.")
        }
    }

    private func launch(_ url: URL?, _ name: String) {
        guard let url else { return }
        openURL(url) { opened in
            if !opened { missing = name }
        }
    }
}

struct InstanceCard: View {
    let name: String
    let detail: String
    let icon: String?
    let asset: Asset?
    var footnote: String?
    let play: () -> Void
    let remove: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Group {
                    if let icon, let image = UIImage(named: icon) {
                        Image(uiImage: image).resizable().interpolation(.none).padding(8)
                    } else {
                        HIcon(name: "game-controller-01", size: 26).foregroundStyle(Theme.accentSoft)
                    }
                }
                .frame(width: 50, height: 50)
                .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Theme.fill(0.08)))
                VStack(alignment: .leading, spacing: 2) {
                    Text(name).font(.heading(21)).foregroundStyle(Theme.ink(1)).lineLimit(1)
                    Text(detail).font(.geist(13)).foregroundStyle(Theme.ink(0.65)).lineLimit(2)
                    if let footnote {
                        Text(footnote).font(.mono(11)).foregroundStyle(Theme.ink(0.45))
                    }
                }
                Spacer(minLength: 0)
                if let remove {
                    Menu {
                        Button(role: .destructive, action: remove) {
                            Label("Remove from this list", systemImage: "eye.slash")
                        }
                    } label: {
                        HIcon(name: "more-horizontal", size: 20).foregroundStyle(Theme.ink(0.7))
                            .frame(width: 36, height: 36)
                    }
                }
            }
            HStack(spacing: 8) {
                Button(action: play) {
                    HStack(spacing: 8) {
                        HIcon(name: "play", size: 18)
                        Text("Play")
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(AccentButton(height: 46))
                if let asset, let url = URL(string: asset.browserDownloadUrl) {
                    Menu {
                        ForEach(["sidestore", "altstore"], id: \.self) { scheme in
                            if let encoded = asset.browserDownloadUrl.addingPercentEncoding(withAllowedCharacters: .alphanumerics),
                               let link = URL(string: "\(scheme)://install?url=\(encoded)") {
                                Link(destination: link) {
                                    Label(scheme == "sidestore" ? "Open in SideStore" : "Open in AltStore",
                                          systemImage: "arrow.down.app")
                                }
                            }
                        }
                        Link(destination: url) { Label("Download in Safari", systemImage: "safari") }
                        ShareLink(item: url) { Label("Share", systemImage: "square.and.arrow.up") }
                    } label: {
                        HStack(spacing: 6) {
                            HIcon(name: "download-04", size: 18)
                            Text("Install")
                        }
                        .font(.geist(14, .medium))
                        .foregroundStyle(Theme.ink(0.9))
                        .padding(.horizontal, 16)
                        .frame(height: 46)
                        .background(RoundedRectangle(cornerRadius: Theme.Radius.large, style: .continuous)
                            .fill(Theme.fill(0.08)))
                    }
                }
            }
        }
        .padding(16)
        .glass()
    }
}
