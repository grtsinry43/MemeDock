import AppKit
import SwiftUI

/// Direct-manipulation crop overlay: drag inside to move, drag any of the four
/// corners or four edges to resize, double-click to reset, and tap to toggle playback.
struct CreatorCropOverlay: View {
    @Binding var crop: CGRect
    @Binding var aspectLock: Double?
    let sourceSize: CGSize
    var onTap: (() -> Void)? = nil
    var onReset: (() -> Void)? = nil

    private enum Handle: CaseIterable {
        case topLeft, topRight, bottomLeft, bottomRight
        case top, bottom, left, right

        var isCorner: Bool {
            switch self {
            case .topLeft, .topRight, .bottomLeft, .bottomRight: return true
            case .top, .bottom, .left, .right: return false
            }
        }
    }

    @State private var initial: CGRect?
    @State private var active = false
    @State private var movedDuringDrag = false

    private let minNormalizedSize: CGFloat = 0.05

    var body: some View {
        GeometryReader { geometry in
            let size = geometry.size
            let rect = CGRect(
                x: crop.minX * size.width,
                y: crop.minY * size.height,
                width: crop.width * size.width,
                height: crop.height * size.height
            )
            ZStack(alignment: .topLeading) {
                // Dim exterior
                Path { path in
                    path.addRect(CGRect(origin: .zero, size: size))
                    path.addRect(rect)
                }
                .fill(.black.opacity(0.52), style: FillStyle(eoFill: true))
                .allowsHitTesting(false)

                // Draggable interior (move crop, tap to toggle playback, double-click to reset)
                Rectangle()
                    .fill(.clear)
                    .contentShape(Rectangle())
                    .overlay {
                        ZStack {
                            if active {
                                RuleOfThirdsShape()
                                    .stroke(.white.opacity(0.35), lineWidth: 0.75)
                                    .transition(.opacity)
                            }
                            Rectangle()
                                .strokeBorder(.white.opacity(0.95), lineWidth: 1.5)
                        }
                    }
                    .frame(width: max(1, rect.width), height: max(1, rect.height))
                    .position(x: rect.midX, y: rect.midY)
                    .gesture(
                        TapGesture(count: 2).onEnded {
                            onReset?()
                        }
                    )
                    .simultaneousGesture(
                        DragGesture(minimumDistance: 0)
                            .onChanged { value in
                                let distance = hypot(value.translation.width, value.translation.height)
                                if !movedDuringDrag && distance >= 3 {
                                    movedDuringDrag = true
                                    active = true
                                    initial = crop
                                }
                                guard movedDuringDrag, let origin = initial else { return }
                                let nx = origin.minX + value.translation.width / max(1, size.width)
                                let ny = origin.minY + value.translation.height / max(1, size.height)
                                crop.origin.x = min(1 - origin.width, max(0, nx))
                                crop.origin.y = min(1 - origin.height, max(0, ny))
                            }
                            .onEnded { _ in
                                if !movedDuringDrag {
                                    onTap?()
                                }
                                initial = nil
                                active = false
                                movedDuringDrag = false
                            }
                    )
                    .accessibilityLabel(L10n.text(.creatorCrop))

                // Four edge resize zones
                edgeHandle(.top, rect: rect, size: size)
                edgeHandle(.bottom, rect: rect, size: size)
                edgeHandle(.left, rect: rect, size: size)
                edgeHandle(.right, rect: rect, size: size)

                // Four corner resize brackets
                cornerHandle(.topLeft, rect: rect, size: size)
                cornerHandle(.topRight, rect: rect, size: size)
                cornerHandle(.bottomLeft, rect: rect, size: size)
                cornerHandle(.bottomRight, rect: rect, size: size)

                // Live pixel size badge while adjusting
                if active {
                    let pxW = max(1, Int((Double(sourceSize.width) * crop.width).rounded()))
                    let pxH = max(1, Int((Double(sourceSize.height) * crop.height).rounded()))
                    Text("\(pxW) × \(pxH)")
                        .font(MemeDockTypography.caption)
                        .monospacedDigit()
                        .foregroundStyle(.white)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(.black.opacity(0.72), in: RoundedRectangle(cornerRadius: 6))
                        .position(
                            x: min(max(44, rect.midX), max(44, size.width - 44)),
                            y: rect.minY > 28 ? rect.minY - 16 : rect.minY + 18
                        )
                        .allowsHitTesting(false)
                        .transition(.opacity)
                }
            }
            .animation(MemeDockMotion.feedback, value: active)
        }
        .accessibilityElement(children: .contain)
    }

    private func cornerHandle(_ handle: Handle, rect: CGRect, size: CGSize) -> some View {
        let point: CGPoint
        switch handle {
        case .topLeft: point = CGPoint(x: rect.minX, y: rect.minY)
        case .topRight: point = CGPoint(x: rect.maxX, y: rect.minY)
        case .bottomLeft: point = CGPoint(x: rect.minX, y: rect.maxY)
        case .bottomRight: point = CGPoint(x: rect.maxX, y: rect.maxY)
        default: point = .zero
        }

        return CornerBracket(corner: handle)
            .stroke(.white, style: StrokeStyle(lineWidth: 3, lineCap: .round, lineJoin: .round))
            .shadow(color: .black.opacity(0.45), radius: 2, y: 1)
            .frame(width: 28, height: 28)
            .contentShape(Rectangle())
            .position(point)
            .gesture(resizeGesture(handle: handle, size: size))
            .accessibilityLabel(L10n.text(.creatorResizeCrop))
    }

    private func edgeHandle(_ handle: Handle, rect: CGRect, size: CGSize) -> some View {
        let horizontal = handle == .top || handle == .bottom
        let length = max(20, (horizontal ? rect.width : rect.height) - 28)
        let width: CGFloat = horizontal ? length : 16
        let height: CGFloat = horizontal ? 16 : length
        let point: CGPoint
        switch handle {
        case .top: point = CGPoint(x: rect.midX, y: rect.minY)
        case .bottom: point = CGPoint(x: rect.midX, y: rect.maxY)
        case .left: point = CGPoint(x: rect.minX, y: rect.midY)
        case .right: point = CGPoint(x: rect.maxX, y: rect.midY)
        default: point = .zero
        }

        return ZStack {
            Capsule()
                .fill(.white.opacity(0.9))
                .shadow(color: .black.opacity(0.45), radius: 1.5, y: 0.5)
                .frame(width: horizontal ? min(24, length * 0.5) : 3,
                       height: horizontal ? 3 : min(24, length * 0.5))
        }
        .frame(width: width, height: height)
        .contentShape(Rectangle())
        .position(point)
        .gesture(resizeGesture(handle: handle, size: size))
        .accessibilityLabel(L10n.text(.creatorResizeCrop))
    }

    private func resizeGesture(handle: Handle, size: CGSize) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { value in
                let origin = initial ?? crop
                if initial == nil {
                    initial = origin
                    active = true
                }
                let dx = value.translation.width / max(1, size.width)
                let dy = value.translation.height / max(1, size.height)

                if !handle.isCorner {
                    // Edge drag unlocks fixed aspect ratio for natural freeform cropping.
                    if aspectLock != nil { aspectLock = nil }
                    crop = resizedFreeform(origin: origin, handle: handle, dx: dx, dy: dy)
                } else if let aspect = aspectLock, sourceSize.height > 0, sourceSize.width > 0 {
                    let sourceAspect = Double(sourceSize.width) / Double(sourceSize.height)
                    let normAspect = CGFloat(aspect / sourceAspect)
                    crop = resizedLocked(origin: origin, handle: handle, dx: dx, dy: dy, normAspect: normAspect)
                } else {
                    crop = resizedFreeform(origin: origin, handle: handle, dx: dx, dy: dy)
                }
            }
            .onEnded { _ in
                initial = nil
                active = false
            }
    }

    private func resizedFreeform(origin: CGRect, handle: Handle, dx: CGFloat, dy: CGFloat) -> CGRect {
        var minX = origin.minX
        var maxX = origin.maxX
        var minY = origin.minY
        var maxY = origin.maxY

        switch handle {
        case .topLeft:
            minX = min(maxX - minNormalizedSize, max(0, origin.minX + dx))
            minY = min(maxY - minNormalizedSize, max(0, origin.minY + dy))
        case .topRight:
            maxX = max(minX + minNormalizedSize, min(1, origin.maxX + dx))
            minY = min(maxY - minNormalizedSize, max(0, origin.minY + dy))
        case .bottomLeft:
            minX = min(maxX - minNormalizedSize, max(0, origin.minX + dx))
            maxY = max(minY + minNormalizedSize, min(1, origin.maxY + dy))
        case .bottomRight:
            maxX = max(minX + minNormalizedSize, min(1, origin.maxX + dx))
            maxY = max(minY + minNormalizedSize, min(1, origin.maxY + dy))
        case .top:
            minY = min(maxY - minNormalizedSize, max(0, origin.minY + dy))
        case .bottom:
            maxY = max(minY + minNormalizedSize, min(1, origin.maxY + dy))
        case .left:
            minX = min(maxX - minNormalizedSize, max(0, origin.minX + dx))
        case .right:
            maxX = max(minX + minNormalizedSize, min(1, origin.maxX + dx))
        }
        return CGRect(x: minX, y: minY, width: maxX - minX, height: maxY - minY)
    }

    private func resizedLocked(
        origin: CGRect,
        handle: Handle,
        dx: CGFloat,
        dy: CGFloat,
        normAspect: CGFloat
    ) -> CGRect {
        let anchorX: CGFloat
        let anchorY: CGFloat
        let rawW: CGFloat
        let rawH: CGFloat
        let maxW: CGFloat
        let maxH: CGFloat
        let isLeft: Bool
        let isTop: Bool

        switch handle {
        case .topLeft:
            anchorX = origin.maxX; anchorY = origin.maxY
            rawW = origin.width - dx; rawH = origin.height - dy
            maxW = anchorX; maxH = anchorY
            isLeft = true; isTop = true
        case .topRight:
            anchorX = origin.minX; anchorY = origin.maxY
            rawW = origin.width + dx; rawH = origin.height - dy
            maxW = 1 - anchorX; maxH = anchorY
            isLeft = false; isTop = true
        case .bottomLeft:
            anchorX = origin.maxX; anchorY = origin.minY
            rawW = origin.width - dx; rawH = origin.height + dy
            maxW = anchorX; maxH = 1 - anchorY
            isLeft = true; isTop = false
        default: // .bottomRight
            anchorX = origin.minX; anchorY = origin.minY
            rawW = origin.width + dx; rawH = origin.height + dy
            maxW = 1 - anchorX; maxH = 1 - anchorY
            isLeft = false; isTop = false
        }

        // Drive from whichever axis moved further proportionally
        var width: CGFloat
        var height: CGFloat
        if abs(dx) >= abs(dy) {
            width = max(minNormalizedSize, rawW)
            height = width / normAspect
        } else {
            height = max(minNormalizedSize, rawH)
            width = height * normAspect
        }

        // Clamp within frame bounds while preserving aspect ratio
        if width > maxW {
            width = maxW
            height = width / normAspect
        }
        if height > maxH {
            height = maxH
            width = height * normAspect
        }
        if width < minNormalizedSize {
            width = minNormalizedSize
            height = width / normAspect
        }
        if height < minNormalizedSize {
            height = minNormalizedSize
            width = height * normAspect
        }

        let x = isLeft ? anchorX - width : anchorX
        let y = isTop ? anchorY - height : anchorY
        return CGRect(x: max(0, min(1 - width, x)), y: max(0, min(1 - height, y)), width: width, height: height)
    }

    private struct CornerBracket: Shape {
        let corner: Handle

        func path(in rect: CGRect) -> Path {
            let cx = rect.midX
            let cy = rect.midY
            let arm: CGFloat = 10
            var path = Path()
            switch corner {
            case .topLeft:
                path.move(to: CGPoint(x: cx, y: cy + arm))
                path.addLine(to: CGPoint(x: cx, y: cy))
                path.addLine(to: CGPoint(x: cx + arm, y: cy))
            case .topRight:
                path.move(to: CGPoint(x: cx - arm, y: cy))
                path.addLine(to: CGPoint(x: cx, y: cy))
                path.addLine(to: CGPoint(x: cx, y: cy + arm))
            case .bottomLeft:
                path.move(to: CGPoint(x: cx, y: cy - arm))
                path.addLine(to: CGPoint(x: cx, y: cy))
                path.addLine(to: CGPoint(x: cx + arm, y: cy))
            case .bottomRight:
                path.move(to: CGPoint(x: cx - arm, y: cy))
                path.addLine(to: CGPoint(x: cx, y: cy))
                path.addLine(to: CGPoint(x: cx, y: cy - arm))
            default:
                break
            }
            return path
        }
    }
}

private struct RuleOfThirdsShape: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        for fraction in [CGFloat(1) / 3, CGFloat(2) / 3] {
            let x = rect.minX + rect.width * fraction
            let y = rect.minY + rect.height * fraction
            path.move(to: CGPoint(x: x, y: rect.minY))
            path.addLine(to: CGPoint(x: x, y: rect.maxY))
            path.move(to: CGPoint(x: rect.minX, y: y))
            path.addLine(to: CGPoint(x: rect.maxX, y: y))
        }
        return path
    }
}
