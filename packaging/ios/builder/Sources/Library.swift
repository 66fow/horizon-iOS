import Compression
import Foundation
import SwiftUI

/// A Horizon build on GitHub: a release of the fork with a Horizon .ipa (ios-r<N> or ios-native-…).
struct RemoteBuild: Identifiable, Equatable {
    let tag: String
    let title: String
    let date: Date
    let size: Int
    let url: String
    let mods: [String]

    var id: String { tag }
    /// Its folder in Builds/: the tag without "ios-" (r31, native-r4…).
    var folder: String { tag.hasPrefix("ios-") ? String(tag.dropFirst(4)) : tag }
    var native: Bool { tag.hasPrefix("ios-native-") }
}

/// A build unpacked on this iPhone, in Documents/Builds/<folder>/ (Files → Horizon Mods → Builds).
struct LocalBuild: Codable, Identifiable, Equatable {
    let tag: String
    let folder: String
    let title: String
    let date: Date
    let mods: [String]
    /// The .app folder's name inside the build's folder.
    let app: String
    var bytes: Int64

    var id: String { folder }
}

@MainActor
final class Library: ObservableObject {
    @Published var remote: [RemoteBuild] = []
    @Published var local: [LocalBuild] = []
    @Published var progress: [String: Double] = [:]
    @Published var error: String?
    @Published var loading = false

    static var documents: URL { FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0] }
    static var builds: URL { documents.appendingPathComponent("Builds") }

    /// The prepared game files are in Documents/resources (the Files app shows them).
    var hasGameFiles: Bool {
        FileManager.default.fileExists(atPath: Self.documents.appendingPathComponent("resources/horizon-resources.json").path)
    }

    init() { scan() }

    func scan() {
        let fm = FileManager.default
        try? fm.createDirectory(at: Self.builds, withIntermediateDirectories: true)
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let folders = (try? fm.contentsOfDirectory(at: Self.builds, includingPropertiesForKeys: nil)) ?? []
        local = folders.compactMap { folder in
            guard let data = try? Data(contentsOf: folder.appendingPathComponent("build.json")) else { return nil }
            return try? decoder.decode(LocalBuild.self, from: data)
        }
        .sorted { $0.date > $1.date }
    }

    func refresh(gitHub: GitHub) async {
        loading = true
        defer { loading = false }
        do {
            let releases: [ReleaseEntry] = try await gitHub.get("/releases?per_page=100")
            remote = releases.compactMap { r in
                guard r.tagName.hasPrefix("ios-r") || r.tagName.hasPrefix("ios-native-"),
                      let ipa = r.assets.first(where: { $0.name.hasSuffix(".ipa") && $0.name.hasPrefix("Horizon-") }) else { return nil }
                let mods = Instance.of(r)?.mods ?? Release.mods(r.body) ?? []
                let title = r.tagName.hasPrefix("ios-native-") ? "Native \(r.tagName.dropFirst(11))" : String(r.tagName.dropFirst(4))
                return RemoteBuild(tag: r.tagName, title: title, date: r.createdAt, size: ipa.size,
                                   url: ipa.browserDownloadUrl, mods: r.tagName.hasPrefix("ios-native-") ? mods : [])
            }
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }

    func isLocal(_ build: RemoteBuild) -> Bool { local.contains { $0.folder == build.folder } }

    /// Downloads `build`'s .ipa and unpacks its app into Builds/<folder>/.
    func get(_ build: RemoteBuild) async {
        guard progress[build.tag] == nil, let url = URL(string: build.url) else { return }
        progress[build.tag] = 0
        defer { progress[build.tag] = nil }
        do {
            let file = try await Downloader.download(url) { [weak self] fraction in
                Task { @MainActor in self?.progress[build.tag] = fraction * 0.9 }
            }
            defer { try? FileManager.default.removeItem(at: file) }
            progress[build.tag] = 0.92
            let target = Self.builds.appendingPathComponent(build.folder)
            let staging = Self.builds.appendingPathComponent(".\(build.folder).part")
            try? FileManager.default.removeItem(at: staging)
            let (app, bytes) = try await Task.detached(priority: .userInitiated) {
                try Unzip.app(from: file, into: staging)
            }.value
            let meta = LocalBuild(tag: build.tag, folder: build.folder, title: build.title, date: build.date,
                                  mods: build.mods, app: app, bytes: bytes)
            let encoder = JSONEncoder()
            encoder.dateEncodingStrategy = .iso8601
            encoder.outputFormatting = .prettyPrinted
            try encoder.encode(meta).write(to: staging.appendingPathComponent("build.json"))
            try? FileManager.default.removeItem(at: target)
            try FileManager.default.moveItem(at: staging, to: target)
            scan()
        } catch {
            self.error = "\(build.title): \(error.localizedDescription)"
        }
    }

    func delete(_ build: LocalBuild) {
        try? FileManager.default.removeItem(at: Self.builds.appendingPathComponent(build.folder))
        scan()
    }
}

/// One file download with progress.
final class Downloader: NSObject, URLSessionDownloadDelegate {
    private var done: CheckedContinuation<URL, Error>?
    private let progress: (Double) -> Void

    private init(progress: @escaping (Double) -> Void) { self.progress = progress }

    static func download(_ url: URL, progress: @escaping (Double) -> Void) async throws -> URL {
        let downloader = Downloader(progress: progress)
        let session = URLSession(configuration: .default, delegate: downloader, delegateQueue: nil)
        defer { session.finishTasksAndInvalidate() }
        return try await withCheckedThrowingContinuation { done in
            downloader.done = done
            session.downloadTask(with: url).resume()
        }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64,
                    totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        if totalBytesExpectedToWrite > 0 { progress(Double(totalBytesWritten) / Double(totalBytesExpectedToWrite)) }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        let code = (downloadTask.response as? HTTPURLResponse)?.statusCode ?? 0
        guard code == 200 else {
            done?.resume(throwing: AIError(message: "the download failed (error \(code))"))
            done = nil
            return
        }
        // The file is removed when this returns: keep it.
        let kept = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".ipa")
        do {
            try FileManager.default.moveItem(at: location, to: kept)
            done?.resume(returning: kept)
        } catch {
            done?.resume(throwing: error)
        }
        done = nil
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let error {
            done?.resume(throwing: error)
            done = nil
        }
    }
}

/// Reads an .ipa (a zip) and writes the app inside it (`Payload/<name>.app/…`).
enum Unzip {
    struct Problem: LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    /// The .app's folder name and the bytes written.
    static func app(from ipa: URL, into folder: URL) throws -> (String, Int64) {
        let data = try Data(contentsOf: ipa, options: .alwaysMapped)
        let fm = FileManager.default
        try fm.createDirectory(at: folder, withIntermediateDirectories: true)
        var appName: String?
        var written: Int64 = 0
        try data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) in
            func u16(_ at: Int) throws -> Int {
                guard at >= 0, at + 2 <= raw.count else { throw Problem(message: "the .ipa is cut short") }
                return Int(raw.loadUnaligned(fromByteOffset: at, as: UInt16.self).littleEndian)
            }
            func u32(_ at: Int) throws -> Int {
                guard at >= 0, at + 4 <= raw.count else { throw Problem(message: "the .ipa is cut short") }
                return Int(raw.loadUnaligned(fromByteOffset: at, as: UInt32.self).littleEndian)
            }
            // The end of the central directory, within the last 64 KiB.
            var end = -1
            var at = raw.count - 22
            while at >= max(0, raw.count - 65_558) {
                if try u32(at) == 0x0605_4b50 { end = at; break }
                at -= 1
            }
            guard end >= 0 else { throw Problem(message: "the .ipa isn't a zip") }
            let count = try u16(end + 10)
            var entry = try u32(end + 16)
            for _ in 0..<count {
                guard try u32(entry) == 0x0201_4b50 else { throw Problem(message: "the .ipa's directory is damaged") }
                let method = try u16(entry + 10)
                let packed = try u32(entry + 20)
                let size = try u32(entry + 24)
                let nameLength = try u16(entry + 28)
                let extra = try u16(entry + 30)
                let comment = try u16(entry + 32)
                let attributes = try u32(entry + 38)
                let local = try u32(entry + 42)
                guard entry + 46 + nameLength <= raw.count,
                      let name = String(bytes: raw[(entry + 46)..<(entry + 46 + nameLength)], encoding: .utf8) else {
                    throw Problem(message: "the .ipa's directory is damaged")
                }
                entry += 46 + nameLength + extra + comment
                // Only the app: Payload/<name>.app/…; no links, no paths that climb out.
                let parts = name.split(separator: "/", omittingEmptySubsequences: false)
                guard parts.count >= 2, parts[0] == "Payload", parts[1].hasSuffix(".app") else { continue }
                if parts.contains(where: { $0 == ".." || $0 == "." }) || name.hasPrefix("/") {
                    throw Problem(message: "the .ipa has an unsafe path: \(name)")
                }
                if (attributes >> 16) & 0o170000 == 0o120000 { continue }
                appName = appName ?? String(parts[1])
                let relative = parts.dropFirst(1).joined(separator: "/")
                let to = folder.appendingPathComponent(relative)
                if name.hasSuffix("/") {
                    try fm.createDirectory(at: to, withIntermediateDirectories: true)
                    continue
                }
                try fm.createDirectory(at: to.deletingLastPathComponent(), withIntermediateDirectories: true)
                let header = local
                guard try u32(header) == 0x0403_4b50 else { throw Problem(message: "the .ipa is damaged at \(name)") }
                let start = try header + 30 + u16(header + 26) + u16(header + 28)
                guard start + packed <= raw.count else { throw Problem(message: "the .ipa is cut short at \(name)") }
                let source = UnsafeRawBufferPointer(rebasing: raw[start..<(start + packed)])
                var out = Data(count: size)
                switch method {
                case 0:
                    guard packed == size else { throw Problem(message: "\(name) has the wrong size") }
                    out = Data(source)
                case 8:
                    let n = out.withUnsafeMutableBytes { dst -> Int in
                        guard size > 0 else { return 0 }
                        return compression_decode_buffer(dst.bindMemory(to: UInt8.self).baseAddress!, size,
                                                         source.bindMemory(to: UInt8.self).baseAddress!, packed,
                                                         nil, COMPRESSION_ZLIB)
                    }
                    guard n == size else { throw Problem(message: "\(name) didn't unpack") }
                default:
                    throw Problem(message: "\(name) is packed in a way this can't read (\(method))")
                }
                try out.write(to: to)
                written += Int64(size)
            }
        }
        guard let appName else { throw Problem(message: "there's no app in the .ipa") }
        return (appName, written)
    }
}
