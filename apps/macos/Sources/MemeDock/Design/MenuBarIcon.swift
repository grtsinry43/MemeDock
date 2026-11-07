import AppKit

/// The foreground sticker from the shared app icon, adapted to an 18 pt
/// template: no background or shadow, with the face and fold cut out in alpha.
enum MenuBarIcon {
    static let image: NSImage = {
        let image = NSImage(size: NSSize(width: 18, height: 18), flipped: true) { _ in
            guard let context = NSGraphicsContext.current?.cgContext else { return false }
            context.saveGState()
            defer { context.restoreGState() }
            context.translateBy(x: 9, y: 9)
            context.scaleBy(x: 0.28, y: 0.28)
            context.rotate(by: -8 * .pi / 180)
            context.translateBy(x: -54, y: -54)
            context.setFillColor(NSColor.black.cgColor)
            context.move(to: CGPoint(x: 40, y: 27))
            context.addLine(to: CGPoint(x: 68, y: 27))
            context.addCurve(to: CGPoint(x: 81, y: 40), control1: CGPoint(x: 75.18, y: 27), control2: CGPoint(x: 81, y: 32.82))
            context.addLine(to: CGPoint(x: 81, y: 65))
            context.addLine(to: CGPoint(x: 65, y: 81))
            context.addLine(to: CGPoint(x: 40, y: 81))
            context.addCurve(to: CGPoint(x: 27, y: 68), control1: CGPoint(x: 32.82, y: 81), control2: CGPoint(x: 27, y: 75.18))
            context.addLine(to: CGPoint(x: 27, y: 40))
            context.addCurve(to: CGPoint(x: 40, y: 27), control1: CGPoint(x: 27, y: 32.82), control2: CGPoint(x: 32.82, y: 27))
            context.closePath()
            context.fillPath()

            context.setBlendMode(.clear)
            for x: CGFloat in [45, 58] {
                context.fillEllipse(in: CGRect(x: x - 3.6, y: 43.4, width: 7.2, height: 7.2))
            }
            context.setLineWidth(4.4)
            context.setLineCap(.round)
            context.move(to: CGPoint(x: 42.5, y: 56))
            context.addQuadCurve(to: CGPoint(x: 60.5, y: 56), control: CGPoint(x: 51.5, y: 65))
            context.strokePath()
            // Keep a visible rim around the curl at menu-bar scale.
            context.move(to: CGPoint(x: 76, y: 67.5))
            context.addLine(to: CGPoint(x: 71, y: 67.5))
            context.addQuadCurve(to: CGPoint(x: 67.5, y: 71), control: CGPoint(x: 67.5, y: 67.5))
            context.addLine(to: CGPoint(x: 67.5, y: 76))
            context.closePath()
            context.fillPath()
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = "MemeDock"
        return image
    }()
}
