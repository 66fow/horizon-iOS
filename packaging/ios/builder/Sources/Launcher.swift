import CryptoKit
import Darwin
import Foundation
import MachO
import ObjectiveC
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Plays a downloaded Horizon build inside Horizon Mods (packaging/ios/builder/play.md), the way an
/// app launcher like LiveContainer runs apps, without its code: the build's program is made a
/// library and signed with the player's own certificate (the signer, signer/), then Horizon Mods
/// restarts, loads it before its own screens start, and runs its `main`. The build sees Horizon
/// Mods' folders as its own, so every build shares the game files, worlds, mods and packs.
enum Launcher {
    struct Pending: Codable {
        let folder: String
        let app: String
    }

    private static var support: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    }
    private static var pendingFile: URL { support.appendingPathComponent("launch.json") }
    static let library = "HorizonGame.dylib"

    /// Why the last launch didn't start, for the Play tab.
    static var lastError: String? {
        get { UserDefaults.standard.string(forKey: "launchError") }
        set { UserDefaults.standard.set(newValue, forKey: "launchError") }
    }

    // MARK: At start

    /// Called first thing: if a build is waiting to be played, play it (this never returns then).
    /// The wish is used once, so the next start after the game is closed is Horizon Mods again.
    static func runPendingLaunch() {
        guard let data = try? Data(contentsOf: pendingFile),
              let pending = try? JSONDecoder().decode(Pending.self, from: data) else { return }
        try? FileManager.default.removeItem(at: pendingFile)
        let folder = Library.builds.appendingPathComponent(pending.folder)
        let path = folder.appendingPathComponent(library).path
        guard let handle = dlopen(path, RTLD_NOW | RTLD_GLOBAL) else {
            lastError = "The iPhone didn't load the build: " + (dlerror().map { String(cString: $0) } ?? "no reason given")
                + ". Import your certificate again in Settings, then play it again."
            return
        }
        _ = handle
        guard let entry = entryPoint(endingWith: "/\(pending.folder)/\(library)") else {
            lastError = "The build has no starting point."
            return
        }
        lastError = nil
        BundleSwap.use(folder.appendingPathComponent(pending.app).path)
        typealias Main = @convention(c) (Int32, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?) -> Int32
        let main = unsafeBitCast(entry, to: Main.self)
        exit(main(CommandLine.argc, CommandLine.unsafeArgv))
    }

    /// The `LC_MAIN` entry of the loaded image whose path ends with `suffix`.
    private static func entryPoint(endingWith suffix: String) -> UnsafeRawPointer? {
        for i in 0..<_dyld_image_count() {
            guard let name = _dyld_get_image_name(i), String(cString: name).hasSuffix(suffix),
                  let header = _dyld_get_image_header(i) else { continue }
            let base = UnsafeRawPointer(header)
            var command = base + MemoryLayout<mach_header_64>.size
            for _ in 0..<Int(header.pointee.ncmds) {
                let lc = command.load(as: load_command.self)
                if lc.cmd == 0x8000_0028 { // LC_MAIN
                    let main = command.load(as: entry_point_command.self)
                    return base + Int(main.entryoff)
                }
                command += Int(lc.cmdsize)
            }
        }
        return nil
    }

    // MARK: From the Play tab

    enum Problem: LocalizedError {
        case noCertificate, failed(String)
        var errorDescription: String? {
            switch self {
            case .noCertificate: return "Import your signing certificate in Settings first."
            case .failed(let why): return why
            }
        }
    }

    /// Signs `build` for this iPhone if it isn't yet (or was signed with another certificate),
    /// then restarts Horizon Mods into it.
    @MainActor
    static func play(_ build: LocalBuild, library: Library) async throws {
        guard let certificate = Certificate.stored else { throw Problem.noCertificate }
        let folder = Library.builds.appendingPathComponent(build.folder)
        let out = folder.appendingPathComponent(Self.library)
        if build.signedWith != certificate.fingerprint || !FileManager.default.fileExists(atPath: out.path) {
            let app = folder.appendingPathComponent(build.app)
            let info = NSDictionary(contentsOf: app.appendingPathComponent("Info.plist"))
            guard let executable = info?["CFBundleExecutable"] as? String else {
                throw Problem.failed("The build has no program in it.")
            }
            let input = app.appendingPathComponent(executable)
            try await Task.detached(priority: .userInitiated) {
                try Signer.prepare(input: input, output: out, certificate: certificate,
                                   identifier: (info?["CFBundleIdentifier"] as? String) ?? "dev.horizon.game")
            }.value
            library.markSigned(build, with: certificate.fingerprint)
        }
        try JSONEncoder().encode(Pending(folder: build.folder, app: build.app)).write(to: pendingFile, options: .atomic)
        relaunch()
    }

    /// Opens Horizon Mods again and closes this run of it, so the next start plays the build.
    @MainActor
    static func relaunch() {
        if let workspace = NSClassFromString("LSApplicationWorkspace") as? NSObject.Type,
           let shared = workspace.perform(NSSelectorFromString("defaultWorkspace"))?.takeUnretainedValue() as? NSObject,
           let id = Bundle.main.bundleIdentifier {
            _ = shared.perform(NSSelectorFromString("openApplicationWithBundleID:"), with: id)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { exit(0) }
    }
}

/// While a build plays, the main bundle is the build's app (its Info.plist, its name), as it
/// would be installed on its own: UIKit reads how the app starts from it.
enum BundleSwap {
    static var guest: Bundle?

    static func use(_ path: String) {
        guard let bundle = Bundle(path: path),
              let original = class_getClassMethod(Bundle.self, #selector(getter: Bundle.main)),
              let replacement = class_getClassMethod(Bundle.self, #selector(getter: Bundle.horizonGuest)) else { return }
        guest = bundle
        method_exchangeImplementations(original, replacement)
    }
}

extension Bundle {
    @objc class var horizonGuest: Bundle { BundleSwap.guest! }
}

// MARK: The player's certificate

/// The certificate the player's sideloading app signs Horizon Mods with (a .p12 and its password),
/// kept in the keychain. A library is only loaded if it's signed by the same team.
struct Certificate {
    let p12: Data
    let password: String

    var fingerprint: String { SHA256.hash(data: p12).map { String(format: "%02x", $0) }.joined() }

    static var stored: Certificate? {
        guard let data = Data(base64Encoded: Secrets.load("p12")), !data.isEmpty else { return nil }
        return Certificate(p12: data, password: Secrets.load("p12password"))
    }

    static func save(_ cert: Certificate?) {
        Secrets.save("p12", cert?.p12.base64EncodedString() ?? "")
        Secrets.save("p12password", cert?.password ?? "")
    }

    /// Name, team and end date, or why the certificate can't be read.
    func describe() -> Result<(name: String, team: String, until: String), Signer.Failure> {
        Signer.describe(self)
    }

    /// The link that asks SideStore or AltStore to hand Horizon Mods its certificate.
    static func request(from app: String) -> URL? {
        URL(string: "\(app)://certificate?callback_template=horizonmods%3A%2F%2Fcertificate%3Fcert%3D%24%28BASE64_CERT%29%26password%3D%24%28PASSWORD%29")
    }

    /// The certificate in a `horizonmods://certificate?cert=…&password=…` link.
    static func from(_ url: URL) -> Certificate? {
        guard url.scheme == "horizonmods", url.host == "certificate",
              let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems,
              let cert = items.first(where: { $0.name == "cert" })?.value,
              let data = Data(base64Encoded: cert.replacingOccurrences(of: " ", with: "+")) else { return nil }
        return Certificate(p12: data, password: items.first { $0.name == "password" }?.value ?? "")
    }
}

/// The signer (signer/, Rust), linked into the app for iPhones.
enum Signer {
    struct Failure: LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    static func prepare(input: URL, output: URL, certificate: Certificate, identifier: String) throws {
        #if targetEnvironment(simulator)
        throw Failure(message: "Builds play on an iPhone, not in the simulator.")
        #else
        var error = [CChar](repeating: 0, count: 1024)
        let code = certificate.p12.withUnsafeBytes { p12 in
            hm_prepare(input.path, output.path, p12.bindMemory(to: UInt8.self).baseAddress, p12.count,
                       certificate.password, identifier, &error, error.count)
        }
        if code != 0 { throw Failure(message: "Signing the build failed: " + String(cString: error)) }
        #endif
    }

    static func describe(_ certificate: Certificate) -> Result<(name: String, team: String, until: String), Failure> {
        #if targetEnvironment(simulator)
        return .failure(Failure(message: "Certificates are read on an iPhone."))
        #else
        var out = [CChar](repeating: 0, count: 1024)
        let code = certificate.p12.withUnsafeBytes { p12 in
            hm_certificate(p12.bindMemory(to: UInt8.self).baseAddress, p12.count, certificate.password, &out, out.count)
        }
        let text = String(cString: out)
        guard code == 0 else { return .failure(Failure(message: text)) }
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        return .success((lines.first ?? "", lines.count > 1 ? lines[1] : "", lines.count > 2 ? lines[2] : ""))
        #endif
    }
}

#if !targetEnvironment(simulator)
@_silgen_name("hm_prepare")
func hm_prepare(_ input: UnsafePointer<CChar>, _ output: UnsafePointer<CChar>, _ p12: UnsafePointer<UInt8>?, _ p12Len: Int,
                _ password: UnsafePointer<CChar>, _ identifier: UnsafePointer<CChar>,
                _ error: UnsafeMutablePointer<CChar>, _ errorLen: Int) -> Int32

@_silgen_name("hm_certificate")
func hm_certificate(_ p12: UnsafePointer<UInt8>?, _ p12Len: Int, _ password: UnsafePointer<CChar>,
                    _ out: UnsafeMutablePointer<CChar>, _ outLen: Int) -> Int32
#endif

/// The certificate, in Settings.
struct CertificatePlate: View {
    @State private var cert = Certificate.stored
    @State private var importing = false
    @State private var password = ""
    @State private var picked: Data?

    var body: some View {
        Plate("Signing certificate") {
            if let cert {
                switch cert.describe() {
                case .success(let info):
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Rectangle().fill(Theme.live).frame(width: 6, height: 6)
                        Text(info.name.isEmpty ? "Certificate imported" : info.name)
                            .font(.geist(14, .semibold)).foregroundStyle(Theme.ink(1))
                    }
                    Text("Team \(info.team.isEmpty ? "?" : info.team) · until \(String(info.until.prefix(10)))")
                        .font(.mono(11)).foregroundStyle(Theme.ink(0.55))
                case .failure(let error):
                    Text(error.message).font(.geist(13)).foregroundStyle(Theme.warn)
                }
                Button("Remove", role: .destructive) {
                    Certificate.save(nil)
                    self.cert = nil
                }
                .buttonStyle(QuietButton(tint: Theme.danger))
            } else {
                Text("Builds play inside Horizon Mods once they're signed with the certificate your sideloading app signs Horizon Mods with. It works with a free Apple ID.")
                    .font(.geist(13)).foregroundStyle(Theme.ink(0.7))
            }
            HStack(spacing: 8) {
                ForEach(["SideStore", "AltStore"], id: \.self) { app in
                    if let url = Certificate.request(from: app.lowercased()) {
                        Link(destination: url) { Text("From \(app)") }.buttonStyle(QuietButton())
                    }
                }
                Button(".p12 file") { importing = true }.buttonStyle(QuietButton())
            }
            if picked != nil {
                FieldBox {
                    SecureField("", text: $password, prompt: Text("The .p12's password").foregroundColor(Theme.ink(0.4)))
                        .font(.mono(14)).foregroundStyle(Theme.ink(1))
                }
                Button("Use this certificate") {
                    if let picked {
                        let next = Certificate(p12: picked, password: password)
                        Certificate.save(next)
                        cert = next
                    }
                    picked = nil
                    password = ""
                }
                .buttonStyle(AccentButton(height: 40))
            }
            Text("When SideStore or AltStore makes a new certificate (a free Apple ID's can change), import it again: builds are signed again the next time they're played.")
                .font(.geist(12)).foregroundStyle(Theme.ink(0.55))
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.pkcs12, .data]) { result in
            if case .success(let url) = result {
                let open = url.startAccessingSecurityScopedResource()
                defer { if open { url.stopAccessingSecurityScopedResource() } }
                picked = try? Data(contentsOf: url)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .certificateImported)) { _ in cert = Certificate.stored }
    }
}

extension Notification.Name {
    static let certificateImported = Notification.Name("HorizonModsCertificateImported")
}
