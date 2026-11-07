import Foundation

// MARK: Horizon's mod registry (https://horizon-rust.github.io/horizon-mods/index.json)

/// Decodes one element, or nothing when it doesn't fit, so one odd project can't hide the rest.
struct Lossy<T: Decodable>: Decodable {
    let value: T?
    init(from decoder: Decoder) throws { value = try? T(from: decoder) }
}

struct RegistryIndex: Decodable {
    let projects: [Lossy<NativeMod>]
}

struct ModVersion: Decodable {
    let version: String
    let game: String
    let published: String
    let revoked: Bool
    /// Ids of the projects this version needs.
    let requires: [String]

    private enum Keys: String, CodingKey { case version, game, published, revoked, requires }
    private struct Requirement: Decodable { let id: String }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        version = (try? c.decode(String.self, forKey: .version)) ?? "?"
        game = (try? c.decode(String.self, forKey: .game)) ?? ""
        published = (try? c.decode(String.self, forKey: .published)) ?? ""
        if c.contains(.revoked), (try? c.decodeNil(forKey: .revoked)) != true {
            revoked = (try? c.decode(Bool.self, forKey: .revoked)) ?? true
        } else {
            revoked = false
        }
        requires = ((try? c.decode([Requirement].self, forKey: .requires)) ?? []).map(\.id)
    }
}

struct NativeMod: Decodable, Identifiable {
    let id: String
    let kind: String
    let name: String
    let summary: String
    let description: String
    let authors: [String]
    let license: String
    let source: String
    let downloads: Int
    /// Its icon, `https://` (PNG or JPEG).
    let icon: String?
    /// Its gallery, `https://` images in order.
    let screenshots: [String]
    var versions: [ModVersion]

    private enum Keys: String, CodingKey {
        case id, kind, name, summary, description, authors, license, source, downloads, icon, screenshots, versions
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        id = try c.decode(String.self, forKey: .id)
        kind = (try? c.decode(String.self, forKey: .kind)) ?? ""
        name = (try? c.decode(String.self, forKey: .name)) ?? id
        summary = (try? c.decode(String.self, forKey: .summary)) ?? ""
        description = (try? c.decode(String.self, forKey: .description)) ?? ""
        authors = (try? c.decode([String].self, forKey: .authors)) ?? []
        license = (try? c.decode(String.self, forKey: .license)) ?? ""
        source = (try? c.decode(String.self, forKey: .source)) ?? ""
        downloads = (try? c.decode(Int.self, forKey: .downloads)) ?? 0
        icon = (try? c.decode(String.self, forKey: .icon)).flatMap { $0.hasPrefix("https://") ? $0 : nil }
        screenshots = ((try? c.decode([String].self, forKey: .screenshots)) ?? []).filter { $0.hasPrefix("https://") }
        versions = ((try? c.decode([Lossy<ModVersion>].self, forKey: .versions)) ?? []).compactMap(\.value)
    }

    /// This mod with only its versions for `game`, or nil when it has none.
    func forGame(_ game: String) -> NativeMod? {
        var mod = self
        mod.versions = versions.filter { $0.game == game && !$0.revoked }
        return mod.versions.isEmpty ? nil : mod
    }

    var latest: ModVersion? { versions.max { $0.published < $1.published } }

    var byline: String {
        var parts: [String] = []
        if !authors.isEmpty { parts.append("by " + authors.joined(separator: ", ")) }
        if let v = latest?.version { parts.append("v" + v) }
        return parts.joined(separator: " · ")
    }
}

// MARK: GitHub Actions

struct RunList: Decodable { let workflowRuns: [Run] }

struct Run: Decodable, Identifiable {
    let id: Int
    let runNumber: Int
    let status: String
    let conclusion: String?
    let createdAt: Date
    let htmlUrl: String
    let event: String

    enum State { case running, success, failed, cancelled }

    var state: State {
        if status != "completed" { return .running }
        switch conclusion {
        case "success": return .success
        case "cancelled": return .cancelled
        default: return .failed
        }
    }
}

struct JobList: Decodable { let jobs: [Job] }

struct Job: Decodable {
    let name: String
    let status: String
    let conclusion: String?
    let steps: [Step]?
}

struct Step: Decodable {
    let name: String
    let status: String
    let conclusion: String?
}

struct Release: Decodable {
    let body: String?
    let assets: [Asset]

    /// The mod ids in the release notes' first line: "…native mods: `a b`."
    static func mods(_ body: String?) -> [String]? {
        guard let body, let start = body.firstIndex(of: "`") else { return nil }
        let rest = body[body.index(after: start)...]
        guard let end = rest.firstIndex(of: "`") else { return nil }
        return rest[..<end].split(separator: " ").map(String.init)
    }
}

struct Asset: Decodable {
    let name: String
    let size: Int
    let browserDownloadUrl: String
}

struct Dispatched: Decodable { let workflowRunId: Int }

/// What the Builds tab shows about one run beyond the run itself.
struct RunDetail {
    var progress: Double = 0
    var step: String = "Waiting for GitHub"
    var mods: [String]?
    var asset: Asset?
    var failure: String?
    /// Nothing more to fetch for this run.
    var settled = false
}

/// How far a run is, from its three jobs (pick → stage → ipa).
enum BuildProgress {
    static let jobs: [(name: String, weight: Double, label: String, machine: String)] = [
        ("pick", 0.05, "Getting ready", "machine"),
        ("stage", 0.35, "Adding the mods", "machine"),
        ("ipa", 0.60, "Building for iPhone", "Mac"),
    ]

    static func of(_ list: [Job]) -> (Double, String) {
        var total = 0.0
        var label = "Waiting for GitHub"
        for spec in jobs {
            guard let job = list.first(where: { $0.name == spec.name }) else { continue }
            if job.status == "completed" {
                total += spec.weight
                continue
            }
            let steps = job.steps ?? []
            if !steps.isEmpty {
                let done = steps.filter { $0.status == "completed" }.count
                total += spec.weight * Double(done) / Double(steps.count)
            }
            if job.status == "in_progress" {
                let current = steps.first { $0.status == "in_progress" }?.name
                label = spec.label + (current.map { ": " + $0 } ?? "")
            } else {
                label = spec.label + ": waiting for a " + spec.machine
            }
            break
        }
        return (min(total, 1), label)
    }

    static func failure(_ list: [Job]) -> String {
        switch list.first(where: { $0.conclusion == "failure" })?.name {
        case "pick":
            return "GitHub couldn't read which mods to build."
        case "stage":
            return "Adding the mods failed. A mod made for another Horizon version may not apply: remove it and build again."
        case "ipa":
            return "Compiling for iPhone failed. A mod may use something iOS doesn't have: remove it and build again."
        default:
            return "This build didn't finish."
        }
    }
}

// MARK: Instances

/// A release in the fork, as the Instances tab reads it.
struct ReleaseEntry: Decodable {
    let tagName: String
    let body: String?
    let assets: [Asset]
    let createdAt: Date
}

/// One installable Horizon with native mods: its own app identifier
/// (`dev.horizon.game.i<id>`) and link (`horizon-i<id>://`), so several sit side by side.
struct Instance: Identifiable {
    let id: String
    var name: String
    var mods: [String]
    var build: Int
    var asset: Asset?
    var date: Date

    var link: URL? { URL(string: "horizon-i\(id)://") }

    private struct Marker: Decodable {
        let id: String
        let name: String
        let mods: [String]
    }

    /// The instance a native build's release notes name (`<!-- horizon-instance {…} -->`).
    static func of(_ release: ReleaseEntry) -> Instance? {
        guard release.tagName.hasPrefix("ios-native-r"),
              let build = Int(release.tagName.dropFirst("ios-native-r".count)),
              let body = release.body,
              let start = body.range(of: "<!-- horizon-instance "),
              let end = body.range(of: " -->", range: start.upperBound..<body.endIndex),
              let marker = try? JSONDecoder().decode(Marker.self, from: Data(body[start.upperBound..<end.lowerBound].utf8))
        else { return nil }
        return Instance(id: marker.id, name: marker.name, mods: marker.mods, build: build,
                        asset: release.assets.first { $0.name.hasSuffix(".ipa") }, date: release.createdAt)
    }
}
