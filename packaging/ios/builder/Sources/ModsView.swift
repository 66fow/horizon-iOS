import SwiftUI

struct ModsView: View {
    @EnvironmentObject var store: Store
    @State private var search = ""
    @State private var onlyPicked = false
    @State private var detail: NativeMod?
    @State private var confirming = false

    private var shown: [NativeMod] {
        store.mods.filter { mod in
            (!onlyPicked || store.picked.contains(mod.id))
                && (search.isEmpty
                    || mod.name.localizedCaseInsensitiveContains(search)
                    || mod.summary.localizedCaseInsensitiveContains(search)
                    || mod.id.localizedCaseInsensitiveContains(search))
        }
    }

    private var caption: String {
        store.mods.isEmpty
            ? "Mods compiled into Horizon for your iPhone."
            : "\(store.mods.count) mods for \(Store.game). Pick some, then build."
    }

    var body: some View {
        ScrollViewReader { reader in
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                PageHeader(title: "Native mods", caption: caption)
                    .id("top")
                if !store.hasToken { SetupCard() }
                SearchField(text: $search)
                Segmented(onlyPicked: $onlyPicked, picked: store.pickedInOrder.count)
                list
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 16)
        }
        .onChange(of: store.demoScroll) { target in
            guard let target else { return }
            withAnimation(.easeInOut(duration: 1.4)) { reader.scrollTo(target, anchor: .top) }
        }
        }
        .onChange(of: store.demoBuildSheet) { open in confirming = open }
        .onChange(of: store.demoDetail) { id in
            detail = id.flatMap { id in store.mods.first { $0.id == id } }
        }
        .scrollDismissesKeyboard(.interactively)
        .refreshable { await store.loadMods() }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if !store.pickedInOrder.isEmpty {
                BuildBar { confirming = true }
                    .padding(.horizontal, 16)
                    .padding(.bottom, 8)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.easeOut(duration: 0.2), value: store.pickedInOrder.isEmpty)
        .sheet(item: $detail, onDismiss: { store.demoDetail = nil }) { mod in
            ModDetail(mod: mod).environmentObject(store)
        }
        .sheet(isPresented: $confirming) {
            BuildSheet().environmentObject(store)
        }
        .alert("Couldn't start the build", isPresented: Binding(
            get: { store.notice != nil }, set: { if !$0 { store.notice = nil } })
        ) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(store.notice ?? "")
        }
    }

    @ViewBuilder private var list: some View {
        if store.mods.isEmpty, store.loadingMods {
            ProgressView()
                .tint(Theme.ink(0.8))
                .frame(maxWidth: .infinity)
                .padding(.vertical, 48)
        } else if store.mods.isEmpty, let error = store.modsError {
            ErrorPlate(message: error) { Task { await store.loadMods() } }
        } else if shown.isEmpty {
            Text(onlyPicked ? "Nothing picked yet. Tap the box beside a mod to pick it." : "No mods match “\(search)”.")
                .font(.geist(14))
                .foregroundStyle(Theme.ink(0.65))
                .frame(maxWidth: .infinity)
                .padding(.vertical, 32)
        } else {
            LazyVStack(spacing: 8) {
                ForEach(shown) { mod in
                    ModRow(mod: mod, picked: store.picked.contains(mod.id),
                           toggle: { withAnimation(.easeOut(duration: 0.15)) { store.toggle(mod) } },
                           open: { detail = mod })
                        .id(mod.id)
                }
            }
        }
    }

}

struct SearchField: View {
    @Binding var text: String

    var body: some View {
        HStack(spacing: 8) {
            HIcon(name: "search-01", size: 18).foregroundStyle(Theme.ink(0.5))
            TextField("", text: $text, prompt: Text("Search mods").foregroundColor(Theme.ink(0.4)))
                .font(.geist(15))
                .foregroundStyle(Theme.ink(1))
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .submitLabel(.search)
            if !text.isEmpty {
                Button { text = "" } label: {
                    HIcon(name: "cancel-01", size: 16).foregroundStyle(Theme.ink(0.55))
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 14)
        .frame(height: 46)
        .glass(Theme.Radius.large)
    }
}

/// All or only the picked mods: a pill, the open side on the selected fill.
struct Segmented: View {
    @Binding var onlyPicked: Bool
    let picked: Int

    var body: some View {
        HStack(spacing: 4) {
            segment("All", value: false)
            segment(picked > 0 ? "Picked \(picked)" : "Picked", value: true)
        }
        .padding(4)
        .glass(Theme.Radius.large)
    }

    private func segment(_ label: String, value: Bool) -> some View {
        let on = onlyPicked == value
        return Button {
            withAnimation(.easeOut(duration: 0.15)) { onlyPicked = value }
        } label: {
            Text(label)
                .font(.geist(13, .medium))
                .foregroundStyle(Theme.ink(on ? 1 : 0.6))
                .frame(maxWidth: .infinity)
                .frame(height: 34)
                .background(RoundedRectangle(cornerRadius: Theme.Radius.control, style: .continuous)
                    .fill(Theme.fill(on ? 0.12 : 0)))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

/// A mod: its icon, name, summary and author; the box picks it, the rest opens its page.
struct ModRow: View {
    let mod: NativeMod
    let picked: Bool
    let toggle: () -> Void
    let open: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Button(action: open) {
                HStack(spacing: 12) {
                    ModIcon(url: mod.icon, size: 54)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(mod.name)
                            .font(.geist(16, .semibold))
                            .foregroundStyle(Theme.ink(1))
                            .lineLimit(1)
                        if !mod.summary.isEmpty {
                            Text(mod.summary)
                                .font(.geist(13))
                                .foregroundStyle(Theme.ink(0.65))
                                .lineLimit(2)
                        }
                        Text(mod.byline)
                            .font(.geist(11))
                            .foregroundStyle(Theme.ink(0.45))
                            .lineLimit(1)
                    }
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            Button(action: toggle) {
                Tick(on: picked).frame(width: 44, height: 44).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(picked ? "Unpick \(mod.name)" : "Pick \(mod.name)")
        }
        .padding(.leading, 12)
        .padding(.vertical, 12)
        .padding(.trailing, 6)
        .glass(Theme.Radius.popover, fill: picked ? 0.05 : 0)
        .overlay(RoundedRectangle(cornerRadius: Theme.Radius.popover, style: .continuous)
            .strokeBorder(Theme.accentSoft.opacity(picked ? 0.5 : 0), lineWidth: 1))
    }
}

struct Tick: View {
    let on: Bool

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .fill(on ? Theme.accent : Theme.fill(0.08))
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .strokeBorder(Theme.ink(on ? 0 : 0.25), lineWidth: 1.5)
            if on {
                HIcon(name: "tick-02", size: 18).foregroundStyle(Color.white)
            }
        }
        .frame(width: 30, height: 30)
    }
}

struct BuildBar: View {
    @EnvironmentObject var store: Store
    let start: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text("\(store.pickedInOrder.count) picked")
                    .font(.geist(15, .semibold))
                    .foregroundStyle(Theme.ink(1))
                Text(store.hasToken ? "About 30–45 minutes on GitHub" : "Add a GitHub token in Settings first")
                    .font(.geist(12))
                    .foregroundStyle(Theme.ink(0.6))
            }
            Spacer()
            Button(action: start) {
                HStack(spacing: 8) {
                    if store.starting {
                        ProgressView().tint(Color.white)
                    } else {
                        Text("Build")
                        HIcon(name: "arrow-right-01", size: 18)
                    }
                }
                .frame(minWidth: 84)
            }
            .buttonStyle(AccentButton(height: 46))
            .disabled(!store.hasToken || store.starting)
            .opacity(!store.hasToken || store.starting ? 0.4 : 1)
        }
        .padding(12)
        .glass(Theme.Radius.panel)
    }
}

struct SetupCard: View {
    @EnvironmentObject var store: Store

    var body: some View {
        Plate("Connect GitHub") {
            Text("GitHub builds Horizon with your mods. Add a token once so this app can start builds on \(store.repo.isEmpty ? "your fork" : store.repo).")
                .font(.geist(14))
                .foregroundStyle(Theme.ink(0.75))
            Button("Set up") { store.tab = .settings }
                .buttonStyle(AccentButton(height: 40))
        }
    }
}

/// A mod's page: its icon and name, screenshots, description and details.
struct ModDetail: View {
    @EnvironmentObject var store: Store
    @Environment(\.dismiss) private var dismiss
    let mod: NativeMod

    private var text: AttributedString {
        let raw = mod.description.isEmpty ? mod.summary : mod.description
        let options = AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        return (try? AttributedString(markdown: raw, options: options)) ?? AttributedString(raw)
    }

    private var picked: Bool { store.picked.contains(mod.id) }

    var body: some View {
        ZStack {
            SceneBackground()
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    HStack {
                        Spacer()
                        Button { dismiss() } label: {
                            HIcon(name: "cancel-01", size: 18)
                                .foregroundStyle(Theme.ink(0.9))
                                .padding(11)
                                .glass(Theme.Radius.row)
                        }
                        .buttonStyle(.plain)
                    }
                    HStack(spacing: 14) {
                        ModIcon(url: mod.icon, size: 76)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(mod.name)
                                .font(.heading(28))
                                .foregroundStyle(Theme.ink(1))
                                .lineLimit(2)
                                .minimumScaleFactor(0.7)
                            Text(mod.byline)
                                .font(.geist(13))
                                .foregroundStyle(Theme.ink(0.6))
                        }
                    }
                    if !mod.screenshots.isEmpty {
                        Gallery(urls: mod.screenshots)
                    }
                    if !mod.summary.isEmpty || !mod.description.isEmpty {
                        Plate {
                            Text(text)
                                .font(.geist(15))
                                .foregroundStyle(Theme.ink(0.82))
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                    Plate("Details") {
                        VStack(spacing: 0) {
                            Detail("Version", mod.latest?.version ?? "?")
                            if !mod.authors.isEmpty { Detail("By", mod.authors.joined(separator: ", ")) }
                            if !mod.license.isEmpty { Detail("License", mod.license) }
                            Detail("Downloads", "\(mod.downloads)")
                            if let needs = mod.latest?.requires, !needs.isEmpty {
                                Detail("Needs", store.names(needs))
                            }
                            Detail("Id", mod.id, mono: true)
                        }
                        if let url = URL(string: mod.source), mod.source.hasPrefix("https://") {
                            Link(destination: url) {
                                HStack(spacing: 6) {
                                    HIcon(name: "github", size: 16)
                                    Text("Source code")
                                }
                            }
                            .buttonStyle(QuietButton())
                        }
                    }
                }
                .padding(16)
            }
            .safeAreaInset(edge: .bottom, spacing: 0) {
                Button {
                    withAnimation(.easeOut(duration: 0.15)) { store.toggle(mod) }
                } label: {
                    HStack(spacing: 8) {
                        HIcon(name: picked ? "cancel-01" : "tick-02", size: 18)
                        Text(picked ? "Remove from the build" : "Add to the build")
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
}

private struct Detail: View {
    let label: String
    let value: String
    let mono: Bool

    init(_ label: String, _ value: String, mono: Bool = false) {
        self.label = label
        self.value = value
        self.mono = mono
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).font(.geist(13)).foregroundStyle(Theme.ink(0.55))
            Spacer(minLength: 16)
            Text(value)
                .font(mono ? .mono(13) : .geist(14, .medium))
                .foregroundStyle(Theme.ink(0.95))
                .multilineTextAlignment(.trailing)
        }
        .padding(.vertical, 9)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.fill(0.08)).frame(height: 1) }
    }
}
