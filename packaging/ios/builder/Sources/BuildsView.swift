import SwiftUI
import UIKit

struct BuildsView: View {
    @EnvironmentObject var store: Store

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                PageHeader(title: "Builds", caption: "GitHub compiles Horizon with your mods, then it's here to install.")
                if let error = store.buildsError {
                    ErrorPlate(message: error) { Task { await store.refreshRuns() } }
                }
                if store.runs.isEmpty, store.buildsError == nil {
                    EmptyBuilds()
                }
                LazyVStack(spacing: 10) {
                    ForEach(store.runs) { run in
                        BuildCard(run: run, detail: store.details[run.id] ?? RunDetail())
                    }
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 16)
        }
        .refreshable { await store.refreshRuns() }
    }
}

struct EmptyBuilds: View {
    @EnvironmentObject var store: Store

    var body: some View {
        Plate {
            VStack(spacing: 10) {
                HIcon(name: "package-01", size: 34).foregroundStyle(Theme.ink(0.5))
                Text("No builds yet").font(.heading(20)).foregroundStyle(Theme.ink(1))
                Text("Pick mods and tap Build. Finished builds show up here, ready to install.")
                    .font(.geist(14))
                    .foregroundStyle(Theme.ink(0.65))
                    .multilineTextAlignment(.center)
                Button("Pick mods") { store.tab = .mods }
                    .buttonStyle(QuietButton())
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 12)
        }
    }
}

struct BuildCard: View {
    @EnvironmentObject var store: Store
    let run: Run
    let detail: RunDetail
    @State private var confirmStop = false

    private var chip: (String, Color) {
        switch run.state {
        case .running: return ("Building", Theme.accentSoft)
        case .success: return ("Ready", Theme.live)
        case .failed: return ("Failed", Theme.danger)
        case .cancelled: return ("Stopped", Theme.ink(0.55))
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .center) {
                Text("Build \(run.runNumber)")
                    .font(.heading(22))
                    .foregroundStyle(Theme.ink(1))
                Spacer()
                StateChip(label: chip.0, color: chip.1)
            }
            VStack(alignment: .leading, spacing: 4) {
                if let target = store.instance(of: run) {
                    Text(target.name)
                        .font(.geist(13, .semibold))
                        .foregroundStyle(Theme.accentSoft)
                }
                if let mods = detail.mods, !mods.isEmpty {
                    Text(store.names(mods))
                        .font(.geist(14, .medium))
                        .foregroundStyle(Theme.ink(0.85))
                }
                (Text("Started ") + Text(run.createdAt, style: .relative) + Text(" ago"))
                    .font(.geist(12))
                    .foregroundStyle(Theme.ink(0.5))
            }
            switch run.state {
            case .running:
                ProgressBar(value: detail.progress)
                HStack(alignment: .firstTextBaseline) {
                    Text(detail.step)
                        .font(.geist(12))
                        .foregroundStyle(Theme.ink(0.65))
                        .lineLimit(2)
                    Spacer()
                    Text("\(Int(detail.progress * 100))%")
                        .font(.mono(12))
                        .foregroundStyle(Theme.ink(0.65))
                }
                Button { confirmStop = true } label: {
                    HStack(spacing: 6) {
                        HIcon(name: "stop", size: 16)
                        Text("Stop build")
                    }
                }
                .buttonStyle(QuietButton(tint: Theme.danger))
            case .success:
                if let asset = detail.asset {
                    InstallButtons(asset: asset)
                } else {
                    Text("Finding the download…")
                        .font(.geist(13))
                        .foregroundStyle(Theme.ink(0.6))
                }
            case .failed:
                Text(detail.failure ?? "This build didn't finish.")
                    .font(.geist(14))
                    .foregroundStyle(Theme.ink(0.8))
                if let url = URL(string: run.htmlUrl) {
                    Link(destination: url) {
                        HStack(spacing: 6) {
                            HIcon(name: "link-01", size: 16)
                            Text("Open the log on GitHub")
                        }
                    }
                    .buttonStyle(QuietButton())
                }
            case .cancelled:
                EmptyView()
            }
        }
        .padding(16)
        .glass()
        .confirmationDialog("Stop build \(run.runNumber)?", isPresented: $confirmStop, titleVisibility: .visible) {
            Button("Stop build", role: .destructive) { Task { await store.cancel(run) } }
            Button("Keep building", role: .cancel) {}
        }
    }
}

struct InstallButtons: View {
    let asset: Asset
    @State private var copied = false

    private var url: URL? { URL(string: asset.browserDownloadUrl) }

    /// A sideloading app's "install from this link" address.
    private func install(_ scheme: String) -> URL? {
        guard let encoded = asset.browserDownloadUrl.addingPercentEncoding(withAllowedCharacters: .alphanumerics) else {
            return nil
        }
        return URL(string: "\(scheme)://install?url=\(encoded)")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Menu {
                    if let link = install("sidestore") {
                        Link(destination: link) { Label("Open in SideStore", systemImage: "arrow.down.app") }
                    }
                    if let link = install("altstore") {
                        Link(destination: link) { Label("Open in AltStore", systemImage: "arrow.down.app") }
                    }
                    if let url {
                        Link(destination: url) { Label("Download in Safari", systemImage: "safari") }
                    }
                } label: {
                    HStack(spacing: 8) {
                        HIcon(name: "download-04", size: 18)
                        Text("Install")
                    }
                    .font(.geist(15, .semibold))
                    .foregroundStyle(Color.white)
                    .frame(maxWidth: .infinity)
                    .frame(height: 46)
                    .background(RoundedRectangle(cornerRadius: Theme.Radius.large, style: .continuous).fill(Theme.accent))
                }
                if let url {
                    ShareLink(item: url) {
                        HIcon(name: "upload-04", size: 18).frame(width: 46, height: 46)
                    }
                    .foregroundStyle(Theme.ink(0.9))
                    .background(RoundedRectangle(cornerRadius: Theme.Radius.large, style: .continuous).fill(Theme.fill(0.08)))
                }
                Button {
                    UIPasteboard.general.string = asset.browserDownloadUrl
                    copied = true
                } label: {
                    HIcon(name: copied ? "tick-02" : "copy-01", size: 18).frame(width: 46, height: 46)
                }
                .buttonStyle(.plain)
                .foregroundStyle(copied ? Theme.live : Theme.ink(0.9))
                .background(RoundedRectangle(cornerRadius: Theme.Radius.large, style: .continuous).fill(Theme.fill(0.08)))
            }
            Text("\(asset.name) · \(ByteCountFormatter.string(fromByteCount: Int64(asset.size), countStyle: .file)). "
                 + (asset.name.range(of: #"-i[0-9]+\.ipa$"#, options: .regularExpression) != nil
                    ? "It installs as its own app, beside Horizon."
                    : "Install it over Horizon: your worlds and game files stay."))
                .font(.geist(12))
                .foregroundStyle(Theme.ink(0.55))
        }
    }
}
