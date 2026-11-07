import SwiftUI
import UIKit

struct SettingsView: View {
    @EnvironmentObject var store: Store
    @State private var checking = false
    @State private var check: (ok: Bool, text: String)?

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("owner/repository", text: $store.repo)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .font(.body.monospaced())
                } header: {
                    Text("Your fork")
                } footer: {
                    Text("The GitHub repository that builds Horizon for iOS.")
                }

                Section {
                    SecureField("github_pat_…", text: $store.token)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    Button {
                        if let text = UIPasteboard.general.string {
                            store.token = text.trimmingCharacters(in: .whitespacesAndNewlines)
                            check = nil
                        }
                    } label: {
                        Label("Paste token", systemImage: "doc.on.clipboard")
                    }
                    Button {
                        Task {
                            checking = true
                            let result = await store.checkToken()
                            check = (result.0, result.1)
                            checking = false
                        }
                    } label: {
                        HStack {
                            Label("Check token", systemImage: "checkmark.shield")
                            Spacer()
                            if checking { ProgressView() }
                        }
                    }
                    .disabled(!store.hasToken || checking)
                    if let check {
                        Label(check.text, systemImage: check.ok ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                            .font(.subheadline)
                            .foregroundStyle(check.ok ? Color.green : Color.orange)
                    }
                    if store.hasToken {
                        Button("Remove token", role: .destructive) {
                            store.token = ""
                            check = nil
                        }
                    }
                } header: {
                    Text("GitHub token")
                } footer: {
                    Text("Kept in this iPhone's keychain, and only ever sent to GitHub.")
                }

                Section {
                    StepRow(number: 1, text: "Open GitHub's token page below and sign in.")
                    StepRow(number: 2, text: "Name it “Horizon Mods”. Under Repository access choose Only select repositories, then \(store.repo.isEmpty ? "your fork" : store.repo).")
                    StepRow(number: 3, text: "Under Permissions, Repository permissions, set Actions to Read and write.")
                    StepRow(number: 4, text: "Tap Generate token, copy it, come back here and tap Paste token.")
                    Link(destination: URL(string: "https://github.com/settings/personal-access-tokens/new")!) {
                        Label("Open GitHub's token page", systemImage: "safari")
                    }
                } header: {
                    Text("Make a token")
                }

                Section {
                    Text("Native mods change Horizon's own code and run with full access to the app. Only build mods you trust.")
                        .font(.subheadline)
                    if let url = URL(string: "https://github.com/\(store.repo)/releases") {
                        Link("All builds on GitHub", destination: url)
                    }
                    LabeledContent("Version",
                                   value: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?")
                } header: {
                    Text("About")
                }
            }
            .navigationTitle("Settings")
        }
    }
}

struct StepRow: View {
    let number: Int
    let text: String

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text("\(number)")
                .font(.caption.bold())
                .frame(width: 22, height: 22)
                .foregroundStyle(.white)
                .background(Color.horizon, in: Circle())
            Text(text).font(.subheadline)
        }
    }
}
