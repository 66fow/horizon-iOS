import SwiftUI
import UIKit

struct BuildsView: View {
    @EnvironmentObject var store: Store

    var body: some View {
        NavigationStack {
            List {
                if let error = store.buildsError {
                    Section { ErrorRow(message: error) { Task { await store.refreshRuns() } } }
                }
                if store.runs.isEmpty, store.buildsError == nil {
                    Section { EmptyBuilds() }
                }
                ForEach(store.runs) { run in
                    Section {
                        BuildCard(run: run, detail: store.details[run.id] ?? RunDetail())
                    }
                }
            }
            .navigationTitle("Builds")
            .refreshable { await store.refreshRuns() }
        }
    }
}

struct EmptyBuilds: View {
    @EnvironmentObject var store: Store

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: "hammer")
                .font(.largeTitle)
                .foregroundStyle(.secondary)
            Text("No builds yet").font(.headline)
            Text("Pick mods and tap Build. Finished builds show up here, ready to install.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            Button("Pick mods") { store.tab = .mods }
                .buttonStyle(.bordered)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
    }
}

struct BuildCard: View {
    @EnvironmentObject var store: Store
    let run: Run
    let detail: RunDetail
    @State private var confirmStop = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Build \(run.runNumber)").font(.headline)
                Spacer()
                StatusBadge(state: run.state)
            }
            if let mods = detail.mods, !mods.isEmpty {
                Text(store.names(mods)).font(.subheadline)
            }
            (Text(run.createdAt, style: .relative) + Text(" ago"))
                .font(.caption)
                .foregroundStyle(.secondary)
            switch run.state {
            case .running:
                ProgressView(value: detail.progress)
                Text(detail.step)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                Button("Stop build", role: .destructive) { confirmStop = true }
                    .buttonStyle(.borderless)
                    .font(.subheadline)
            case .success:
                if let asset = detail.asset {
                    InstallButtons(asset: asset)
                } else {
                    Text("Finding the download…")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            case .failed:
                Text(detail.failure ?? "This build didn't finish.").font(.subheadline)
                if let url = URL(string: run.htmlUrl) {
                    Link("Open the log on GitHub", destination: url).font(.subheadline)
                }
            case .cancelled:
                Text("Stopped.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 6)
        .confirmationDialog("Stop build \(run.runNumber)?", isPresented: $confirmStop, titleVisibility: .visible) {
            Button("Stop build", role: .destructive) { Task { await store.cancel(run) } }
            Button("Keep building", role: .cancel) {}
        }
    }
}

struct StatusBadge: View {
    let state: Run.State

    private var look: (String, Color) {
        switch state {
        case .running: return ("Building", .horizon)
        case .success: return ("Ready", .green)
        case .failed: return ("Failed", .red)
        case .cancelled: return ("Stopped", .gray)
        }
    }

    var body: some View {
        let (label, color) = look
        Text(label)
            .font(.caption.bold())
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .foregroundStyle(color)
            .background(color.opacity(0.15), in: Capsule())
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
            Text("\(asset.name) · \(ByteCountFormatter.string(fromByteCount: Int64(asset.size), countStyle: .file))")
                .font(.caption)
                .foregroundStyle(.secondary)
            HStack(spacing: 10) {
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
                    Label("Install", systemImage: "arrow.down.circle.fill")
                        .font(.subheadline.bold())
                        .padding(.horizontal, 16)
                        .padding(.vertical, 8)
                        .foregroundStyle(.white)
                        .background(Color.horizon, in: Capsule())
                }
                if let url {
                    ShareLink(item: url) {
                        Image(systemName: "square.and.arrow.up")
                    }
                    .buttonStyle(.bordered)
                }
                Button {
                    UIPasteboard.general.string = asset.browserDownloadUrl
                    copied = true
                } label: {
                    Image(systemName: copied ? "checkmark" : "doc.on.doc")
                }
                .buttonStyle(.bordered)
            }
            Text("Install it over Horizon: your worlds and game files stay.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}
