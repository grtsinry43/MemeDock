// render.swift <svg> <png> <scale> [font.ttf...]: draws the SVG with AppKit at the scale, so the
// installer's background is made on a build machine with nothing installed. Font files given are
// registered for this process alone, so the SVG may name a face the system does not have without
// the machine having it installed. Ported from rdm's pkgs/macos/render.swift -- it is the whole
// trick, and it needs no change.
import AppKit

let args = CommandLine.arguments
guard args.count >= 4, let scale = Double(args[3]) else {
	fputs("usage: render.swift <svg> <png> <scale> [font.ttf...]\n", stderr)
	exit(2)
}
let source = URL(fileURLWithPath: args[1])
let dest = URL(fileURLWithPath: args[2])
for font in args.dropFirst(4) {
	var error: Unmanaged<CFError>?
	if !CTFontManagerRegisterFontsForURL(URL(fileURLWithPath: font) as CFURL, .process, &error) {
		fputs("cannot register \(font): \(error!.takeRetainedValue())\n", stderr)
		exit(1)
	}
}
guard let image = NSImage(contentsOf: source) else {
	fputs("cannot read \(source.path)\n", stderr)
	exit(1)
}
let width = Int(image.size.width * scale)
let height = Int(image.size.height * scale)
let rep = NSBitmapImageRep(
	bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8, samplesPerPixel: 4,
	hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
)!
// The rep keeps the SVG's size in points, so drawing at the image's size fills it at the scale.
rep.size = image.size
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
NSGraphicsContext.current!.imageInterpolation = .high
image.draw(in: NSRect(origin: .zero, size: image.size))
NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: dest)
print("\(width)x\(height) -> \(dest.path)")
