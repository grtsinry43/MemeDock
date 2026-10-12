import SwiftUI

/// Tags keep their natural widths and wrap within the inspector or picker.
struct MemeDockTagLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrangement(width: proposal.width ?? .infinity, subviews: subviews).size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let layout = arrangement(width: bounds.width, subviews: subviews)
        for (index, subview) in subviews.enumerated() {
            subview.place(
                at: CGPoint(x: bounds.minX + layout.positions[index].x, y: bounds.minY + layout.positions[index].y),
                anchor: .topLeading,
                proposal: ProposedViewSize(layout.sizes[index])
            )
        }
    }

    private func arrangement(width: CGFloat, subviews: Subviews) -> (size: CGSize, positions: [CGPoint], sizes: [CGSize]) {
        let limit = max(0, width)
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        var usedWidth: CGFloat = 0
        var positions: [CGPoint] = []
        var sizes: [CGSize] = []
        for subview in subviews {
            let size = subview.sizeThatFits(ProposedViewSize(width: limit.isFinite ? limit : nil, height: nil))
            if x > 0, x + size.width > limit {
                x = 0
                y += rowHeight + spacing
                rowHeight = 0
            }
            positions.append(CGPoint(x: x, y: y))
            sizes.append(size)
            usedWidth = max(usedWidth, x + size.width)
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
        return (CGSize(width: usedWidth, height: y + rowHeight), positions, sizes)
    }
}
