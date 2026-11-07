import Foundation
import SwiftUI
import UIKit

// MARK: A mod the AI writes

enum ModKind: String, Codable, CaseIterable, Identifiable {
    case content, native
    var id: String { rawValue }
    var title: String { self == .content ? "Content mod" : "Native mod" }
    var detail: String {
        self == .content
            ? "Blocks, items, recipes and loot as data. Installs straight away in Horizon's My mods."
            : "Rust code in the game itself (a new HUD, key, rule). Compiled into a build on GitHub."
    }
}

/// One file of a mod: text, or a small PNG drawn as a palette and rows of characters.
struct ModFile: Codable, Identifiable, Equatable {
    var path: String
    var text: String?
    var palette: [String: String]?
    var rows: [String]?

    var id: String { path }
    var isPicture: Bool { palette != nil && rows != nil }

    func bytes() throws -> Data {
        if let text { return Data(text.utf8) }
        guard let palette, let rows, let width = rows.first?.count, width > 0, width <= 256, rows.count <= 256 else {
            throw AIError(message: "\(path) is neither text nor a picture")
        }
        return try PixelArt.png(palette: palette, rows: rows, width: width, path: path)
    }
}

enum PixelArt {
    static func color(_ text: String) -> UIColor? {
        let t = text.trimmingCharacters(in: .whitespaces).lowercased()
        if t == "transparent" || t == "none" || t.isEmpty { return .clear }
        var hex = t.hasPrefix("#") ? String(t.dropFirst()) : t
        if hex.count == 6 { hex += "ff" }
        guard hex.count == 8, let v = UInt32(hex, radix: 16) else { return nil }
        return UIColor(red: CGFloat((v >> 24) & 255) / 255, green: CGFloat((v >> 16) & 255) / 255,
                       blue: CGFloat((v >> 8) & 255) / 255, alpha: CGFloat(v & 255) / 255)
    }

    static func png(palette: [String: String], rows: [String], width: Int, path: String) throws -> Data {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = false
        let image = try UIGraphicsImageRenderer(size: CGSize(width: width, height: rows.count), format: format).image { ctx in
            for (y, row) in rows.enumerated() {
                for (x, ch) in row.prefix(width).enumerated() {
                    guard let entry = palette[String(ch)] else { continue }
                    guard let colour = color(entry) else { continue }
                    colour.setFill()
                    ctx.fill(CGRect(x: x, y: y, width: 1, height: 1))
                }
            }
        }
        guard let data = image.pngData() else { throw AIError(message: "\(path): the picture didn't draw") }
        return data
    }
}

enum CheckState: Codable, Equatable {
    case none
    case checking(run: Int?, since: Date)
    case passed(version: String)
    case failed(log: String)
}

struct Draft: Codable, Identifiable, Equatable {
    var key = UUID()
    var kind: ModKind
    var modId = ""
    var name = "New mod"
    var version = "0.1.0"
    var summary = ""
    var files: [ModFile] = []
    var chat: [ChatTurn] = []
    var check: CheckState = .none
    var updated = Date()

    var id: UUID { key }
}

/// The AI's answer: its words for the player and the mod.
struct Generated: Decodable {
    let id: String
    let name: String
    let version: String
    let summary: String?
    let files: [ModFile]
}

enum Answer {
    static let fence = "```horizon-mod"

    /// The words before (and after) the mod block, and the mod.
    static func parse(_ text: String) -> (words: String, mod: Generated?, problem: String?) {
        guard let start = text.range(of: fence) else {
            return (text.trimmingCharacters(in: .whitespacesAndNewlines), nil, nil)
        }
        let rest = text[start.upperBound...]
        guard let end = rest.range(of: "\n```") else {
            return (String(text[..<start.lowerBound]), nil, "The answer was cut off before the mod ended.")
        }
        let json = rest[..<end.lowerBound]
        let words = (String(text[..<start.lowerBound]) + String(rest[end.upperBound...]))
            .trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let mod = try JSONDecoder().decode(Generated.self, from: Data(json.utf8))
            return (words, mod, nil)
        } catch {
            return (words, nil, "The mod's JSON didn't read: \(error.localizedDescription)")
        }
    }

    static let idPattern = #"^[a-z0-9_]{1,64}$"#
    static let pathPattern = #"^[A-Za-z0-9_][A-Za-z0-9_./-]*$"#

    /// What's wrong with `mod` as a `kind` mod, or nil.
    static func problem(_ mod: Generated, kind: ModKind) -> String? {
        guard mod.id.range(of: idPattern, options: .regularExpression) != nil else {
            return "The mod id “\(mod.id)” isn't [a-z0-9_]."
        }
        guard mod.version.range(of: #"^[0-9A-Za-z.+-]{1,32}$"#, options: .regularExpression) != nil else {
            return "The version “\(mod.version)” isn't a plain version."
        }
        guard !mod.files.isEmpty, mod.files.count <= 400 else { return "The mod has no files, or too many." }
        var seen = Set<String>()
        for file in mod.files {
            let p = file.path
            if p.range(of: pathPattern, options: .regularExpression) == nil || p.contains("..") || p.contains("//") {
                return "The path “\(p)” isn't a plain relative path."
            }
            if !seen.insert(p).inserted { return "“\(p)” is there twice." }
            if file.text == nil && !file.isPicture { return "“\(p)” has no text or picture." }
            if kind == .native && !p.hasPrefix("mixins/\(mod.id)/") {
                return "A native mod's files go under mixins/\(mod.id)/, not “\(p)”."
            }
        }
        switch kind {
        case .content where !seen.contains("horizon-mod.toml"):
            return "A content mod needs horizon-mod.toml."
        case .native where !seen.contains("mixins/\(mod.id)/horizon-mixin.toml"):
            return "A native mod needs mixins/\(mod.id)/horizon-mixin.toml."
        default:
            return nil
        }
    }
}

// MARK: What the AI is told

enum Prompt {
    private static var cache: [String: String] = [:]

    /// A file of the fork's source (public), from its ios-build branch.
    private static func source(_ repo: String, _ path: String) async -> String {
        if let hit = cache[path] { return hit }
        guard let url = URL(string: "https://raw.githubusercontent.com/\(repo)/ios-build/\(path)"),
              let result = try? await URLSession.shared.data(from: url),
              (result.1 as? HTTPURLResponse)?.statusCode == 200,
              let text = String(data: result.0, encoding: .utf8) else { return "" }
        cache[path] = text
        return text
    }

    /// `text` from the line `start` to the next heading at its level or above.
    private static func section(_ text: String, _ start: String) -> String {
        guard let from = text.range(of: "\n" + start) else { return "" }
        let level = start.prefix { $0 == "#" }.count
        var out = [String]()
        for line in text[from.upperBound...].split(separator: "\n", omittingEmptySubsequences: false).dropFirst() {
            let hashes = line.prefix { $0 == "#" }.count
            if hashes > 0 && hashes <= level && line.dropFirst(hashes).hasPrefix(" ") { break }
            out.append(String(line))
        }
        return start + "\n" + out.joined(separator: "\n")
    }

    static let contentTemplate: [(String, String)] = [
        ("horizon-mod.toml", "horizon-mod.toml"),
        ("data/{id}/horizon/block/glowing_amethyst.json", "block.json"),
        ("assets/{id}/blockstates/glowing_amethyst.json", "blockstate.json"),
        ("assets/{id}/models/block/glowing_amethyst.json", "model.json"),
        ("assets/{id}/items/glowing_amethyst.json", "item.json"),
        ("assets/{id}/lang/en_us.json", "lang.json"),
        ("data/{id}/loot_table/blocks/glowing_amethyst.json", "loot.json"),
        ("data/{id}/recipe/glowing_amethyst.json", "recipe.json"),
        ("data/minecraft/tags/block/mineable/pickaxe.json", "pickaxe.json"),
    ]

    static func instructions(kind: ModKind, repo: String) async -> String {
        var reference = [String]()
        switch kind {
        case .content:
            let mods = await source(repo, "docs/mods.md")
            reference.append(section(mods, "### The manifest"))
            reference.append(section(mods, "### Content mods (API 0.2)"))
            let packs = await source(repo, "docs/modpacks.md")
            reference.append(section(packs, "### Registries 0.2: data-driven blocks, items and mob effects"))
            var example = ["An example content mod (Horizon's own template; {id} is the mod id):"]
            for (path, file) in contentTemplate {
                let text = await source(repo, "crates/cargo-horizon-mod/template/content/\(file)")
                example.append("--- \(path)\n\(text)")
            }
            reference.append(example.joined(separator: "\n"))
        case .native:
            let native = await source(repo, "docs/native-mods.md")
            reference.append(section(native, "## Trait mods (draft)"))
            var example = ["An example native trait mod (Horizon's own template; {id} is the mod id):"]
            for (path, file) in [("mixins/{id}/horizon-mixin.toml", "horizon-mixin.toml"),
                                 ("mixins/{id}/api/Cargo.toml", "api.toml"),
                                 ("mixins/{id}/api/src/lib.rs", "lib.rs")] {
                let text = await source(repo, "crates/cargo-horizon-mod/template/native/\(file)")
                example.append("--- \(path)\n\(text)")
            }
            reference.append(example.joined(separator: "\n"))
            let api = await source(repo, "crates/horizon-client/src/mod_api.rs")
            reference.append("crates/horizon-client/src/mod_api.rs, everything a trait mod can use:\n```rust\n\(api)\n```")
        }
        let rules: String
        switch kind {
        case .content:
            rules = """
            - Paths: horizon-mod.toml, data/<id>/…, assets/<id>/… (and data/minecraft/tags/… for vanilla tags).
            - The manifest has api = "0.2", side = "both" and capabilities = ["registries"], and only the keys shown.
            - Blocks and items copy a vanilla template and change what the reference allows; textures can reuse \
            vanilla ones (minecraft:block/…) or be your own PNGs.
            """
        case .native:
            rules = """
            - Every path is under mixins/<id>/: horizon-mixin.toml ([mod] id and version, [api] path = "api"), \
            api/Cargo.toml (package mod-<id>, edition 2024, the one dependency \
            horizon-client = { git = "https://github.com/horizon-rust/horizon" }), api/src/lib.rs with \
            `pub fn hooks() -> Box<dyn Hooks>`.
            - Implement only hooks and use only types that mod_api.rs below declares. No other crates. \
            Don't write horizon-native.json: the build adds it.
            """
        }
        return """
        You write mods for Horizon, a Rust rewrite of Minecraft: Java Edition 26.3 that the player runs on an \
        iPhone. The player describes a mod and you write all of its files. This is a \(kind == .content ? "content mod: data and assets only, no code" : "native mod: a Rust trait mod compiled into the game").

        How to answer:
        - First, a few short sentences for the player: what the mod does and how to use it in the game. No code there.
        - Then exactly one fenced block that starts with ```horizon-mod and holds one JSON object:
          {"id": "<[a-z0-9_]{1,64}>", "name": "<name>", "version": "<x.y.z>", "summary": "<one line>", "files": [ … ]}
        - "files" is the whole mod, every file, every time (never only what changed). A file is \
        {"path": "<relative path>", "text": "<its text>"}. A PNG texture is \
        {"path": "assets/<id>/textures/….png", "palette": {"a": "#RRGGBB", ".": "transparent"}, "rows": ["<16 characters>", … 16 rows]}.
        - Keep the same id for the same mod. Raise the patch version each time you change it.
        - When the player sends errors from GitHub's check, fix them and send the whole mod again.
        - Never invent fields, hooks or types that the reference doesn't show. If the idea can't be done with \
        what's there, say so and offer the closest thing that can.
        \(rules)

        Reference (Horizon's own documentation and source):

        \(reference.joined(separator: "\n\n"))
        """
    }
}

// MARK: The mod maker

@MainActor
final class Maker: ObservableObject {
    @Published var drafts: [Draft] = []
    @Published var provider: Provider {
        didSet { UserDefaults.standard.set(provider.rawValue, forKey: "aiProvider") }
    }
    @Published var models: [Provider: String] {
        didSet { UserDefaults.standard.set(models.reduce(into: [String: String]()) { $0[$1.key.rawValue] = $1.value }, forKey: "aiModels") }
    }
    @Published var keys: [Provider: String] = [:]
    @Published var chatgpt: ChatGPTAccount?
    @Published var thinking: UUID?
    @Published var streamed = ""
    @Published var busy: UUID?
    @Published var notice: String?

    private let signer = ChatGPTSignIn()

    private static var folder: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("drafts")
    }

    init() {
        let defaults = UserDefaults.standard
        provider = Provider(rawValue: defaults.string(forKey: "aiProvider") ?? "") ?? .chatgpt
        let saved = defaults.dictionary(forKey: "aiModels") as? [String: String] ?? [:]
        models = saved.reduce(into: [Provider: String]()) { out, pair in
            if let p = Provider(rawValue: pair.key) { out[p] = pair.value }
        }
        for p in [Provider.claude, .openai, .gemini] { keys[p] = Secrets.load(p.rawValue) }
        if let data = Secrets.load("chatgpt").data(using: .utf8) {
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            chatgpt = try? decoder.decode(ChatGPTAccount.self, from: data)
        }
        load()
    }

    // MARK: Accounts

    func connected(_ p: Provider) -> Bool {
        p == .chatgpt ? chatgpt != nil : !(keys[p] ?? "").isEmpty
    }

    var anyConnected: Bool { Provider.allCases.contains(where: connected) }

    func setKey(_ p: Provider, _ key: String) {
        let key = key.trimmingCharacters(in: .whitespacesAndNewlines)
        keys[p] = key
        Secrets.save(p.rawValue, key)
        if !key.isEmpty, !connected(provider) { provider = p }
    }

    private func saveChatGPT() {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        Secrets.save("chatgpt", chatgpt.flatMap { try? encoder.encode($0) }.flatMap { String(data: $0, encoding: .utf8) } ?? "")
    }

    func signInChatGPT() async {
        do {
            chatgpt = try await signer.signIn(previous: chatgpt)
            saveChatGPT()
            provider = .chatgpt
        } catch {
            let text = error.localizedDescription
            if !text.contains("closed") && !text.contains("canceled") && !text.contains("cancelled") { notice = text }
        }
    }

    func signOutChatGPT() {
        chatgpt = nil
        saveChatGPT()
    }

    func model(_ p: Provider) -> String { models[p].flatMap { $0.isEmpty ? nil : $0 } ?? p.defaultModel }

    private func client(_ p: Provider) async throws -> AIClient {
        var account = chatgpt
        if p == .chatgpt {
            guard let current = chatgpt else { throw AIError(message: "Sign in with ChatGPT in Settings first.") }
            account = try await ChatGPTSignIn.fresh(current)
            if account?.accessToken != current.accessToken {
                chatgpt = account
                saveChatGPT()
            }
        }
        var client = AIClient(provider: p, model: model(p), key: keys[p] ?? "", chatgpt: account)
        if client.model.isEmpty {
            // OpenAI's and Google's model names change: the account's newest big one.
            let list = try await client.models()
            let pick = Self.pick(list, provider: p)
            guard let pick else { throw AIError(message: "\(p.short) lists no model to write with: pick one in Settings.") }
            models[p] = pick
            client = AIClient(provider: p, model: pick, key: client.key, chatgpt: account)
        }
        return client
    }

    static func pick(_ list: [String], provider: Provider) -> String? {
        let usable = list.filter { !$0.contains("audio") && !$0.contains("image") && !$0.contains("tts") && !$0.contains("embed") && !$0.contains("realtime") && !$0.contains("search") }
        switch provider {
        case .gemini:
            return usable.filter { $0.contains("pro") && !$0.contains("preview-tts") }.sorted(by: >).first ?? usable.sorted(by: >).first
        default:
            return usable.filter { !$0.contains("mini") && !$0.contains("nano") }.sorted(by: >).first ?? usable.first
        }
    }

    func modelList(_ p: Provider) async -> [String] {
        (try? await client(p).models()) ?? []
    }

    // MARK: Drafts

    private func load() {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let files = (try? FileManager.default.contentsOfDirectory(at: Self.folder, includingPropertiesForKeys: nil)) ?? []
        drafts = files.compactMap { try? decoder.decode(Draft.self, from: Data(contentsOf: $0)) }
            .sorted { $0.updated > $1.updated }
    }

    private func save(_ draft: Draft) {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        try? FileManager.default.createDirectory(at: Self.folder, withIntermediateDirectories: true)
        try? encoder.encode(draft).write(to: Self.folder.appendingPathComponent("\(draft.key.uuidString).json"), options: .atomic)
    }

    func update(_ draft: Draft) {
        var draft = draft
        draft.updated = Date()
        if let i = drafts.firstIndex(where: { $0.key == draft.key }) { drafts[i] = draft } else { drafts.insert(draft, at: 0) }
        drafts.sort { $0.updated > $1.updated }
        save(draft)
    }

    func delete(_ draft: Draft) {
        drafts.removeAll { $0.key == draft.key }
        try? FileManager.default.removeItem(at: Self.folder.appendingPathComponent("\(draft.key.uuidString).json"))
    }

    func draft(_ key: UUID) -> Draft? { drafts.first { $0.key == key } }

    // MARK: Writing with the AI

    /// Sends the player's `text` (or, with nil, the last check's errors) and takes the answer.
    func send(_ key: UUID, text: String?, repo: String) async {
        guard var draft = draft(key), thinking == nil else { return }
        let message: String
        if let text {
            message = text
        } else if case .failed(let log) = draft.check {
            message = "GitHub's check of version \(draft.version) failed with:\n```\n\(log)\n```\nFix the mod and send all of its files again, with the patch version raised."
        } else {
            return
        }
        draft.chat.append(ChatTurn(role: .user, text: message))
        update(draft)
        thinking = key
        streamed = ""
        defer { thinking = nil }
        do {
            let ai = try await client(provider)
            let instructions = await Prompt.instructions(kind: draft.kind, repo: repo)
            let answer = try await ai.send(instructions: instructions, turns: draft.chat) { [weak self] partial in
                Task { @MainActor in self?.streamed = partial }
            }
            guard var latest = self.draft(key) else { return }
            latest.chat.append(ChatTurn(role: .assistant, text: answer))
            let parsed = Answer.parse(answer)
            if let mod = parsed.mod {
                if let problem = Answer.problem(mod, kind: latest.kind) {
                    notice = "The mod the AI wrote has a problem: \(problem) Ask it to fix that."
                } else if !latest.modId.isEmpty && mod.id != latest.modId {
                    notice = "The AI changed the mod's id to \(mod.id). Ask it to keep \(latest.modId)."
                } else {
                    latest.modId = mod.id
                    latest.name = mod.name
                    latest.version = mod.version
                    latest.summary = mod.summary ?? ""
                    latest.files = mod.files
                    latest.check = .none
                }
            } else if let problem = parsed.problem {
                notice = problem + " Ask it to send the whole mod again."
            }
            update(latest)
        } catch {
            notice = error.localizedDescription
        }
    }

    // MARK: GitHub

    /// Puts the draft's files in `user-mods/<id>/` on the fork's user-mods branch and starts its check.
    func upload(_ key: UUID, gitHub: GitHub) async {
        guard var draft = draft(key), !draft.files.isEmpty, busy == nil else { return }
        busy = key
        defer { busy = nil }
        do {
            let prefix = "user-mods/\(draft.modId)/"
            let files = try draft.files.map { (prefix + $0.path, try $0.bytes()) }
            try await Self.commit(gitHub, branch: "user-mods", prefix: prefix, files: files,
                                  message: "Mod maker: \(draft.modId) \(draft.version)", orphan: true)
            let since = Date()
            var run: Int?
            let path = "/actions/workflows/ios-user-mod.yml/dispatches"
            var body: [String: Any] = ["ref": "ios-build", "inputs": ["id": draft.modId], "return_run_details": true]
            let data: Data
            do {
                data = try await gitHub.data(path, method: "POST", json: body)
            } catch let error as GitHubError where error.code == 422 && error.message.contains("return_run_details") {
                body["return_run_details"] = nil
                data = try await gitHub.data(path, method: "POST", json: body)
            }
            run = (try? GitHub.decoder.decode(Dispatched.self, from: data))?.workflowRunId
            draft.check = .checking(run: run, since: since)
            update(draft)
        } catch {
            notice = error.localizedDescription
        }
    }

    /// One commit on `branch` setting `files` (path, bytes) and removing what else is under `prefix`.
    static func commit(_ gh: GitHub, branch: String, prefix: String, files: [(String, Data)],
                       message: String, orphan: Bool) async throws {
        var head: String?
        do {
            head = (try await gh.object("/git/ref/heads/\(branch)")["object"] as? [String: Any])?["sha"] as? String
        } catch let error as GitHubError where error.code == 404 || error.code == 409 {
            guard orphan else { throw error }
        }
        var baseTree: String?
        var stale = Set<String>()
        if let head {
            let commit = try await gh.object("/git/commits/\(head)")
            baseTree = (commit["tree"] as? [String: Any])?["sha"] as? String
            if let baseTree, !prefix.isEmpty {
                let tree = try await gh.object("/git/trees/\(baseTree)?recursive=1")
                for entry in tree["tree"] as? [[String: Any]] ?? [] {
                    if entry["type"] as? String == "blob", let path = entry["path"] as? String, path.hasPrefix(prefix) {
                        stale.insert(path)
                    }
                }
            }
        }
        var entries: [[String: Any]] = []
        for (path, bytes) in files {
            let blob = try await gh.object("/git/blobs", method: "POST",
                                           json: ["content": bytes.base64EncodedString(), "encoding": "base64"])
            guard let sha = blob["sha"] as? String else { throw AIError(message: "GitHub didn't keep \(path)") }
            entries.append(["path": path, "mode": "100644", "type": "blob", "sha": sha])
            stale.remove(path)
        }
        if baseTree != nil {
            for path in stale { entries.append(["path": path, "mode": "100644", "type": "blob", "sha": NSNull()]) }
        }
        var treeBody: [String: Any] = ["tree": entries]
        if let baseTree { treeBody["base_tree"] = baseTree }
        let tree = try await gh.object("/git/trees", method: "POST", json: treeBody)
        guard let treeSha = tree["sha"] as? String else { throw AIError(message: "GitHub didn't make the folder") }
        let commit = try await gh.object("/git/commits", method: "POST",
                                         json: ["message": message, "tree": treeSha, "parents": head.map { [$0] } ?? []])
        guard let commitSha = commit["sha"] as? String else { throw AIError(message: "GitHub didn't make the commit") }
        if head != nil {
            try await gh.data("/git/refs/heads/\(branch)", method: "PATCH", json: ["sha": commitSha, "force": false])
        } else {
            try await gh.data("/git/refs", method: "POST", json: ["ref": "refs/heads/\(branch)", "sha": commitSha])
        }
    }

    var checking: Bool {
        drafts.contains { if case .checking = $0.check { return true } else { return false } }
    }

    /// Follows every check that's running.
    func poll(gitHub: GitHub) async {
        for draft in drafts {
            guard case .checking(let run, let since) = draft.check else { continue }
            var next = draft
            do {
                var id = run
                if id == nil {
                    let list: RunList = try await gitHub.get("/actions/workflows/ios-user-mod.yml/runs?per_page=10&event=workflow_dispatch")
                    id = list.workflowRuns.first { $0.createdAt >= since.addingTimeInterval(-60) }?.id
                    if let id { next.check = .checking(run: id, since: since) }
                }
                guard let id else {
                    if Date().timeIntervalSince(since) > 300 { next.check = .failed(log: "GitHub never started the check.") }
                    update(next)
                    continue
                }
                let runInfo = try await gitHub.object("/actions/runs/\(id)")
                guard runInfo["status"] as? String == "completed" else {
                    if next != draft { update(next) }
                    continue
                }
                switch runInfo["conclusion"] as? String {
                case "success":
                    next.check = .passed(version: draft.version)
                case "cancelled":
                    next.check = .none
                default:
                    next.check = .failed(log: await Self.errors(run: id, gitHub: gitHub))
                }
                update(next)
            } catch {
                notice = error.localizedDescription
            }
        }
    }

    /// The failed step's errors from a run's log, for the AI to read.
    static func errors(run: Int, gitHub: GitHub) async -> String {
        guard let jobs: JobList = try? await gitHub.get("/actions/runs/\(run)/jobs"),
              let failed = try? await gitHub.object("/actions/runs/\(run)/jobs"),
              let job = (failed["jobs"] as? [[String: Any]])?.first(where: { $0["conclusion"] as? String == "failure" }),
              let jobId = job["id"] as? Int else {
            return "The check failed, and its log couldn't be read."
        }
        let step = jobs.jobs.first { $0.conclusion == "failure" }?.steps?.first { $0.conclusion == "failure" }?.name ?? "a step"
        guard let text = await Self.log(job: jobId, gitHub: gitHub) else {
            return "The step “\(step)” failed, and its log couldn't be read."
        }
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map { line -> String in
            // "2026-11-07T15:04:05.1234567Z message"
            let s = String(line)
            if s.count > 29, s.dropFirst(4).hasPrefix("-"), let space = s.firstIndex(of: " ") { return String(s[s.index(after: space)...]) }
            return s
        }
        var keep = [String]()
        var after = 0
        for line in lines {
            let lower = line.lowercased()
            if lower.contains("error") || line.contains("-->") || lower.contains("refused") || lower.contains("::error::") {
                keep.append(line)
                after = 8
            } else if after > 0 {
                keep.append(line)
                after -= 1
            }
        }
        if keep.isEmpty { keep = Array(lines.suffix(60)) }
        var out = "The step “\(step)” failed:\n" + keep.joined(separator: "\n")
        if out.count > 7000 { out = String(out.prefix(7000)) + "\n…" }
        return out
    }

    /// A job's log. GitHub answers with a redirect to its storage, which must not get the token.
    static func log(job: Int, gitHub: GitHub) async -> String? {
        guard let url = URL(string: "https://api.github.com/repos/\(gitHub.repo)/actions/jobs/\(job)/logs") else { return nil }
        var request = URLRequest(url: url)
        request.setValue("Bearer \(gitHub.token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        guard let result = try? await URLSession.shared.data(for: request, delegate: NoTokenOnRedirect()),
              (result.1 as? HTTPURLResponse)?.statusCode == 200 else { return nil }
        return String(data: result.0, encoding: .utf8)
    }

    // MARK: Horizon's registry

    /// Makes the mod its own public repository with a release, as Horizon's registry lists mods,
    /// and returns the submit page to open.
    func prepareSubmission(_ key: UUID, gitHub: GitHub) async -> URL? {
        guard let draft = draft(key), case .passed = draft.check, busy == nil else { return nil }
        busy = key
        defer { busy = nil }
        do {
            let me = try JSONSerialization.jsonObject(with: try await gitHub.call("https://api.github.com/user")) as? [String: Any]
            guard let login = me?["login"] as? String else { throw AIError(message: "GitHub didn't say who the token is.") }
            let name = "horizon-mod-\(draft.modId.replacingOccurrences(of: "_", with: "-"))"
            let repo = GitHub(repo: "\(login)/\(name)", token: gitHub.token)
            do {
                try await repo.data("")
            } catch let error as GitHubError where error.code == 404 {
                do {
                    try await gitHub.call("https://api.github.com/user/repos", method: "POST", json: [
                        "name": name, "description": String(draft.summary.prefix(300)), "auto_init": true,
                        "license_template": "mit", "has_wiki": false,
                    ])
                } catch let error as GitHubError where error.code == 403 || error.code == 404 {
                    throw AIError(message: "Your token can't create repositories. Make one with All repositories access and Administration + Contents: Read and write, or create github.com/\(login)/\(name) yourself and try again.")
                }
                try? await Task.sleep(nanoseconds: 3_000_000_000)
            }
            var files = try draft.files.map { ($0.path, try $0.bytes()) }
            let readme = "# \(draft.name)\n\n\(draft.summary)\n\nA \(draft.kind == .content ? "content" : "native") mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3), made with Horizon Mods' mod maker.\n"
            files.append(("README.md", Data(readme.utf8)))
            try await Self.commit(repo, branch: "main", prefix: "", files: files,
                                  message: "\(draft.name) \(draft.version)", orphan: false)
            // The file GitHub's check built, from the fork's release.
            let tag = "mod-\(draft.modId)-v\(draft.version)"
            let release = try await gitHub.object("/releases/tags/\(tag)")
            guard let asset = (release["assets"] as? [[String: Any]])?.first,
                  let assetName = asset["name"] as? String,
                  let assetURL = (asset["browser_download_url"] as? String).flatMap(URL.init(string:)) else {
                throw AIError(message: "The checked file isn't in \(tag) yet.")
            }
            let (bytes, _) = try await URLSession.shared.data(from: assetURL)
            let made = try await repo.object("/releases", method: "POST", json: [
                "tag_name": "v\(draft.version)", "name": "\(draft.name) \(draft.version)",
                "body": draft.summary, "draft": false, "prerelease": false,
            ])
            guard let releaseId = made["id"] as? Int else { throw AIError(message: "GitHub didn't make the release.") }
            try await repo.call("https://uploads.github.com/repos/\(login)/\(name)/releases/\(releaseId)/assets?name=\(assetName)",
                                method: "POST", body: bytes, contentType: "application/octet-stream")
            notice = "github.com/\(login)/\(name) is ready. On Horizon's submit page, sign in with GitHub and give it that repository."
            return URL(string: "https://playhorizon.dev/mods/submit")
        } catch {
            notice = error.localizedDescription
            return nil
        }
    }
}

/// Drops the GitHub token when a request is sent on to another host.
final class NoTokenOnRedirect: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        var next = request
        if next.url?.host != "api.github.com" { next.setValue(nil, forHTTPHeaderField: "Authorization") }
        completionHandler(next)
    }
}
