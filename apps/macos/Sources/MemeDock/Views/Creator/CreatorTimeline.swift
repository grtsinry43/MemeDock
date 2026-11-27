import SwiftUI

/// Unified video trimmer and scrubber: click or drag anywhere on the filmstrip
/// to scrub, drag either handle to trim, or drag the top selection bar to slide
/// the entire clip window.
struct CreatorTimeline: View {
    @Environment(\.colorScheme) private var scheme
    @Bindable var model: CreatorModel
    @State private var dragStartValue: Double?
    @State private var scrubbing = false

    private let trackHeight: CGFloat = 58
    private let handleWidth: CGFloat = 14

    var body: some View {
        let palette = MemeDockPalette(scheme)
        VStack(spacing: 10) {
            transportBar(palette)
            filmstripTrack(palette)
        }
        .disabled(model.busy)
    }

    private func transportBar(_ palette: MemeDockPalette) -> some View {
        HStack(spacing: 8) {
            Button { model.playback.toggle() } label: {
                Image(systemName: model.playback.playing ? "pause.fill" : "play.fill")
                    .font(.system(size: 13, weight: .semibold))
                    .frame(width: 28, height: 28)
                    .contentShape(Rectangle())
            }
            .buttonStyle(MemeDockButtonStyle(kind: .secondary))
            .disabled(!model.playback.available)
            .help(L10n.text(model.playback.playing ? .pauseAnimation : .playAnimation))
            .accessibilityLabel(L10n.text(model.playback.playing ? .pauseAnimation : .playAnimation))

            Text(formatTime(model.playback.position))
                .font(MemeDockTypography.control)
                .monospacedDigit()
                .foregroundStyle(palette.text)
                .frame(minWidth: 56, alignment: .leading)

            HStack(spacing: 4) {
                Button { model.setStartFromPlayhead() } label: {
                    Image(systemName: "arrow.right.to.line")
                        .font(.system(size: 11, weight: .medium))
                        .frame(width: 24, height: 24)
                }
                .buttonStyle(MemeDockButtonStyle(kind: .quiet))
                .help(L10n.text(.creatorSetStart))
                .accessibilityLabel(L10n.text(.creatorSetStart))

                Button { model.setEndFromPlayhead() } label: {
                    Image(systemName: "arrow.left.to.line")
                        .font(.system(size: 11, weight: .medium))
                        .frame(width: 24, height: 24)
                }
                .buttonStyle(MemeDockButtonStyle(kind: .quiet))
                .help(L10n.text(.creatorSetEnd))
                .accessibilityLabel(L10n.text(.creatorSetEnd))
            }

            Spacer(minLength: 8)

            HStack(spacing: 6) {
                Text("\(formatTime(model.start)) – \(formatTime(model.end))")
                    .font(MemeDockTypography.caption)
                    .monospacedDigit()
                    .foregroundStyle(palette.secondary)
                Text("·")
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(palette.secondary.opacity(0.6))
                Text(String(format: "%.2fs / 30s", max(0, model.end - model.start)))
                    .font(MemeDockTypography.caption)
                    .fontWeight(.medium)
                    .monospacedDigit()
                    .foregroundStyle(palette.text)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(palette.input, in: RoundedRectangle(cornerRadius: MemeDockGeometry.badge + 2))
        }
    }

    private func filmstripTrack(_ palette: MemeDockPalette) -> some View {
        GeometryReader { geometry in
            let width = max(1, geometry.size.width)
            let duration = max(0.001, model.duration)
            let left = CGFloat(model.start / duration) * width
            let right = CGFloat(model.end / duration) * width
            let selectionWidth = max(handleWidth * 2, right - left)
            let playheadX = min(width - 1, max(1, CGFloat(model.playback.position / duration) * width))

            ZStack(alignment: .leading) {
                // Filmstrip thumbnails + direct scrub gesture
                ZStack(alignment: .leading) {
                    HStack(spacing: 0) {
                        if model.thumbnails.isEmpty {
                            Rectangle().fill(palette.input)
                        } else {
                            ForEach(model.thumbnails) { thumbnail in
                                Image(nsImage: thumbnail.image)
                                    .resizable()
                                    .scaledToFill()
                                    .frame(width: width / CGFloat(model.thumbnails.count), height: trackHeight)
                                    .clipped()
                            }
                        }
                    }
                    .frame(width: width, height: trackHeight)
                    .background(palette.input)

                    // Dimmed exterior outside selected range
                    Rectangle()
                        .fill(.black.opacity(0.58))
                        .frame(width: max(0, left), height: trackHeight)
                        .allowsHitTesting(false)
                    Rectangle()
                        .fill(.black.opacity(0.58))
                        .frame(width: max(0, width - right), height: trackHeight)
                        .offset(x: right)
                        .allowsHitTesting(false)
                }
                .clipShape(RoundedRectangle(cornerRadius: MemeDockGeometry.control))
                .contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 0)
                        .onChanged { value in
                            if !scrubbing {
                                scrubbing = true
                                model.playback.pause()
                            }
                            let fraction = min(1, max(0, value.location.x / width))
                            model.playback.seek(Double(fraction) * duration)
                        }
                        .onEnded { _ in
                            scrubbing = false
                        }
                )
                .accessibilityLabel(L10n.text(.creatorPlayhead))

                // Selection border frame (top & bottom rails)
                RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                    .strokeBorder(.white, lineWidth: 2.5)
                    .frame(width: max(1, right - left), height: trackHeight)
                    .offset(x: left)
                    .allowsHitTesting(false)

                // Draggable top bar inside selection to slide the whole [start, end] range
                if selectionWidth > handleWidth * 2 + 24 {
                    ZStack {
                        Capsule()
                            .fill(.white.opacity(0.9))
                            .frame(width: min(36, (selectionWidth - handleWidth * 2) * 0.45), height: 3)
                            .shadow(color: .black.opacity(0.4), radius: 1)
                    }
                    .frame(width: max(1, right - left - handleWidth * 2), height: 16)
                    .contentShape(Rectangle())
                    .offset(x: left + handleWidth, y: -(trackHeight / 2 - 8))
                    .gesture(
                        DragGesture(minimumDistance: 1)
                            .onChanged { value in
                                if dragStartValue == nil {
                                    dragStartValue = model.start
                                    model.playback.pause()
                                }
                                guard let origin = dragStartValue else { return }
                                let delta = Double(value.translation.width / width) * duration
                                model.moveRange(to: origin + delta)
                            }
                            .onEnded { _ in dragStartValue = nil }
                    )
                }

                // Playhead line + top cap
                VStack(spacing: 0) {
                    Capsule()
                        .fill(.white)
                        .frame(width: 8, height: 4)
                        .shadow(color: .black.opacity(0.45), radius: 1.5, y: 0.5)
                    Rectangle()
                        .fill(.white)
                        .frame(width: 2, height: trackHeight + 4)
                        .shadow(color: .black.opacity(0.5), radius: 2)
                }
                .offset(x: playheadX - 4, y: -3)
                .allowsHitTesting(false)

                // Left & right trim handles
                let leftHandleX = min(max(0, left), max(0, width - handleWidth * 2))
                let rightHandleX = max(leftHandleX + handleWidth, min(width - handleWidth, right - handleWidth))
                trimHandle(isStart: true, width: width, duration: duration)
                    .offset(x: leftHandleX)
                trimHandle(isStart: false, width: width, duration: duration)
                    .offset(x: rightHandleX)
            }
        }
        .frame(height: trackHeight)
    }

    private func trimHandle(isStart: Bool, width: CGFloat, duration: Double) -> some View {
        UnevenRoundedRectangle(
            topLeadingRadius: isStart ? MemeDockGeometry.control : 2,
            bottomLeadingRadius: isStart ? MemeDockGeometry.control : 2,
            bottomTrailingRadius: isStart ? 2 : MemeDockGeometry.control,
            topTrailingRadius: isStart ? 2 : MemeDockGeometry.control
        )
        .fill(.white)
        .frame(width: handleWidth, height: trackHeight)
        .overlay {
            Capsule()
                .fill(.black.opacity(0.42))
                .frame(width: 2.5, height: 22)
        }
        .shadow(color: .black.opacity(0.25), radius: 2, x: isStart ? 1 : -1)
        .contentShape(Rectangle().inset(by: -6))
        .gesture(
            DragGesture(minimumDistance: 0)
                .onChanged { value in
                    if dragStartValue == nil {
                        dragStartValue = isStart ? model.start : model.end
                        model.playback.pause()
                    }
                    guard let origin = dragStartValue else { return }
                    let seconds = origin + Double(value.translation.width / width) * duration
                    if isStart { model.setStart(seconds) } else { model.setEnd(seconds) }
                }
                .onEnded { _ in dragStartValue = nil }
        )
        .accessibilityLabel(L10n.text(isStart ? .creatorStart : .creatorEnd))
        .accessibilityAdjustableAction { direction in
            let delta = direction == .increment ? 0.1 : -0.1
            if isStart { model.setStart(model.start + delta) } else { model.setEnd(model.end + delta) }
        }
    }

    private func formatTime(_ seconds: Double) -> String {
        let clamped = max(0, seconds)
        let mins = Int(clamped) / 60
        let secs = clamped - Double(mins * 60)
        if mins > 0 {
            return String(format: "%d:%05.2f", mins, secs)
        }
        return String(format: "%.2fs", clamped)
    }
}
