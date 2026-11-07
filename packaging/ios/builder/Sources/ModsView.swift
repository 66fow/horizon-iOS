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

    var body: some View {
        NavigationStack {
            List {
                if !store.hasToken {
                    Section { SetupCard() }
                }
                Section {
                    Picker("Show", selection: $onlyPicked) {
                        Text("All").tag(false)
                        Text("Picked (\(store.pickedInOrder.count))").tag(true)
                    }
                    .pickerStyle(.segmented)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                }
                Section {
                    if store.mods.isEmpty, store.loadingMods {
                        HStack {
                            Spacer()
                            ProgressView("Loading mods…")
                            Spacer()
                        }
                        .padding(.vertical)
                    } else if store.mods.isEmpty, let error = store.modsError {
                        ErrorRow(message: error) { Task { await store.loadMods() } }
                    } else if shown.isEmpty {
                        Text(onlyPicked ? "Nothing picked yet. Tap a mod to pick it." : "No mods match “\(search)”.")
                            .foregroundStyle(.secondary)
                    }
                    ForEach(shown) { mod in
                        ModRow(mod: mod, picked: store.picked.contains(mod.id),
                               toggle: { store.toggle(mod) }, info: { detail = mod })
                    }
                } footer: {
                    if !store.mods.isEmpty {
                        Text("\(store.mods.count) native mods for Minecraft \(Store.game). Each build compiles the mods you pick into Horizon.")
                    }
                }
            }
            .navigationTitle("Native mods")
            .searchable(text: $search, prompt: "Search mods")
            .refreshable { await store.loadMods() }
            .toolbar {
                if !store.picked.isEmpty {
                    Button("Clear") { store.picked = [] }
                }
            }
            .safeAreaInset(edge: .bottom) {
                if !store.pickedInOrder.isEmpty {
                    BuildBar { confirming = true }
                }
            }
            .sheet(item: $detail) { mod in
                ModDetail(mod: mod).environmentObject(store)
            }
            .confirmationDialog("Build Horizon with \(store.pickedInOrder.count) mods?",
                                isPresented: $confirming, titleVisibility: .visible) {
                Button("Build") { Task { await store.build() } }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text(buildMessage)
            }
            .alert("Couldn't start the build", isPresented: Binding(
                get: { store.notice != nil }, set: { if !$0 { store.notice = nil } })
            ) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(store.notice ?? "")
            }
        }
    }

    private var buildMessage: String {
        var lines = [store.names(store.pickedInOrder) + "."]
        if store.activeCount > 0 {
            lines.append("A build is already running: starting this one stops it.")
        }
        lines.append("Native mods change Horizon's own code and run with full access to the app. Only build mods you trust.")
        return lines.joined(separator: "\n\n")
    }
}

struct ModRow: View {
    let mod: NativeMod
    let picked: Bool
    let toggle: () -> Void
    let info: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Button(action: toggle) {
                HStack(alignment: .top, spacing: 12) {
                    Image(systemName: picked ? "checkmark.circle.fill" : "circle")
                        .font(.title2)
                        .foregroundStyle(picked ? Color.horizon : Color.secondary)
                    VStack(alignment: .leading, spacing: 3) {
                        Text(mod.name)
                            .font(.headline)
                            .foregroundStyle(.primary)
                        if !mod.summary.isEmpty {
                            Text(mod.summary)
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                        }
                        Text(mod.byline)
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                    }
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            Button(action: info) {
                Image(systemName: "info.circle").font(.title3)
            }
            .buttonStyle(.borderless)
        }
        .padding(.vertical, 4)
    }
}

struct BuildBar: View {
    @EnvironmentObject var store: Store
    let start: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text("\(store.pickedInOrder.count) picked").font(.headline)
                Text(store.hasToken ? "About 30–45 minutes on GitHub" : "Add a GitHub token in Settings first")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Button(action: start) {
                if store.starting {
                    ProgressView().frame(minWidth: 70)
                } else {
                    Label("Build", systemImage: "hammer.fill").bold().frame(minWidth: 70)
                }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .disabled(!store.hasToken || store.starting)
        }
        .padding(.horizontal)
        .padding(.vertical, 10)
        .background(.bar)
    }
}

struct SetupCard: View {
    @EnvironmentObject var store: Store

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Connect GitHub", systemImage: "key.fill").font(.headline)
            Text("GitHub builds Horizon with your mods. Add a token once so this app can start builds on \(store.repo.isEmpty ? "your fork" : store.repo).")
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Button("Set up") { store.tab = .settings }
                .buttonStyle(.bordered)
        }
        .padding(.vertical, 4)
    }
}

struct ModDetail: View {
    @EnvironmentObject var store: Store
    @Environment(\.dismiss) private var dismiss
    let mod: NativeMod

    private var text: AttributedString {
        let raw = mod.description.isEmpty ? mod.summary : mod.description
        let options = AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        return (try? AttributedString(markdown: raw, options: options)) ?? AttributedString(raw)
    }

    var body: some View {
        NavigationStack {
            List {
                Section { Text(text) }
                Section("Details") {
                    LabeledContent("Version", value: mod.latest?.version ?? "?")
                    if !mod.authors.isEmpty {
                        LabeledContent("By", value: mod.authors.joined(separator: ", "))
                    }
                    if !mod.license.isEmpty {
                        LabeledContent("License", value: mod.license)
                    }
                    LabeledContent("Downloads", value: "\(mod.downloads)")
                    if let needs = mod.latest?.requires, !needs.isEmpty {
                        LabeledContent("Needs", value: store.names(needs))
                    }
                    if let url = URL(string: mod.source), !mod.source.isEmpty {
                        Link(destination: url) { Label("Source code", systemImage: "chevron.left.forwardslash.chevron.right") }
                    }
                }
            }
            .navigationTitle(mod.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(store.picked.contains(mod.id) ? "Unpick" : "Pick") { store.toggle(mod) }
                }
            }
        }
    }
}
