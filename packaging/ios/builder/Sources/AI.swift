import AuthenticationServices
import CryptoKit
import Foundation
import Network
import Security
import UIKit

/// The AIs the mod maker can write with. ChatGPT uses the player's plan through OpenAI's
/// "Sign in with ChatGPT"; the others use the player's own API key. Claude and Gemini
/// subscriptions are not offered: Anthropic's terms forbid using a Claude.ai login in other
/// apps, and Google has no allowed way for apps to use a Gemini subscription.
enum Provider: String, CaseIterable, Identifiable, Codable {
    case chatgpt, claude, openai, gemini
    var id: String { rawValue }

    var title: String {
        switch self {
        case .chatgpt: return "ChatGPT (your plan)"
        case .claude: return "Claude (API key)"
        case .openai: return "OpenAI (API key)"
        case .gemini: return "Gemini (API key)"
        }
    }

    var short: String {
        switch self {
        case .chatgpt: return "ChatGPT"
        case .claude: return "Claude"
        case .openai: return "OpenAI"
        case .gemini: return "Gemini"
        }
    }

    /// Where the player gets a key.
    var keyPage: URL? {
        switch self {
        case .chatgpt: return nil
        case .claude: return URL(string: "https://console.anthropic.com/settings/keys")
        case .openai: return URL(string: "https://platform.openai.com/api-keys")
        case .gemini: return URL(string: "https://aistudio.google.com/apikey")
        }
    }

    var defaultModel: String {
        switch self {
        // OpenAI's and Google's names change often: theirs are picked from the account's list.
        case .chatgpt, .openai, .gemini: return ""
        case .claude: return "claude-sonnet-5-5"
        }
    }
}

struct AIError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

/// Keychain items for the AI accounts (service `dev.horizon.mods.ai`).
enum Secrets {
    private static func base(_ account: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: "dev.horizon.mods.ai",
         kSecAttrAccount as String: account]
    }

    private static func file(_ account: String) -> URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("secret-\(account)")
    }

    static func load(_ account: String) -> String {
        var query = base(account)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        if SecItemCopyMatching(query as CFDictionary, &out) == errSecSuccess,
           let data = out as? Data, let text = String(data: data, encoding: .utf8) {
            return text
        }
        return (try? String(contentsOf: file(account), encoding: .utf8)) ?? ""
    }

    static func save(_ account: String, _ value: String) {
        SecItemDelete(base(account) as CFDictionary)
        try? FileManager.default.removeItem(at: file(account))
        guard !value.isEmpty else { return }
        var add = base(account)
        add[kSecValueData as String] = Data(value.utf8)
        add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        if SecItemAdd(add as CFDictionary, nil) != errSecSuccess {
            let url = file(account)
            try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? Data(value.utf8).write(to: url, options: [.atomic, .completeFileProtection])
        }
    }
}

/// One turn of a conversation with the AI.
struct ChatTurn: Codable, Identifiable, Equatable {
    var id = UUID()
    let role: Role
    var text: String

    enum Role: String, Codable { case user, assistant }
}

// MARK: Sign in with ChatGPT

/// A ChatGPT sign-in: OpenAI's "Sign in with ChatGPT" for open-source tools (OAuth 2 with PKCE,
/// the client registered on first sign-in, the code sent to a server on 127.0.0.1:1455 that this
/// app runs while the sign-in page is open).
struct ChatGPTAccount: Codable {
    var clientId: String
    var accessToken: String
    var refreshToken: String
    var expires: Date
    var email: String
}

final class ChatGPTSignIn: NSObject, ASWebAuthenticationPresentationContextProviding {
    static let authorize = "https://auth.openai.com/api/accounts/authorize"
    static let token = "https://auth.openai.com/api/accounts/oauth/token"
    static let redirect = "http://127.0.0.1:1455/auth/callback"
    static let resource = "https://api.openai.com/v1"
    static let scope = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct"

    private var session: ASWebAuthenticationSession?
    private var anchor = ASPresentationAnchor()

    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor { anchor }

    /// The machine's id for OpenAI's consent page: made once, kept.
    private static var hostId: String {
        let key = "chatgptHostId"
        if let id = UserDefaults.standard.string(forKey: key) { return id }
        let id = "urn:uuid:" + UUID().uuidString.lowercased()
        UserDefaults.standard.set(id, forKey: key)
        return id
    }

    private static func random() -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return Data(bytes).base64URL
    }

    @MainActor
    func signIn(previous: ChatGPTAccount?) async throws -> ChatGPTAccount {
        anchor = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows).first { $0.isKeyWindow } ?? ASPresentationAnchor()
        let verifier = Self.random()
        let challenge = Data(SHA256.hash(data: Data(verifier.utf8))).base64URL
        let state = Self.random()
        let nonce = Self.random()
        var items = [
            URLQueryItem(name: "client_id", value: previous?.clientId ?? "dynamic_agent_client"),
            URLQueryItem(name: "ext_agent_host_id", value: Self.hostId),
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "redirect_uri", value: Self.redirect),
            URLQueryItem(name: "scope", value: Self.scope),
            URLQueryItem(name: "resource", value: Self.resource),
            URLQueryItem(name: "state", value: state),
            URLQueryItem(name: "nonce", value: nonce),
            URLQueryItem(name: "code_challenge", value: challenge),
            URLQueryItem(name: "code_challenge_method", value: "S256"),
        ]
        if previous == nil { items.append(URLQueryItem(name: "agent_name_hint", value: "Horizon Mods")) }
        var url = URLComponents(string: Self.authorize)!
        url.queryItems = items

        let server = try LoopbackServer(port: 1455)
        let page = url.url!
        let callback: [String: String] = try await withThrowingTaskGroup(of: [String: String]?.self) { group in
            group.addTask { try await server.nextCallback() }
            group.addTask { @MainActor in
                try await self.present(page)
                return nil
            }
            do {
                while let result = try await group.next() {
                    if let query = result {
                        // The page never calls back itself: close it, which ends its task.
                        await MainActor.run { self.session?.cancel() }
                        server.stop()
                        return query
                    }
                }
            } catch {
                // Closed by the player: the server's task ends too.
                server.stop()
                throw error
            }
            server.stop()
            throw AIError(message: "Sign-in was closed before it finished.")
        }
        session = nil
        if let error = callback["error"] {
            throw AIError(message: "ChatGPT sign-in: \(callback["error_description"] ?? error)")
        }
        guard callback["state"] == state, let code = callback["code"] else {
            throw AIError(message: "The sign-in answer didn't match: try again.")
        }
        let clientId = callback["client_id"] ?? previous?.clientId ?? ""
        guard !clientId.isEmpty, clientId != "dynamic_agent_client" else {
            throw AIError(message: "OpenAI didn't register Horizon Mods: try again.")
        }
        let tokens = try await Self.tokenRequest([
            "grant_type": "authorization_code", "client_id": clientId, "code": code,
            "code_verifier": verifier, "redirect_uri": Self.redirect, "resource": Self.resource,
        ])
        guard tokens.scope?.contains("chatgpt.tokens.use.direct") ?? false else {
            throw AIError(message: "“Use your ChatGPT plan” wasn't allowed on the sign-in page, so Horizon Mods can't write with it.")
        }
        let claims = tokens.idToken.flatMap(JWT.claims) ?? [:]
        if let sent = claims["nonce"] as? String, sent != nonce {
            throw AIError(message: "The sign-in answer didn't match: try again.")
        }
        return ChatGPTAccount(clientId: clientId, accessToken: tokens.accessToken,
                              refreshToken: tokens.refreshToken ?? previous?.refreshToken ?? "",
                              expires: Date().addingTimeInterval(Double(tokens.expiresIn ?? 3600)),
                              email: claims["email"] as? String ?? previous?.email ?? "")
    }

    private func present(_ url: URL) async throws {
        try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, Error>) in
            let session = ASWebAuthenticationSession(url: url, callbackURLScheme: "horizonmods") { _, error in
                // The code arrives at the loopback server; the page itself never calls back.
                done.resume(throwing: error ?? AIError(message: "Sign-in was closed."))
            }
            session.presentationContextProvider = self
            session.prefersEphemeralWebBrowserSession = false
            self.session = session
            if !session.start() {
                done.resume(throwing: AIError(message: "The sign-in page didn't open."))
            }
        }
    }

    struct Tokens: Decodable {
        let accessToken: String
        let refreshToken: String?
        let idToken: String?
        let scope: String?
        let expiresIn: Int?
    }

    static func tokenRequest(_ form: [String: String]) async throws -> Tokens {
        var request = URLRequest(url: URL(string: token)!)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        var allowed = CharacterSet.alphanumerics
        allowed.insert(charactersIn: "-._~")
        request.httpBody = form.map { key, value in
            "\(key)=\(value.addingPercentEncoding(withAllowedCharacters: allowed) ?? "")"
        }.joined(separator: "&").data(using: .utf8)
        let (data, response) = try await URLSession.shared.data(for: request)
        guard (response as? HTTPURLResponse)?.statusCode == 200 else {
            throw AIError(message: "ChatGPT sign-in failed: \(String(data: data, encoding: .utf8)?.prefix(300) ?? "")")
        }
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(Tokens.self, from: data)
    }

    /// `account` with an access token good for at least another minute.
    static func fresh(_ account: ChatGPTAccount) async throws -> ChatGPTAccount {
        if account.expires > Date().addingTimeInterval(60) { return account }
        let tokens = try await tokenRequest([
            "grant_type": "refresh_token", "client_id": account.clientId,
            "refresh_token": account.refreshToken, "resource": resource,
        ])
        var next = account
        next.accessToken = tokens.accessToken
        next.refreshToken = tokens.refreshToken ?? account.refreshToken
        next.expires = Date().addingTimeInterval(Double(tokens.expiresIn ?? 3600))
        return next
    }
}

/// A one-request HTTP server on 127.0.0.1, for the sign-in's redirect.
final class LoopbackServer: @unchecked Sendable {
    private let listener: NWListener
    private let queue = DispatchQueue(label: "dev.horizon.mods.loopback")
    private var waiting: CheckedContinuation<[String: String]?, Error>?
    private var arrived: [String: String]?

    init(port: UInt16) throws {
        let parameters = NWParameters.tcp
        parameters.requiredInterfaceType = .loopback
        parameters.allowLocalEndpointReuse = true
        listener = try NWListener(using: parameters, on: NWEndpoint.Port(rawValue: port)!)
        listener.newConnectionHandler = { [weak self] connection in self?.serve(connection) }
        listener.start(queue: queue)
    }

    /// Stops listening; a wait still open ends with nothing.
    func stop() {
        queue.async {
            self.listener.cancel()
            self.waiting?.resume(returning: nil)
            self.waiting = nil
        }
    }

    func nextCallback() async throws -> [String: String]? {
        try await withCheckedThrowingContinuation { done in
            queue.async {
                if let arrived = self.arrived {
                    done.resume(returning: arrived)
                } else {
                    self.waiting = done
                }
            }
        }
    }

    private func serve(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 16_384) { [weak self] data, _, _, _ in
            guard let self else { return }
            let head = data.flatMap { String(data: $0, encoding: .utf8) }?.components(separatedBy: "\r\n").first ?? ""
            let parts = head.split(separator: " ")
            var query: [String: String]?
            if parts.count >= 2, parts[0] == "GET", parts[1].hasPrefix("/auth/callback"),
               let comps = URLComponents(string: "http://127.0.0.1" + parts[1]) {
                query = Dictionary((comps.queryItems ?? []).map { ($0.name, $0.value ?? "") }, uniquingKeysWith: { a, _ in a })
            }
            let page = query == nil
                ? "Not found"
                : "<html><body style=\"font:17px -apple-system;background:#0a0919;color:#fff;text-align:center;padding-top:40vh\">Signed in. Go back to Horizon Mods.</body></html>"
            let reply = "HTTP/1.1 \(query == nil ? "404 Not Found" : "200 OK")\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: \(page.utf8.count)\r\nConnection: close\r\n\r\n\(page)"
            connection.send(content: Data(reply.utf8), completion: .contentProcessed { _ in connection.cancel() })
            if let query {
                if let waiting = self.waiting {
                    self.waiting = nil
                    waiting.resume(returning: query)
                } else {
                    self.arrived = query
                }
            }
        }
    }
}

enum JWT {
    /// A JWT's claims, unverified: the token came straight from auth.openai.com over TLS.
    static func claims(_ token: String) -> [String: Any]? {
        let parts = token.split(separator: ".")
        guard parts.count >= 2 else { return nil }
        var text = String(parts[1]).replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        while text.count % 4 != 0 { text += "=" }
        guard let data = Data(base64Encoded: text) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }
}

extension Data {
    var base64URL: String {
        base64EncodedString().replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    }
}

// MARK: Talking to the AI

/// One request to the chosen AI: `instructions` and the conversation, the answer's text streamed.
struct AIClient {
    let provider: Provider
    let model: String
    let key: String
    let chatgpt: ChatGPTAccount?

    func send(instructions: String, turns: [ChatTurn], onText: @escaping (String) -> Void) async throws -> String {
        let request: URLRequest
        switch provider {
        case .claude:
            request = try json("https://api.anthropic.com/v1/messages", headers: [
                "x-api-key": key, "anthropic-version": "2023-06-01",
            ], body: [
                "model": model, "max_tokens": 32000, "stream": true, "system": instructions,
                "messages": turns.map { ["role": $0.role.rawValue, "content": $0.text] },
            ])
        case .openai, .chatgpt:
            let token = provider == .chatgpt ? (chatgpt?.accessToken ?? "") : key
            request = try json("https://api.openai.com/v1/responses", headers: [
                "Authorization": "Bearer \(token)",
            ], body: [
                "model": model, "instructions": instructions, "store": false, "stream": true,
                "input": turns.map { ["role": $0.role.rawValue, "content": $0.text] },
            ])
        case .gemini:
            let name = model.hasPrefix("models/") ? String(model.dropFirst(7)) : model
            request = try json("https://generativelanguage.googleapis.com/v1beta/models/\(name):streamGenerateContent?alt=sse",
                               headers: ["x-goog-api-key": key], body: [
                "systemInstruction": ["parts": [["text": instructions]]],
                "contents": turns.map { ["role": $0.role == .user ? "user" : "model", "parts": [["text": $0.text]]] },
            ])
        }
        let (bytes, response) = try await URLSession.shared.bytes(for: request)
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard status == 200 else {
            var body = ""
            for try await line in bytes.lines {
                body += line
                if body.count > 2000 { break }
            }
            throw AIError(message: Self.describe(provider, status, body))
        }
        var text = ""
        for try await line in bytes.lines {
            guard line.hasPrefix("data:") else { continue }
            let payload = line.dropFirst(5).trimmingCharacters(in: .whitespaces)
            guard payload != "[DONE]", let data = payload.data(using: .utf8),
                  let event = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { continue }
            if let failure = Self.failure(provider, event) { throw AIError(message: failure) }
            if let delta = Self.delta(provider, event), !delta.isEmpty {
                text += delta
                onText(text)
            }
        }
        if text.isEmpty { throw AIError(message: "\(provider.short) sent back nothing.") }
        return text
    }

    private func json(_ url: String, headers: [String: String], body: [String: Any]) throws -> URLRequest {
        var request = URLRequest(url: URL(string: url)!)
        request.httpMethod = "POST"
        request.timeoutInterval = 600
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        for (k, v) in headers { request.setValue(v, forHTTPHeaderField: k) }
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        return request
    }

    private static func delta(_ provider: Provider, _ event: [String: Any]) -> String? {
        switch provider {
        case .claude:
            guard event["type"] as? String == "content_block_delta",
                  let delta = event["delta"] as? [String: Any] else { return nil }
            return delta["text"] as? String
        case .openai, .chatgpt:
            guard event["type"] as? String == "response.output_text.delta" else { return nil }
            return event["delta"] as? String
        case .gemini:
            let candidates = event["candidates"] as? [[String: Any]]
            let parts = (candidates?.first?["content"] as? [String: Any])?["parts"] as? [[String: Any]]
            return parts?.compactMap { $0["text"] as? String }.joined()
        }
    }

    private static func failure(_ provider: Provider, _ event: [String: Any]) -> String? {
        if event["type"] as? String == "response.failed" || event["type"] as? String == "error" {
            let error = (event["response"] as? [String: Any])?["error"] as? [String: Any] ?? event["error"] as? [String: Any]
            let code = error?["code"] as? String ?? ""
            if code.contains("usage_limit") { return "You've reached your ChatGPT plan's limit for now. Try again later." }
            return "\(provider.short) stopped: \(error?["message"] as? String ?? code)"
        }
        return nil
    }

    static func describe(_ provider: Provider, _ status: Int, _ body: String) -> String {
        let message = ((try? JSONSerialization.jsonObject(with: Data(body.utf8))) as? [String: Any])
            .flatMap { ($0["error"] as? [String: Any])?["message"] as? String } ?? String(body.prefix(300))
        switch (provider, status) {
        case (.chatgpt, 403) where body.contains("not_eligible"):
            return "This ChatGPT account can't lend its plan to apps (it needs Go, Plus or Pro)."
        case (.chatgpt, 429):
            return "You've reached your ChatGPT plan's limit for now. Try again later."
        case (_, 401), (_, 403):
            return "\(provider.short) didn't accept the \(provider == .chatgpt ? "sign-in: sign in again" : "API key"). \(message)"
        case (_, 404):
            return "\(provider.short) doesn't have that model: pick another in Settings. \(message)"
        case (_, 429):
            return "\(provider.short) says slow down (or the key's credit ran out). \(message)"
        default:
            return "\(provider.short) error \(status): \(message)"
        }
    }

    /// The models this account can use, for the picker.
    func models() async throws -> [String] {
        var request: URLRequest
        switch provider {
        case .claude:
            request = URLRequest(url: URL(string: "https://api.anthropic.com/v1/models?limit=100")!)
            request.setValue(key, forHTTPHeaderField: "x-api-key")
            request.setValue("2023-06-01", forHTTPHeaderField: "anthropic-version")
        case .openai, .chatgpt:
            request = URLRequest(url: URL(string: "https://api.openai.com/v1/models")!)
            request.setValue("Bearer \(provider == .chatgpt ? chatgpt?.accessToken ?? "" : key)", forHTTPHeaderField: "Authorization")
        case .gemini:
            request = URLRequest(url: URL(string: "https://generativelanguage.googleapis.com/v1beta/models?pageSize=200")!)
            request.setValue(key, forHTTPHeaderField: "x-goog-api-key")
        }
        let (data, response) = try await URLSession.shared.data(for: request)
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard status == 200, let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
            throw AIError(message: Self.describe(provider, status, String(data: data, encoding: .utf8) ?? ""))
        }
        switch provider {
        case .gemini:
            return ((object["models"] as? [[String: Any]]) ?? [])
                .filter { (($0["supportedGenerationMethods"] as? [String]) ?? []).contains("generateContent") }
                .compactMap { ($0["name"] as? String).map { $0.replacingOccurrences(of: "models/", with: "") } }
                .filter { $0.hasPrefix("gemini") }
        case .openai, .chatgpt:
            return ((object["data"] as? [[String: Any]]) ?? [])
                .filter { ($0["visibility"] as? String ?? "list") == "list" }
                .compactMap { $0["id"] as? String }
                .filter { $0.hasPrefix("gpt") || $0.hasPrefix("o") }
                .sorted(by: >)
        case .claude:
            return ((object["data"] as? [[String: Any]]) ?? []).compactMap { $0["id"] as? String }
        }
    }
}
