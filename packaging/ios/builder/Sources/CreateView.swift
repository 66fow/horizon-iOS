import SwiftUI
import UIKit

/// The mod maker: the player's own mods, and a new one.
struct CreateView: View {
    @EnvironmentObject var store: Store
    @EnvironmentObject var maker: Maker
    @State private var editing: UUID?
    @State private var choosing = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                PageHeader(title: "Create", caption: "Describe a mod. Your AI writes it, GitHub checks it.")
                if !maker.anyConnected {
                    Plate("Connect an AI") {
                        Text("Sign in with ChatGPT to use your plan, or paste a Claude, OpenAI or Gemini API key.")
                            .font(.geist(14))
                            .foregroundStyle(Theme.ink(0.75))
                        Button("Connect") { store.tab = .settings }.buttonStyle(AccentButton(height: 40))
                    }
                }
                Button { choosing = true } label: {
                    HStack(spacing: 8) {
                        HIcon(name: "plus-sign", size: 18)
                        Text("New mod")
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(AccentButton(height: 50))
                .disabled(!maker.anyConnected)
                .opacity(maker.anyConnected ? 1 : 0.4)
                if maker.drafts.isEmpty {
                    Plate {
                        Text("Your mods show up here. Try “a block of glowing copper that drops itself” or “a HUD line with the time of day”.")
                            .font(.geist(14))
                            .foregroundStyle(Theme.ink(0.7))
                    }
                }
                LazyVStack(spacing: 8) {
                    ForEach(maker.drafts) { draft in
                        Button { editing = draft.key } label: { DraftRow(draft: draft) }
                            .buttonStyle(.plain)
                    }
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 16)
        }
        .confirmationDialog("What kind of mod?", isPresented: $choosing, titleVisibility: .visible) {
            ForEach(ModKind.allCases) { kind in
                Button(kind.title) {
                    let draft = Draft(kind: kind)
                    maker.update(draft)
                    editing = draft.key
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("A content mod is data (blocks, items, recipes) and installs straight away. A native mod is Rust code compiled into a build.")
        }
        .fullScreenCover(item: Binding(get: { editing.map(Key.init) }, set: { editing = $0?.id })) { key in
            ModEditor(key: key.id)
                .environmentObject(store)
                .environmentObject(maker)
        }
    }

    private struct Key: Identifiable { let id: UUID }
}

struct DraftRow: View {
    let draft: Draft

    private var state: (String, Color) {
        switch draft.check {
        case .none: return (draft.files.isEmpty ? "New" : "Not checked", Theme.ink(0.55))
        case .checking: return ("Checking", Theme.accentSoft)
        case .passed: return ("Ready", Theme.live)
        case .failed: return ("Needs a fix", Theme.danger)
        }
    }

    var body: some View {
        HStack(spacing: 12) {
            HIcon(name: draft.kind == .content ? "package-01" : "dashboard-square-01", size: 24)
                .foregroundStyle(Theme.accentSoft)
                .frame(width: 48, height: 48)
                .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Theme.fill(0.08)))
            VStack(alignment: .leading, spacing: 2) {
                Text(draft.name).font(.geist(16, .semibold)).foregroundStyle(Theme.ink(1)).lineLimit(1)
                Text(draft.summary.isEmpty ? draft.kind.title : draft.summary)
                    .font(.geist(13)).foregroundStyle(Theme.ink(0.65)).lineLimit(2)
                Text(draft.modId.isEmpty ? draft.kind.title : "\(draft.modId) · v\(draft.version)")
                    .font(.mono(11)).foregroundStyle(Theme.ink(0.45))
            }
            Spacer(minLength: 0)
            StateChip(label: state.0, color: state.1)
        }
        .padding(12)
        .glass(Theme.Radius.popover)
    }
}

/// One mod: the conversation with the AI, its files, and checking, using and sharing it.
struct ModEditor: View {
    @EnvironmentObject var store: Store
    @EnvironmentObject var maker: Maker
    @Environment(\.dismiss) private var dismiss
    @Environment(\.openURL) private var openURL
    let key: UUID
    @State private var text = ""
    @State private var showFiles = false
    @State private var confirmDelete = false
    @State private var confirmSubmit = false

    private var draft: Draft? { maker.draft(key) }
    private var thinking: Bool { maker.thinking == key }

    var body: some View {
        ZStack {
            SceneBackground()
            if let draft {
                VStack(spacing: 0) {
                    header(draft)
                    ScrollViewReader { reader in
                        ScrollView {
                            VStack(alignment: .leading, spacing: 10) {
                                if draft.chat.isEmpty { intro(draft) }
                                ForEach(draft.chat) { turn in Bubble(turn: turn).id(turn.id) }
                                if thinking {
                                    Bubble(turn: ChatTurn(role: .assistant, text: maker.streamed.isEmpty ? "Thinking…" : maker.streamed),
                                           live: true)
                                        .id("live")
                                }
                                if !draft.files.isEmpty { actions(draft) }
                            }
                            .padding(16)
                        }
                        .scrollDismissesKeyboard(.interactively)
                        .onChange(of: draft.chat.count) { _ in
                            if let last = draft.chat.last { withAnimation { reader.scrollTo(last.id, anchor: .bottom) } }
                        }
                        .onChange(of: maker.streamed) { _ in reader.scrollTo("live", anchor: .bottom) }
                    }
                    composer
                }
            }
        }
        .preferredColorScheme(.dark)
        .sheet(isPresented: $showFiles) {
            if let draft { FilesSheet(draft: draft) }
        }
        .alert("Mod maker", isPresented: Binding(get: { maker.notice != nil }, set: { if !$0 { maker.notice = nil } })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(maker.notice ?? "")
        }
        .confirmationDialog("Delete this mod from Horizon Mods?", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button("Delete", role: .destructive) {
                if let draft { maker.delete(draft) }
                dismiss()
            }
        } message: {
            Text("What's already on GitHub stays there.")
        }
        .confirmationDialog("Send it to Horizon's mod registry?", isPresented: $confirmSubmit, titleVisibility: .visible) {
            Button("Make its repository and open the submit page") {
                Task {
                    if let url = await maker.prepareSubmission(key, gitHub: store.gitHubClient) { openURL(url) }
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("The registry lists a mod from its own public GitHub repository. This makes github.com/<you>/horizon-mod-… with the mod and a release, then opens playhorizon.dev/mods/submit, where you sign in with GitHub and send it for review. Your token needs to be able to create repositories.")
        }
        .task(id: key) {
            while !Task.isCancelled {
                if maker.checking { await maker.poll(gitHub: store.gitHubClient) }
                try? await Task.sleep(nanoseconds: 15_000_000_000)
            }
        }
    }

    private func header(_ draft: Draft) -> some View {
        HStack(spacing: 10) {
            Button { dismiss() } label: {
                HIcon(name: "cancel-01", size: 18).foregroundStyle(Theme.ink(0.9)).padding(11).glass(Theme.Radius.row)
            }
            .buttonStyle(.plain)
            VStack(alignment: .leading, spacing: 1) {
                Text(draft.name).font(.heading(20)).foregroundStyle(Theme.ink(1)).lineLimit(1)
                Text("\(draft.kind.title) · \(maker.provider.short) \(maker.model(maker.provider))")
                    .font(.geist(11)).foregroundStyle(Theme.ink(0.55)).lineLimit(1)
            }
            Spacer()
            Menu {
                Picker("AI", selection: $maker.provider) {
                    ForEach(Provider.allCases.filter(maker.connected)) { p in Text(p.short).tag(p) }
                }
                if !draft.files.isEmpty {
                    Button { showFiles = true } label: { Label("Files (\(draft.files.count))", systemImage: "doc.text") }
                }
                Button(role: .destructive) { confirmDelete = true } label: { Label("Delete", systemImage: "trash") }
            } label: {
                HIcon(name: "more-horizontal", size: 20).foregroundStyle(Theme.ink(0.9)).padding(10).glass(Theme.Radius.row)
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 8)
        .padding(.bottom, 4)
    }

    private func intro(_ draft: Draft) -> some View {
        Plate(draft.kind.title) {
            Text(draft.kind.detail).font(.geist(14)).foregroundStyle(Theme.ink(0.75))
            Text(draft.kind == .content
                 ? "Say what to add: “a ruby ore that drops rubies, and ruby tools”, “a lamp block that glows purple”."
                 : "Say what it should do: “show my coordinates and the biome on screen”, “greet players by name when they join”.")
                .font(.geist(13)).foregroundStyle(Theme.ink(0.6))
        }
    }

    @ViewBuilder private func actions(_ draft: Draft) -> some View {
        Plate("\(draft.modId) · v\(draft.version)") {
            Button { showFiles = true } label: {
                HStack(spacing: 6) {
                    HIcon(name: "package-01", size: 16)
                    Text("\(draft.files.count) files")
                }
            }
            .buttonStyle(QuietButton())
            switch draft.check {
            case .none:
                Text("Next: GitHub checks it the way Horizon's registry does\(draft.kind == .native ? ", and compiles it into the game" : "").")
                    .font(.geist(13)).foregroundStyle(Theme.ink(0.65))
                uploadButton(draft, label: "Upload and check")
            case .checking:
                HStack(spacing: 8) {
                    ProgressView().tint(Theme.accentSoft)
                    Text(draft.kind == .native ? "Checking: a native mod is compiled into the game, about 15–25 minutes." : "Checking: a few minutes.")
                        .font(.geist(13)).foregroundStyle(Theme.ink(0.7))
                }
            case .passed(let version):
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Rectangle().fill(Theme.live).frame(width: 6, height: 6)
                    Text(draft.kind == .content
                         ? "Version \(version) passed. In Horizon, open Get mods → My mods to install it."
                         : "Version \(version) passed. It's in the Mods tab under Yours: pick it and build.")
                        .font(.geist(13)).foregroundStyle(Theme.ink(0.85))
                }
                HStack(spacing: 8) {
                    if draft.kind == .native {
                        Button("Pick for builds") {
                            store.picked.insert(draft.modId)
                            dismiss()
                            store.tab = .mods
                        }
                        .buttonStyle(QuietButton())
                    }
                    Button("Submit to registry") { confirmSubmit = true }
                        .buttonStyle(QuietButton())
                        .disabled(maker.busy != nil)
                }
            case .failed(let log):
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Rectangle().fill(Theme.danger).frame(width: 6, height: 6)
                    Text(log.split(separator: "\n").prefix(3).joined(separator: "\n"))
                        .font(.mono(11)).foregroundStyle(Theme.ink(0.75)).lineLimit(4)
                }
                Button {
                    Task { await maker.send(key, text: nil, repo: store.repo) }
                } label: {
                    HStack(spacing: 8) {
                        HIcon(name: "refresh", size: 16)
                        Text("Fix it with \(maker.provider.short)")
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(AccentButton(height: 44))
                .disabled(thinking)
            }
        }
    }

    private func uploadButton(_ draft: Draft, label: String) -> some View {
        Button {
            Task { await maker.upload(key, gitHub: store.gitHubClient) }
        } label: {
            HStack(spacing: 8) {
                if maker.busy == key { ProgressView().tint(Color.white) } else { HIcon(name: "upload-04", size: 18) }
                Text(label)
            }
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(AccentButton(height: 46))
        .disabled(!store.hasToken || maker.busy != nil || thinking)
        .opacity(store.hasToken ? 1 : 0.4)
    }

    private var composer: some View {
        HStack(alignment: .bottom, spacing: 8) {
            TextField("", text: $text, prompt: Text(draft?.chat.isEmpty ?? true ? "Describe your mod" : "Ask for a change").foregroundColor(Theme.ink(0.4)), axis: .vertical)
                .font(.geist(15))
                .foregroundStyle(Theme.ink(1))
                .lineLimit(1...6)
                .padding(.horizontal, 12)
                .padding(.vertical, 11)
                .background(RoundedRectangle(cornerRadius: Theme.Radius.large, style: .continuous).fill(Theme.fill(0.07)))
            Button {
                let message = text.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !message.isEmpty else { return }
                text = ""
                Task { await maker.send(key, text: message, repo: store.repo) }
            } label: {
                Group {
                    if thinking { ProgressView().tint(Color.white) } else { HIcon(name: "arrow-right-01", size: 20) }
                }
                .foregroundStyle(Color.white)
                .frame(width: 46, height: 46)
                .background(RoundedRectangle(cornerRadius: Theme.Radius.large, style: .continuous).fill(Theme.accent))
            }
            .buttonStyle(.plain)
            .disabled(thinking || text.trimmingCharacters(in: .whitespaces).isEmpty || !maker.connected(maker.provider))
        }
        .padding(10)
        .glass(Theme.Radius.panel)
        .padding(.horizontal, 12)
        .padding(.bottom, 8)
    }
}

/// A turn of the conversation; the AI's mod block is shown as a line, not as its JSON.
struct Bubble: View {
    let turn: ChatTurn
    var live = false

    private var shown: String {
        guard turn.role == .assistant else {
            return turn.text.hasPrefix("GitHub's check") ? "Sent GitHub's errors to fix." : turn.text
        }
        if let start = turn.text.range(of: Answer.fence) {
            let words = turn.text[..<start.lowerBound].trimmingCharacters(in: .whitespacesAndNewlines)
            return (words.isEmpty ? "" : words + "\n\n") + (live ? "Writing the files…" : "Wrote the mod's files.")
        }
        return turn.text
    }

    var body: some View {
        HStack {
            if turn.role == .user { Spacer(minLength: 40) }
            Text(shown)
                .font(.geist(15))
                .foregroundStyle(Theme.ink(turn.role == .user ? 1 : 0.88))
                .padding(12)
                .background(RoundedRectangle(cornerRadius: Theme.Radius.popover, style: .continuous)
                    .fill(turn.role == .user ? Theme.accent.opacity(0.85) : Color.clear))
                .background(turn.role == .assistant ? GlassPlate(radius: Theme.Radius.popover) : nil)
                .textSelection(.enabled)
            if turn.role == .assistant { Spacer(minLength: 40) }
        }
    }
}

struct FilesSheet: View {
    let draft: Draft
    @Environment(\.dismiss) private var dismiss
    @State private var open: String?

    var body: some View {
        NavigationStack {
            List(draft.files) { file in
                NavigationLink {
                    FileView(file: file)
                } label: {
                    Text(file.path).font(.mono(13))
                }
            }
            .navigationTitle("\(draft.files.count) files")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
        .preferredColorScheme(.dark)
    }
}

struct FileView: View {
    let file: ModFile

    var body: some View {
        ScrollView([.vertical, .horizontal]) {
            if file.isPicture, let data = try? file.bytes(), let image = UIImage(data: data) {
                Image(uiImage: image).resizable().interpolation(.none).frame(width: 256, height: 256).padding()
            } else {
                Text(file.text ?? "").font(.mono(12)).textSelection(.enabled).padding()
            }
        }
        .navigationTitle((file.path as NSString).lastPathComponent)
        .navigationBarTitleDisplayMode(.inline)
    }
}

/// The AI accounts, in Settings.
struct AIAccounts: View {
    @EnvironmentObject var maker: Maker
    @State private var signingIn = false
    @State private var listing: Provider?
    @State private var lists: [Provider: [String]] = [:]

    var body: some View {
        Plate("AI for the mod maker") {
            account(.chatgpt) {
                if let account = maker.chatgpt {
                    Text("Signed in\(account.email.isEmpty ? "" : " as \(account.email)"). Uses your plan's limits.")
                        .font(.geist(13)).foregroundStyle(Theme.ink(0.7))
                    Button("Sign out", role: .destructive) { maker.signOutChatGPT() }
                        .buttonStyle(QuietButton(tint: Theme.danger))
                } else {
                    Text("Uses your ChatGPT Go, Plus or Pro plan, through OpenAI's Sign in with ChatGPT.")
                        .font(.geist(13)).foregroundStyle(Theme.ink(0.7))
                    Button {
                        Task {
                            signingIn = true
                            await maker.signInChatGPT()
                            signingIn = false
                        }
                    } label: {
                        HStack(spacing: 8) {
                            if signingIn { ProgressView().tint(Color.white) }
                            Text("Sign in with ChatGPT")
                        }
                        .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(AccentButton(height: 44))
                }
            }
            ForEach([Provider.claude, .openai, .gemini]) { p in
                account(p) {
                    FieldBox {
                        SecureField("", text: Binding(get: { maker.keys[p] ?? "" }, set: { maker.setKey(p, $0) }),
                                    prompt: Text("API key").foregroundColor(Theme.ink(0.4)))
                            .font(.mono(14))
                            .foregroundStyle(Theme.ink(1))
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                    }
                    HStack(spacing: 8) {
                        Button("Paste") { maker.setKey(p, UIPasteboard.general.string ?? "") }
                            .buttonStyle(QuietButton())
                        if let page = p.keyPage {
                            Link("Get a key", destination: page).buttonStyle(QuietButton())
                        }
                    }
                }
            }
            Text(p0Note)
                .font(.geist(12))
                .foregroundStyle(Theme.ink(0.55))
        }
    }

    private var p0Note: String {
        "Claude and Gemini subscriptions can't be used: Anthropic's terms forbid using a Claude.ai login in other apps, and Google has no allowed way for apps to use a Gemini plan. Their API keys work (Gemini's has a free tier). Keys stay in this iPhone's keychain and only go to their own company."
    }

    @ViewBuilder
    private func account<Content: View>(_ p: Provider, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Button { maker.provider = p } label: {
                    HStack(spacing: 8) {
                        ZStack {
                            Circle().strokeBorder(maker.provider == p ? Theme.accentSoft : Theme.ink(0.3), lineWidth: 1.5)
                            if maker.provider == p { Circle().fill(Theme.accentSoft).padding(5) }
                        }
                        .frame(width: 20, height: 20)
                        Text(p.title).font(.geist(15, .semibold)).foregroundStyle(Theme.ink(1))
                    }
                }
                .buttonStyle(.plain)
                .disabled(!maker.connected(p))
                Spacer()
                if maker.connected(p) {
                    Menu {
                        if let list = lists[p], !list.isEmpty {
                            ForEach(list, id: \.self) { m in Button(m) { maker.models[p] = m } }
                        } else {
                            Button("Load the models") { Task { lists[p] = await maker.modelList(p) } }
                        }
                    } label: {
                        Text(maker.model(p).isEmpty ? "Model: newest" : maker.model(p))
                            .font(.mono(11))
                            .foregroundStyle(Theme.ink(0.7))
                            .lineLimit(1)
                    }
                }
            }
            content()
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: Theme.Radius.row, style: .continuous).fill(Theme.fill(0.04)))
    }
}
