import SwiftUI

/// Surface and type roles come from the Android theme. Desktop navigation and
/// action emphasis use neutral contrast; brand blue is reserved for the caret.
struct MemeDockPalette {
    let canvas: Color
    let raised: Color
    let input: Color
    let pressed: Color
    let text: Color
    let secondary: Color
    let separator: Color
    let accent: Color
    let onAccent: Color
    let inverse: Color
    let onInverse: Color
    let star: Color
    let error: Color
    let success: Color

    init(_ scheme: ColorScheme) {
        if scheme == .dark {
            canvas = Color(rgb: 0x0F1114)
            raised = Color(rgb: 0x181B20)
            input = Color(rgb: 0x23272D)
            pressed = Color(rgb: 0x2C3037)
            text = Color(rgb: 0xE8EAED)
            secondary = Color(rgb: 0xA4AAB4)
            separator = Color(rgb: 0x2A2E35)
            accent = Color(rgb: 0x6EA8FF)
            onAccent = Color(rgb: 0x002A66)
            inverse = Color(rgb: 0xE8EAED)
            onInverse = Color(rgb: 0x1A1D21)
            star = Color(rgb: 0xFFC94D)
            error = Color(rgb: 0xFF8A80)
            success = Color(rgb: 0x1E8E4E)
        } else {
            canvas = Color(rgb: 0xF6F7F9)
            raised = .white
            input = Color(rgb: 0xEBEDF1)
            pressed = Color(rgb: 0xE2E5EA)
            text = Color(rgb: 0x16181C)
            secondary = Color(rgb: 0x565D68)
            separator = Color(rgb: 0xE1E4E9)
            accent = Color(rgb: 0x1F6FEB)
            onAccent = .white
            inverse = Color(rgb: 0x24272C)
            onInverse = Color(rgb: 0xF1F2F4)
            star = Color(rgb: 0xE29A00)
            error = Color(rgb: 0xD93025)
            success = Color(rgb: 0x5BD68A)
        }
    }
}

enum MemeDockGeometry {
    static let badge: CGFloat = 6
    static let control: CGFloat = 10
    static let controlHeight: CGFloat = 36
    static let message: CGFloat = 16
    static let group: CGFloat = 20
    static let dialog: CGFloat = 10
    static let pageInset: CGFloat = 16
    static let sectionGap: CGFloat = 24
}

enum MemeDockMotion {
    static let feedback = Animation.timingCurve(0.2, 0, 0, 1, duration: 0.18)
    static let fade = Animation.timingCurve(0.2, 0, 0, 1, duration: 0.20)
    static let panel = Animation.timingCurve(0.2, 0, 0, 1, duration: 0.30)
}

private extension Color {
    init(rgb: UInt32) {
        self.init(.sRGB,
                  red: Double((rgb >> 16) & 0xFF) / 255,
                  green: Double((rgb >> 8) & 0xFF) / 255,
                  blue: Double(rgb & 0xFF) / 255,
                  opacity: 1)
    }
}
