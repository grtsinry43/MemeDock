import AppKit
import CoreText
import SwiftUI

/// The bundled Latin face and the system Chinese face share explicit weights.
/// Cascading happens per glyph, including in mixed-language titles.
enum MemeDockTypography {
    static let pageTitle = font(size: 26, weight: 700)
    static let sectionTitle = font(size: 20, weight: 600)
    static let rowTitle = font(size: 16, weight: 500)
    static let body = font(size: 14, weight: 400)
    static let control = font(size: 14, weight: 500)
    static let caption = font(size: 12, weight: 400)
    static let sectionLabel = font(size: 12, weight: 500)

    static func registerFonts() {
        guard let url = Bundle.main.url(forResource: "MemeDockQuicksand", withExtension: "ttf") else {
            NSLog("MemeDock: bundled Quicksand font is missing")
            return
        }
        var error: Unmanaged<CFError>?
        if !CTFontManagerRegisterFontsForURL(url as CFURL, .process, &error) {
            if let error = error?.takeRetainedValue() {
                NSLog("MemeDock: could not register Quicksand: %@", String(describing: error))
            }
        }
    }

    private static func font(size: CGFloat, weight: Int) -> Font {
        let chineseFace: String
        switch weight {
        case 600...: chineseFace = "PingFangSC-Semibold"
        case 500..<600: chineseFace = "PingFangSC-Medium"
        default: chineseFace = "PingFangSC-Regular"
        }
        let descriptor = NSFontDescriptor(name: "Quicksand-Regular", size: size)
            .addingAttributes([
                .variation: [NSNumber(value: 0x77676874): NSNumber(value: weight)],
                .cascadeList: [NSFontDescriptor(name: chineseFace, size: size)],
            ])
        let font = NSFont(descriptor: descriptor, size: size)
            ?? NSFont(name: chineseFace, size: size)
            ?? NSFont.systemFont(ofSize: size)
        return Font(font as CTFont)
    }
}
