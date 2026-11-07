import Foundation
import Security

struct GitHubError: LocalizedError {
    let code: Int
    let message: String
    let repo: String

    var errorDescription: String? {
        switch code {
        case 0:
            return message
        case 401:
            return "GitHub didn't accept the token. Paste a new one in Settings."
        case 403 where message.localizedCaseInsensitiveContains("rate limit"):
            return "GitHub's limit for requests without a token was reached. Add a token in Settings."
        case 403:
            return "The token can't do that on \(repo): in Settings, check it has Actions, Contents and Issues set to Read and write."
        case 404:
            return "\(repo) wasn't found. Check the repository in Settings, and that the token can see it."
        case 422:
            return "GitHub refused: \(message)"
        default:
            return "GitHub error \(code)" + (message.isEmpty ? "." : ": \(message)")
        }
    }
}

/// The few GitHub REST calls the app makes, all on one repository.
struct GitHub {
    let repo: String
    let token: String

    static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.keyDecodingStrategy = .convertFromSnakeCase
        d.dateDecodingStrategy = .iso8601
        return d
    }()

    private struct Message: Decodable { let message: String? }

    @discardableResult
    func data(_ path: String, method: String = "GET", json: [String: Any]? = nil) async throws -> Data {
        guard !repo.isEmpty else {
            throw GitHubError(code: 0, message: "Set the repository in Settings.", repo: repo)
        }
        return try await call("https://api.github.com/repos/\(repo)\(path)", method: method, json: json)
    }

    /// Any GitHub address (`api.github.com`, `uploads.github.com`), with the token.
    @discardableResult
    func call(_ address: String, method: String = "GET", json: [String: Any]? = nil,
              body: Data? = nil, contentType: String? = nil) async throws -> Data {
        guard let url = URL(string: address) else {
            throw GitHubError(code: 0, message: "Bad address \(address)", repo: repo)
        }
        var request = URLRequest(url: url)
        if let body {
            request.httpBody = body
            request.setValue(contentType ?? "application/octet-stream", forHTTPHeaderField: "Content-Type")
        }
        request.httpMethod = method
        request.cachePolicy = .reloadIgnoringLocalCacheData
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.setValue("2022-11-28", forHTTPHeaderField: "X-GitHub-Api-Version")
        if !token.isEmpty { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        if let json {
            request.httpBody = try JSONSerialization.data(withJSONObject: json)
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        let (data, response) = try await URLSession.shared.data(for: request)
        let code = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(code) else {
            let message = (try? JSONDecoder().decode(Message.self, from: data))?.message ?? ""
            throw GitHubError(code: code, message: message, repo: repo)
        }
        return data
    }

    func get<T: Decodable>(_ path: String) async throws -> T {
        try Self.decoder.decode(T.self, from: try await data(path))
    }

    /// `data`'s JSON object.
    func object(_ path: String, method: String = "GET", json: [String: Any]? = nil) async throws -> [String: Any] {
        let data = try await data(path, method: method, json: json)
        return (try JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    }
}

/// The GitHub token, in the keychain (or, if the signer gave the app no keychain, a protected
/// file in its Library, which the Files app can't see).
enum TokenStore {
    private static let base: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: "dev.horizon.mods.github",
        kSecAttrAccount as String: "token",
    ]

    private static var file: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("github-token")
    }

    static func load() -> String {
        var query = base
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        if SecItemCopyMatching(query as CFDictionary, &out) == errSecSuccess,
           let data = out as? Data, let token = String(data: data, encoding: .utf8) {
            return token
        }
        return (try? String(contentsOf: file, encoding: .utf8)) ?? ""
    }

    static func save(_ token: String) {
        SecItemDelete(base as CFDictionary)
        try? FileManager.default.removeItem(at: file)
        guard !token.isEmpty else { return }
        var add = base
        add[kSecValueData as String] = Data(token.utf8)
        add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        if SecItemAdd(add as CFDictionary, nil) != errSecSuccess {
            try? FileManager.default.createDirectory(
                at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? Data(token.utf8).write(to: file, options: [.atomic, .completeFileProtection])
        }
    }
}
