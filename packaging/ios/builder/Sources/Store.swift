import Foundation
import SwiftUI

enum AppTab: Hashable { case mods, create, builds, instances, settings }

/// Everything the app knows: the mod list, the picks, the token, and the builds on GitHub.
@MainActor
final class Store: ObservableObject {
    static let workflow = "ios-native-build.yml"
    static let branch = "ios-build"
    static let game = "26.3"
    static let registry = URL(string: "https://horizon-rust.github.io/horizon-mods/index.json")!

    @Published var tab: AppTab = .mods
    /// Instances run inside LiveContainer (one app slot for all of them) instead of as apps of their own.
    @Published var useLiveContainer: Bool {
        didSet { UserDefaults.standard.set(useLiveContainer, forKey: "useLiveContainer") }
    }
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
    @Published var registryMods: [NativeMod] = []
    /// The player's own native mods (the mod maker's, published by ios-user-mod.yml).
    @Published var userMods: [NativeMod] = []
    var mods: [NativeMod] { userMods + registryMods.filter { m in !userMods.contains { $0.id == m.id } } }
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
    /// Every instance built so far, newest first (one entry per instance, its latest build).
    @Published var instances: [Instance] = []
    /// Instances taken off the list on this iPhone.
    @Published var hidden: Set<String> {
        didSet { UserDefaults.standard.set(Array(hidden).sorted(), forKey: "hiddenInstances") }
    }

    /// The instance each run builds, by run id: "<id> <name>".
    private var runInstance: [String: String] {
        didSet { UserDefaults.standard.set(runInstance, forKey: "runInstance") }
    }

    /// The mods each run was started with, by run id (GitHub doesn't keep a run's inputs).
    private var runMods: [String: [String]] {
        didSet { UserDefaults.standard.set(runMods, forKey: "runMods") }
    }
    /// A build started without GitHub saying which run it became: the next new run is it.
    private var pending: (mods: [String], instance: String, since: Date)?

    init() {
        let defaults = UserDefaults.standard
        let bundled = Bundle.main.object(forInfoDictionaryKey: "HorizonRepo") as? String ?? ""
        repo = defaults.string(forKey: "repo") ?? bundled
        token = TokenStore.load()
        picked = Set(defaults.stringArray(forKey: "picked") ?? [])
        runMods = defaults.dictionary(forKey: "runMods") as? [String: [String]] ?? [:]
        runInstance = defaults.dictionary(forKey: "runInstance") as? [String: String] ?? [:]
        hidden = Set(defaults.stringArray(forKey: "hiddenInstances") ?? [])
        useLiveContainer = defaults.bool(forKey: "useLiveContainer")
        // `-HorizonTab builds` opens on a tab (the preview screenshots use it).
        switch defaults.string(forKey: "HorizonTab") {
        case "builds": tab = .builds
        case "instances": tab = .instances
        case "settings": tab = .settings
        case "create": tab = .create
        default: break
        }
    }

    var gitHubClient: GitHub { gitHub }

    private var gitHub: GitHub {
        GitHub(repo: repo.trimmingCharacters(in: .whitespaces),
               token: token.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    var hasToken: Bool { !token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    var activeCount: Int { runs.filter { $0.state == .running }.count }
    /// The picked mods that are in the list, in the list's order.
    var pickedInOrder: [String] { mods.map(\.id).filter(picked.contains) }

    /// The link that opens the Horizon with this app identifier: in LiveContainer (which keeps each
    /// app in a folder named after its identifier), or the app's own link.
    func launchURL(bundle: String, scheme: String) -> URL? {
        useLiveContainer
            ? URL(string: "livecontainer://livecontainer-launch?bundle-name=\(bundle).app")
            : URL(string: "\(scheme)://")
    }

    var shownInstances: [Instance] { instances.filter { !hidden.contains($0.id) } }

    /// The instance a run builds: its id and name.
    func instance(of run: Run) -> (id: String, name: String)? {
        guard let entry = runInstance[String(run.id)], let space = entry.firstIndex(of: " ") else { return nil }
        return (String(entry[..<space]), String(entry[entry.index(after: space)...]))
    }

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
            registryMods = index.projects
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

    /// Starts a build of the picked mods: into `instance` (updating it) or, when nil, a new one.
    func build(instance: String?, name: String) async {
        let mods = pickedInOrder
        guard !mods.isEmpty, hasToken, !starting else { return }
        starting = true
        defer { starting = false }
        let since = Date()
        let id = instance ?? String(Int.random(in: 100_000...999_999))
        let name = name.trimmingCharacters(in: .whitespaces).isEmpty ? "Horizon \(id)" : name
        let path = "/actions/workflows/\(Self.workflow)/dispatches"
        var body: [String: Any] = [
            "ref": Self.branch,
            "inputs": ["mods": mods.joined(separator: ","), "instance": id, "name": name],
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
                runInstance[String(run.workflowRunId)] = "\(id) \(name)"
            } else {
                pending = (mods, "\(id) \(name)", since)
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
                runInstance[String(run.id)] = p.instance
                pending = nil
            }
            runs = shown
            buildsError = nil
            // The newest few: without a token GitHub allows only 60 requests an hour.
            for run in shown.prefix(hasToken ? 10 : 3) {
                await refreshDetail(run)
            }
            await refreshInstances()
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

    func refreshInstances() async {
        guard let releases: [ReleaseEntry] = try? await gitHub.get("/releases?per_page=100") else { return }
        var mine: [String: (Date, NativeMod)] = [:]
        for release in releases {
            guard let mod = NativeMod.userMod(release), (mine[mod.id]?.0 ?? .distantPast) < release.createdAt else { continue }
            mine[mod.id] = (release.createdAt, mod)
        }
        userMods = mine.values.map(\.1).sorted { $0.name < $1.name }
        var newest: [String: Instance] = [:]
        for instance in releases.compactMap(Instance.of) where (newest[instance.id]?.build ?? -1) < instance.build {
            newest[instance.id] = instance
        }
        instances = newest.values.sorted { $0.date > $1.date }
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
