import CryptoKit
import SwiftUI
import UIKit

/// Mod icons and screenshots: downloaded when first shown, kept in the app's Caches folder.
actor ImageCache {
    static let shared = ImageCache()

    private let memory = NSCache<NSString, UIImage>()
    private var running: [String: Task<UIImage?, Never>] = [:]
    private let folder = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("images")

    func image(_ url: URL, maxPixels: CGFloat) async -> UIImage? {
        let key = "\(url.absoluteString)#\(Int(maxPixels))"
        if let hit = memory.object(forKey: key as NSString) { return hit }
        if let task = running[key] { return await task.value }
        let folder = self.folder
        let task = Task<UIImage?, Never> { await Self.load(url, maxPixels: maxPixels, folder: folder) }
        running[key] = task
        let image = await task.value
        running[key] = nil
        if let image { memory.setObject(image, forKey: key as NSString) }
        return image
    }

    private static func load(_ url: URL, maxPixels: CGFloat, folder: URL) async -> UIImage? {
        let digest = SHA256.hash(data: Data(url.absoluteString.utf8)).map { String(format: "%02x", $0) }.joined()
        let file = folder.appendingPathComponent(digest)
        var data = try? Data(contentsOf: file)
        if data == nil {
            guard let fetched = try? await URLSession.shared.data(from: url),
                  (fetched.1 as? HTTPURLResponse)?.statusCode == 200,
                  fetched.0.count < 12_000_000 else { return nil }
            try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try? fetched.0.write(to: file, options: .atomic)
            data = fetched.0
        }
        guard let data, let image = UIImage(data: data) else { return nil }
        let longest = max(image.size.width, image.size.height) * image.scale
        // Small pictures are often pixel art: kept as they are, drawn without smoothing.
        if longest <= maxPixels || longest <= 64 { return image }
        let k = maxPixels / longest
        let size = CGSize(width: image.size.width * k, height: image.size.height * k)
        return await image.byPreparingThumbnail(ofSize: size) ?? image
    }
}

/// A picture from the web, filling its frame; nothing while it loads or when it can't.
struct RemoteImage: View {
    let url: String?
    let maxPixels: CGFloat
    var contentMode: ContentMode = .fill
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            if let image {
                Image(uiImage: image)
                    .resizable()
                    .interpolation(image.size.width * image.scale <= 64 ? .none : .high)
                    .aspectRatio(contentMode: contentMode)
                    .transition(.opacity)
            }
        }
        .task(id: url) {
            guard let url, let link = URL(string: url), link.scheme == "https" else {
                image = nil
                return
            }
            let loaded = await ImageCache.shared.image(link, maxPixels: maxPixels)
            withAnimation(.easeOut(duration: 0.15)) { image = loaded }
        }
    }
}

/// A mod's icon in a slot, or the puzzle glyph when it has none.
struct ModIcon: View {
    let url: String?
    var size: CGFloat = 52

    var body: some View {
        Theme.fill(0.08)
            .overlay(HIcon(name: "puzzle", size: size * 0.42).foregroundStyle(Theme.ink(0.45)))
            .overlay(RemoteImage(url: url, maxPixels: size * 3))
            .frame(width: size, height: size)
            .clipShape(RoundedRectangle(cornerRadius: size * 0.23, style: .continuous))
    }
}

/// A mod's screenshots, swiped through.
struct Gallery: View {
    let urls: [String]
    @State private var open: String?

    var body: some View {
        TabView {
            ForEach(urls, id: \.self) { url in
                Theme.fill(0.06)
                    .overlay(RemoteImage(url: url, maxPixels: 1400))
                    .clipShape(RoundedRectangle(cornerRadius: Theme.Radius.popover, style: .continuous))
                    .padding(.horizontal, 2)
                    .onTapGesture { open = url }
            }
        }
        .tabViewStyle(.page(indexDisplayMode: urls.count > 1 ? .always : .never))
        .frame(height: 210)
        .fullScreenCover(item: Binding(get: { open.map(Shown.init) }, set: { open = $0?.url })) { shown in
            ZStack(alignment: .topTrailing) {
                Color.black.ignoresSafeArea()
                RemoteImage(url: shown.url, maxPixels: 2400, contentMode: .fit)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                Button { open = nil } label: {
                    HIcon(name: "cancel-01", size: 18).foregroundStyle(Theme.ink(1)).padding(12).glass(Theme.Radius.row)
                }
                .buttonStyle(.plain)
                .padding(16)
            }
        }
    }

    private struct Shown: Identifiable {
        let url: String
        var id: String { url }
    }
}
