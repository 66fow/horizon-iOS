import SwiftUI
import UIKit

struct SettingsView: View {
    @EnvironmentObject var store: Store
    @State private var checking = false
    @State private var check: (ok: Bool, text: String)?
    @State private var importingCertificate = false
    @State private var certificateFile: Data?
    @State private var certificateProblem: String?

    var body: some View {
        ScrollViewReader { reader in
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                PageHeader(title: "Settings", caption: "Your fork, and the token that lets this app start builds.")

                Color.clear.frame(height: 0).id("repo")
                Plate("Your fork") {
                    FieldBox {
                        TextField("", text: $store.repo, prompt: Text("owner/repository").foregroundColor(Theme.ink(0.4)))
                            .font(.mono(15))
                            .foregroundStyle(Theme.ink(1))
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                    }
                    Text("The GitHub repository that builds Horizon for iOS.")
                        .font(.geist(12))
                        .foregroundStyle(Theme.ink(0.55))
                }

                Color.clear.frame(height: 0).id("token")
                Plate("GitHub token") {
                    FieldBox {
                        SecureField("", text: $store.token, prompt: Text("github_pat_…").foregroundColor(Theme.ink(0.4)))
                            .font(.mono(15))
                            .foregroundStyle(Theme.ink(1))
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                    }
                    HStack(spacing: 8) {
                        Button {
                            if let text = UIPasteboard.general.string {
                                store.token = text.trimmingCharacters(in: .whitespacesAndNewlines)
                                check = nil
                            }
                        } label: {
                            HStack(spacing: 6) {
                                HIcon(name: "copy-01", size: 16)
                                Text("Paste")
                            }
                        }
                        .buttonStyle(QuietButton())
                        Button {
                            Task {
                                checking = true
                                let result = await store.checkToken()
                                check = (result.0, result.1)
                                checking = false
                            }
                        } label: {
                            HStack(spacing: 6) {
                                if checking {
                                    ProgressView().tint(Theme.ink(0.8)).scaleEffect(0.8)
                                } else {
                                    HIcon(name: "tick-02", size: 16)
                                }
                                Text("Check")
                            }
                        }
                        .buttonStyle(QuietButton())
                        .disabled(!store.hasToken || checking)
                        .opacity(!store.hasToken ? 0.4 : 1)
                        Spacer()
                        if store.hasToken {
                            Button("Remove") {
                                store.token = ""
                                check = nil
                            }
                            .buttonStyle(QuietButton(tint: Theme.danger))
                        }
                    }
                    if let check {
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Rectangle().fill(check.ok ? Theme.live : Theme.warn).frame(width: 6, height: 6)
                            Text(check.text).font(.geist(13)).foregroundStyle(Theme.ink(0.8))
                        }
                    }
                    Text("Kept in this iPhone's keychain, and only ever sent to GitHub.")
                        .font(.geist(12))
                        .foregroundStyle(Theme.ink(0.55))
                }

                Color.clear.frame(height: 0).id("certificate")
                CertificatePlate(importing: $importingCertificate, picked: $certificateFile, problem: certificateProblem)

                AIAccounts()

                Color.clear.frame(height: 0).id("maketoken")
                Plate("Make a token") {
                    StepRow(number: 1, text: "Open GitHub's token page below and sign in.")
                    StepRow(number: 2, text: "Name it “Horizon Mods”. Under Repository access choose Only select repositories, then \(store.repo.isEmpty ? "your fork" : store.repo).")
                    StepRow(number: 3, text: "Under Permissions, Repository permissions, set Actions, Contents and Issues to Read and write. (To send mods to Horizon's registry, give it All repositories and Administration: Read and write too.)")
                    StepRow(number: 4, text: "Tap Generate token, copy it, come back here and tap Paste.")
                    Link(destination: URL(string: "https://github.com/settings/personal-access-tokens/new")!) {
                        HStack(spacing: 8) {
                            HIcon(name: "github", size: 18)
                            Text("Open GitHub's token page")
                        }
                        .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(AccentButton(height: 46))
                }

                Plate("About") {
                    Text("Native mods change Horizon's own code and run with full access to the app. Only build mods you trust.")
                        .font(.geist(14))
                        .foregroundStyle(Theme.ink(0.75))
                    HStack {
                        if let url = URL(string: "https://github.com/\(store.repo)/releases") {
                            Link(destination: url) {
                                HStack(spacing: 6) {
                                    HIcon(name: "link-01", size: 16)
                                    Text("All builds on GitHub")
                                }
                            }
                            .buttonStyle(QuietButton())
                        }
                        Spacer()
                        Text("Version " + (Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?"))
                            .font(.mono(12))
                            .foregroundStyle(Theme.ink(0.5))
                    }
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 16)
        }
        .onChange(of: store.demoScroll) { target in
            guard let target else { return }
            withAnimation(.easeInOut(duration: 1.2)) { reader.scrollTo(target, anchor: .top) }
        }
        }
        .scrollDismissesKeyboard(.interactively)
        // Any file: a .p12 from another app or iCloud doesn't always carry the PKCS #12 type.
        // UIKit's own picker, copying the file in: SwiftUI's picker left a .p12 untappable.
        .sheet(isPresented: $importingCertificate) {
            DocumentPicker { url in
                importingCertificate = false
                guard let url else { return }
                certificateFile = Certificate.read(url)
                certificateProblem = certificateFile == nil
                    ? "Couldn't read \(url.lastPathComponent). Put it in On My iPhone > Horizon Mods instead."
                    : nil
            }
            .ignoresSafeArea()
        }
        .onAppear(perform: takePendingFile)
        .onReceive(NotificationCenter.default.publisher(for: .certificateFile)) { _ in takePendingFile() }
        .onReceive(NotificationCenter.default.publisher(for: .certificateImported)) { _ in certificateFile = nil }
    }

    private func takePendingFile() {
        if let data = Certificate.pendingFile {
            certificateFile = data
            Certificate.pendingFile = nil
        }
    }
}

struct StepRow: View {
    let number: Int
    let text: String

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text("\(number)")
                .font(.geist(12, .semibold))
                .foregroundStyle(Color.white)
                .frame(width: 22, height: 22)
                .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(Theme.accent))
            Text(text)
                .font(.geist(14))
                .foregroundStyle(Theme.ink(0.82))
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}
