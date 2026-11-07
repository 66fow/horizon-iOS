import Foundation
import SwiftUI

enum AppTab: Hashable { case mods, builds, settings }

/// Everything the app knows: the mod list, the picks, the token, and the builds on GitHub.
@MainActor
final class Store: ObservableObject {
    static let workflow = "ios-native-build.yml"
    static let branch = "ios-build"
    static let game = "26.3"
    static let registry = URL(string: "https://horizon-rust.github.io/horizon-mods/index.json")!

    @Published var tab: AppTab = .mods
    @Published var repo: String {
        didSet {
            UserDefaults.standard.set(repo, forKey: "repo")
            runs = []
            details = [:]
        }
    }
    @Published var token: String {
        didSet { TokenStore.save(token.trimmingCharacters(in: .whitespacesAndNewlines)) }
    }
    @Published var mods: [NativeMod] = []
    @Published var modsError: String?
    @Published var loadingMods = false
    @Published var picked: Set<String> {
        didSet { UserDefaults.standard.set(Array(picked).sorted(), forKey: "picked") }
    }
    @Published var runs: [Run] = []
    @Published var details: [Int: RunDetail] = [:]
    @Published var buildsError: String?
    @Published var starting = false
    /// A build that couldn't start, shown as an alert.
    @Published var notice: String?

    /// The mods each run was started with, by run id (GitHub doesn't keep a run's inputs).
    private var runMods: [String: [String]] {
        didSet { UserDefaults.standard.set(runMods, forKey: "runMods") }
    }
    /// A build started without GitHub saying which run it became: the next new run is it.
    private var pending: (mods: [String], since: Date)?

    init() {
        let defaults = UserDefaults.standard
        let bundled = Bundle.main.object(forInfoDictionaryKey: "HorizonRepo") as? String ?? ""
        repo = defaults.string(forKey: "repo") ?? bundled
        token = TokenStore.load()
        picked = Set(defaults.stringArray(forKey: "picked") ?? [])
        runMods = defaults.dictionary(forKey: "runMods") as? [String: [String]] ?? [:]
    }

    private var gitHub: GitHub {
        GitHub(repo: repo.trimmingCharacters(in: .whitespaces),
               token: token.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    var hasToken: Bool { !token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    var activeCount: Int { runs.filter { $0.state == .running }.count }
    /// The picked mods that are in the list, in the list's order.
    var pickedInOrder: [String] { mods.map(\.id).filter(picked.contains) }

    func names(_ ids: [String]) -> String {
        ids.map { id in mods.first { $0.id == id }?.name ?? id }.joined(separator: ", ")
    }

    /// Picks or unpicks `mod`; picking also picks the native mods it needs.
    func toggle(_ mod: NativeMod) {
        if picked.contains(mod.id) {
            picked.remove(mod.id)
            return
        }
        var add = [mod.id]
        var seen = Set<String>()
        while let id = add.popLast() {
            guard seen.insert(id).inserted, let m = mods.first(where: { $0.id == id }) else { continue }
            picked.insert(id)
            add += m.latest?.requires ?? []
        }
    }

    // MARK: Mods

    func loadMods() async {
        loadingMods = true
        defer { loadingMods = false }
        do {
            var request = URLRequest(url: Self.registry)
            request.cachePolicy = .reloadRevalidatingCacheData
            let (data, response) = try await URLSession.shared.data(for: request)
            let code = (response as? HTTPURLResponse)?.statusCode ?? 0
            guard code == 200 else {
                throw GitHubError(code: 0, message: "The mod list didn't load (error \(code)).", repo: repo)
            }
            let index = try JSONDecoder().decode(RegistryIndex.self, from: data)
            mods = index.projects
                .compactMap(\.value)
                .filter { $0.kind == "native-mod" }
                .compactMap { $0.forGame(Self.game) }
                .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
            modsError = nil
        } catch {
            modsError = error.localizedDescription
        }
    }

    // MARK: Builds

    func build() async {
        let mods = pickedInOrder
        guard !mods.isEmpty, hasToken, !starting else { return }
        starting = true
        defer { starting = false }
        let since = Date()
        let path = "/actions/workflows/\(Self.workflow)/dispatches"
        var body: [String: Any] = [
            "ref": Self.branch,
            "inputs": ["mods": mods.joined(separator: ",")],
            "return_run_details": true,
        ]
        do {
            let data: Data
            do {
                data = try await gitHub.data(path, method: "POST", json: body)
            } catch let error as GitHubError where error.code == 422 && error.message.contains("return_run_details") {
                body["return_run_details"] = nil
                data = try await gitHub.data(path, method: "POST", json: body)
            }
            if let run = try? GitHub.decoder.decode(Dispatched.self, from: data) {
                runMods[String(run.workflowRunId)] = mods
            } else {
                pending = (mods, since)
            }
            tab = .builds
            try? await Task.sleep(nanoseconds: 3_000_000_000)
            await refreshRuns()
        } catch {
            notice = error.localizedDescription
        }
    }

    func cancel(_ run: Run) async {
        do {
            try await gitHub.data("/actions/runs/\(run.id)/cancel", method: "POST")
            try? await Task.sleep(nanoseconds: 2_000_000_000)
            await refreshRuns()
        } catch {
            buildsError = error.localizedDescription
        }
    }

    func refreshRuns() async {
        guard !repo.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        do {
            let list: RunList = try await gitHub.get("/actions/workflows/\(Self.workflow)/runs?per_page=20")
            // Issue edits that weren't "Build now" start a run that skips everything.
            let shown = list.workflowRuns.filter { $0.conclusion != "skipped" }
            if let p = pending, let run = shown.first(where: {
                $0.event == "workflow_dispatch" && $0.createdAt >= p.since.addingTimeInterval(-60)
                    && runMods[String($0.id)] == nil
            }) {
                runMods[String(run.id)] = p.mods
                pending = nil
            }
            runs = shown
            buildsError = nil
            // The newest few: without a token GitHub allows only 60 requests an hour.
            for run in shown.prefix(hasToken ? 10 : 3) {
                await refreshDetail(run)
            }
        } catch {
            buildsError = error.localizedDescription
        }
    }

    private func refreshDetail(_ run: Run) async {
        var detail = details[run.id] ?? RunDetail()
        if detail.mods == nil { detail.mods = runMods[String(run.id)] }
        guard !detail.settled else {
            details[run.id] = detail
            return
        }
        switch run.state {
        case .running:
            if let jobs: JobList = try? await gitHub.get("/actions/runs/\(run.id)/jobs") {
                (detail.progress, detail.step) = BuildProgress.of(jobs.jobs)
            }
        case .success:
            if let release: Release = try? await gitHub.get("/releases/tags/ios-native-r\(run.runNumber)") {
                detail.asset = release.assets.first { $0.name.hasSuffix(".ipa") }
                if detail.mods == nil { detail.mods = Release.mods(release.body) }
                detail.settled = true
            }
        case .failed:
            if let jobs: JobList = try? await gitHub.get("/actions/runs/\(run.id)/jobs") {
                detail.failure = BuildProgress.failure(jobs.jobs)
                detail.settled = true
            }
        case .cancelled:
            detail.settled = true
        }
        details[run.id] = detail
    }

    /// Whether the token is accepted, and a line saying so.
    func checkToken() async -> (Bool, String) {
        do {
            try await gitHub.data("/actions/workflows/\(Self.workflow)")
            return (true, "The token works. GitHub checks it may start builds when you tap Build.")
        } catch {
            return (false, error.localizedDescription)
        }
    }
}
