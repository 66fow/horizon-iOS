import SwiftUI
import UIKit

/// The Horizon launcher's glass look (docs/design-system.md, "Glass"; `horizon_theme::glass`):
/// frosted plates over the pixel sunset, round corners and no borders, one violet accent, Geist
/// for text and Bricolage Grotesque for names, Hugeicons in the text colour.
enum Theme {
    static let accent = Color(hex: 0x6b4bff)
    static let accentSoft = Color(hex: 0xa9a9ff)
    static let base = Color(hex: 0x1b1830)
    static let page = Color(hex: 0x0a0919)
    static let live = Color(hex: 0x4ade80)
    static let warn = Color(hex: 0xf2c46b)
    static let danger = Color(hex: 0xff8a8a)

    /// White text at one of the launcher's opacities (`glass::ink`).
    static func ink(_ opacity: Double) -> Color { Color.white.opacity(opacity) }
    /// A white fill for a state (`glass::fill`): hover 0.05, field 0.07, button 0.08, selected 0.1.
    static func fill(_ opacity: Double) -> Color { Color.white.opacity(opacity) }

    enum Radius {
        static let panel: CGFloat = 20
        static let popover: CGFloat = 16
        static let large: CGFloat = 14
        static let row: CGFloat = 12
        static let control: CGFloat = 10
    }
}

extension Color {
    init(hex: UInt32, opacity: Double = 1) {
        self.init(.sRGB,
                  red: Double((hex >> 16) & 255) / 255,
                  green: Double((hex >> 8) & 255) / 255,
                  blue: Double(hex & 255) / 255,
                  opacity: opacity)
    }
}

extension Font {
    enum GeistWeight { case regular, medium, semibold }

    static func geist(_ size: CGFloat, _ weight: GeistWeight = .regular) -> Font {
        switch weight {
        case .regular: return .custom("Geist-Regular", size: size)
        case .medium: return .custom("Geist-Medium", size: size)
        case .semibold: return .custom("Geist-SemiBold", size: size)
        }
    }

    /// Page names.
    static func display(_ size: CGFloat) -> Font { .custom("BricolageGrotesqueDisplay-SemiBold", size: size) }
    /// Card and dialog names.
    static func heading(_ size: CGFloat) -> Font { .custom("BricolageGrotesqueHeading-SemiBold", size: size) }
    static func mono(_ size: CGFloat) -> Font { .custom("GeistMono-Regular", size: size) }
}

/// The pixel sunset behind every screen, scaled by whole texels.
struct SceneBackground: View {
    private static let image = UIImage(named: "scene")

    var body: some View {
        GeometryReader { geo in
            if let image = Self.image {
                Image(uiImage: image)
                    .resizable()
                    .interpolation(.none)
                    .aspectRatio(contentMode: .fill)
                    .frame(width: geo.size.width, height: geo.size.height)
                    .clipped()
            }
        }
        .background(Theme.page)
        .ignoresSafeArea()
    }
}

/// A frosted plate: the scene blurred, under white at 6 % and the accent's dark violet at 22 %.
struct GlassPlate: View {
    let radius: CGFloat
    var fill: Double = 0

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: radius, style: .continuous)
        shape.fill(.ultraThinMaterial)
            .overlay(shape.fill(Theme.base.opacity(0.22)))
            .overlay(shape.fill(Theme.fill(0.06 + fill)))
    }
}

extension View {
    func glass(_ radius: CGFloat = Theme.Radius.panel, fill: Double = 0) -> some View {
        background(GlassPlate(radius: radius, fill: fill))
    }
}

/// One of the launcher's Hugeicons, tinted with the foreground colour.
struct HIcon: View {
    let name: String
    var size: CGFloat = 20

    var body: some View {
        if let image = UIImage(named: "icon-" + name) {
            Image(uiImage: image)
                .renderingMode(.template)
                .resizable()
                .interpolation(.high)
                .frame(width: size, height: size)
        } else {
            Color.clear.frame(width: size, height: size)
        }
    }
}

/// The one main action on a screen: the accent, white text.
struct AccentButton: ButtonStyle {
    var height: CGFloat = 48

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.geist(15, .semibold))
            .foregroundStyle(Color.white)
            .padding(.horizontal, 20)
            .frame(height: height)
            .background(RoundedRectangle(cornerRadius: Theme.Radius.large, style: .continuous)
                .fill(Theme.accent.opacity(configuration.isPressed ? 0.85 : 1)))
    }
}

/// A quiet button: white text on a faint fill.
struct QuietButton: ButtonStyle {
    var tint: Color = Theme.ink(0.85)

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.geist(14, .medium))
            .foregroundStyle(tint)
            .padding(.horizontal, 14)
            .frame(height: 38)
            .background(RoundedRectangle(cornerRadius: Theme.Radius.control, style: .continuous)
                .fill(Theme.fill(configuration.isPressed ? 0.14 : 0.08)))
    }
}

struct PageHeader: View {
    let title: String
    let caption: String
    private static let mark = UIImage(named: "mark")

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                if let mark = Self.mark {
                    Image(uiImage: mark).resizable().interpolation(.none).frame(width: 20, height: 20)
                }
                Text("Horizon").font(.geist(13, .semibold)).foregroundStyle(Theme.ink(0.75))
            }
            Text(title)
                .font(.display(40))
                .foregroundStyle(Theme.ink(1))
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            Text(caption)
                .font(.geist(14))
                .foregroundStyle(Theme.ink(0.7))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.top, 8)
    }
}

/// A titled panel.
struct Plate<Content: View>: View {
    let title: String?
    let content: Content

    init(_ title: String? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let title {
                Text(title).font(.heading(19)).foregroundStyle(Theme.ink(1))
            }
            content
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glass()
    }
}

/// A one-line state: a small square in the state's colour, then the words.
struct StateChip: View {
    let label: String
    let color: Color

    var body: some View {
        HStack(spacing: 6) {
            Rectangle().fill(color).frame(width: 6, height: 6)
            Text(label).font(.geist(12, .medium)).foregroundStyle(color)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(Capsule().fill(color.opacity(0.14)))
    }
}

struct ErrorPlate: View {
    let message: String
    let retry: () -> Void

    var body: some View {
        Plate {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Rectangle().fill(Theme.danger).frame(width: 6, height: 6)
                Text(message).font(.geist(14)).foregroundStyle(Theme.ink(0.85))
            }
            Button("Try again", action: retry).buttonStyle(QuietButton())
        }
    }
}

/// A text field on the launcher's field fill.
struct FieldBox<Content: View>: View {
    let content: Content

    init(@ViewBuilder content: () -> Content) { self.content = content() }

    var body: some View {
        HStack(spacing: 8) { content }
            .padding(.horizontal, 12)
            .frame(height: 44)
            .background(RoundedRectangle(cornerRadius: Theme.Radius.control, style: .continuous)
                .fill(Theme.fill(0.07)))
    }
}

struct ProgressBar: View {
    let value: Double

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(Theme.fill(0.08))
                Capsule().fill(Theme.accentSoft)
                    .frame(width: max(4, geo.size.width * min(max(value, 0), 1)))
            }
        }
        .frame(height: 4)
        .animation(.easeOut(duration: 0.24), value: value)
    }
}
