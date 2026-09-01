import AppKit

// The platform's render of the scene `src/main.rs` draws: one square of Apple
// glass over a blurred, frost-tinted window, with text under it to refract.
//
// Both windows are the same size at the same measurements, so a difference on
// screen is the material and nothing else. Drag the card; the readout gives its
// rect in screen pixels so a screenshot can be decoded against a known rect.
//
//     swiftc -O reference.swift -o reference && ./reference [clear] [light]
//
// Drag the window over a wallpaper with structure in it — over a flat desktop
// a working lens and a dead one look the same.

let CARD: CGFloat = 168
let CARD_RADIUS: CGFloat = 34
let WELL = NSSize(width: 400, height: 360)
let PAD: CGFloat = 20
let FOOT: CGFloat = 86
/// Room for the titlebar the content view runs under.
let TOP: CGFloat = 36

let SPECIMEN = "the quick brown fox jumps over the lazy dog and back again"
let TEXT_SIZE: CGFloat = 15
let LINE_H: CGFloat = 24

func grey(_ v: CGFloat) -> NSColor {
    NSColor(srgbRed: v / 255, green: v / 255, blue: v / 255, alpha: 1)
}

/// bezel's `Theme::glass()` at `Theme::GLASS_ALPHA`.
func frost(dark: Bool) -> NSColor {
    (dark ? grey(8) : grey(235)).withAlphaComponent(0.80)
}

/// The app content the glass floats over: the frost tint over the whole window,
/// as bezel paints `window_bg()`, and the text block inside `rect`.
final class Well: NSView {
    var dark = true
    var rect = NSRect.zero

    override var isFlipped: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        frost(dark: dark).setFill()
        bounds.fill()
        NSGraphicsContext.saveGraphicsState()
        NSBezierPath(rect: rect).setClip()
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: TEXT_SIZE),
            .foregroundColor: dark ? grey(235) : grey(20),
        ]
        var y = rect.minY
        var line = 0
        while y < rect.maxY {
            // Rotated per line so the block is not a vertical grid, which would
            // be as periodic as the bars it replaced.
            let text = String(SPECIMEN.dropFirst(line * 5 % 17))
            text.draw(at: NSPoint(x: rect.minX + 8, y: y), withAttributes: attrs)
            y += LINE_H
            line += 1
        }
        NSGraphicsContext.restoreGraphicsState()
    }
}

/// Drag moves the card itself, so a click on the glass repositions it.
final class Card: NSGlassEffectView {
    private var grab: NSPoint?

    override func mouseDown(with event: NSEvent) {
        grab = convert(event.locationInWindow, from: nil)
    }

    override func mouseDragged(with event: NSEvent) {
        guard let grab, let parent = superview else { return }
        let at = parent.convert(event.locationInWindow, from: nil)
        frame.origin = NSPoint(x: at.x - grab.x, y: at.y - grab.y)
        (window?.delegate as? Delegate)?.report(self)
    }
}

final class Delegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    var dark = !CommandLine.arguments.contains("light")
    var style = CommandLine.arguments.contains("clear") ? 1 : 0
    var mounted: [NSView] = []
    var readout: NSTextField!

    func applicationDidFinishLaunching(_ note: Notification) {
        let w = PAD * 2 + WELL.width
        let h = FOOT + WELL.height + TOP
        let frame = NSRect(x: 0, y: 0, width: w, height: h)
        window = NSWindow(contentRect: frame,
                          styleMask: [.titled, .closable, .miniaturizable, .fullSizeContentView],
                          backing: .buffered, defer: false)
        window.title = "vibrancy — glass on a blurred window"
        window.titlebarAppearsTransparent = true
        window.delegate = self
        // `.behindWindow` vibrancy needs the window to let the desktop reach it.
        window.isOpaque = false
        // The titlebar moves the window; a background drag would steal the
        // card's own.
        window.isMovableByWindowBackground = false
        window.backgroundColor = .clear
        window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        NSApp.appearance = window.appearance

        let root = NSView(frame: frame)
        window.contentView = root

        // Before the first build, which reports into it.
        readout = NSTextField(labelWithString: "")
        readout.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        readout.textColor = .secondaryLabelColor
        readout.frame = NSRect(x: PAD, y: 18, width: w - PAD * 2, height: 16)
        root.addSubview(readout)

        build(into: root)

        let material = NSSegmentedControl(labels: ["regular", "clear"], trackingMode: .selectOne,
                                          target: self, action: #selector(pickStyle(_:)))
        material.selectedSegment = style
        material.frame = NSRect(x: PAD, y: 44, width: 160, height: 24)
        root.addSubview(material)

        let look = NSSegmentedControl(labels: ["dark", "light"], trackingMode: .selectOne,
                                      target: self, action: #selector(pickAppearance(_:)))
        look.selectedSegment = dark ? 0 : 1
        look.frame = NSRect(x: PAD + 172, y: 44, width: 120, height: 24)
        root.addSubview(look)

        let reset = NSButton(title: "reset", target: self, action: #selector(resetCard))
        reset.bezelStyle = .rounded
        reset.frame = NSRect(x: PAD + 304, y: 42, width: 72, height: 26)
        root.addSubview(reset)

        let screen = NSScreen.screens[0]
        window.setFrameTopLeftPoint(NSPoint(x: screen.frame.minX + 60, y: screen.frame.maxY - 60))
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        // A window that is not key carries a different material, which is the
        // trap the first round of measurements fell into.
        Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { timer in
            guard !self.window.isKeyWindow else { return timer.invalidate() }
            NSApp.activate(ignoringOtherApps: true)
            self.window.makeKeyAndOrderFront(nil)
        }
    }

    /// Built rather than restyled: a glass view is only known to pick up the
    /// appearance it was constructed under.
    func build(into root: NSView) {
        for view in mounted { view.removeFromSuperview() }
        mounted = []

        // The vibrancy IS the window, as it is in bezel: one blurred background
        // under everything, never an inset panel with the desktop around it.
        let blur = NSVisualEffectView(frame: root.bounds)
        blur.material = .underWindowBackground
        blur.blendingMode = .behindWindow
        blur.state = .active
        blur.autoresizingMask = [.width, .height]
        let content = Well(frame: root.bounds)
        content.dark = dark
        content.rect = NSRect(x: PAD, y: TOP, width: WELL.width, height: WELL.height)
        content.autoresizingMask = [.width, .height]
        blur.addSubview(content)
        root.addSubview(blur, positioned: .below, relativeTo: nil)
        mounted.append(blur)

        let box = NSRect(x: PAD, y: FOOT, width: WELL.width, height: WELL.height)
        let at = NSRect(x: box.minX + (WELL.width - CARD) / 2,
                        y: box.minY + (WELL.height - CARD) / 2,
                        width: CARD, height: CARD)
        let card = Card(frame: at)
        card.cornerRadius = CARD_RADIUS
        if let s = NSGlassEffectView.Style(rawValue: style) { card.style = s }
        root.addSubview(card)
        mounted.append(card)
        report(card)
    }

    func report(_ card: NSView) {
        let onScreen = window.convertToScreen(card.convert(card.bounds, to: nil))
        let screen = NSScreen.screens[0]
        let s = screen.backingScaleFactor
        // Relative to the text block's top-left, y downwards — bezel's frame,
        // so the two readouts can be read against each other.
        readout.stringValue = String(
            format: "%.0f×%.0f r%.0f @ (%.0f, %.0f) · screen px %.0f %.0f",
            card.frame.width, card.frame.height, CARD_RADIUS,
            card.frame.minX - PAD, FOOT + WELL.height - card.frame.maxY,
            (onScreen.minX - screen.frame.minX) * s, (screen.frame.maxY - onScreen.maxY) * s)
    }

    @objc func resetCard() { build(into: window.contentView!) }

    @objc func pickStyle(_ sender: NSSegmentedControl) {
        style = sender.selectedSegment
        build(into: window.contentView!)
    }

    @objc func pickAppearance(_ sender: NSSegmentedControl) {
        dark = sender.selectedSegment == 0
        let appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        NSApp.appearance = appearance
        window.appearance = appearance
        build(into: window.contentView!)
    }
}

extension Delegate: NSWindowDelegate {}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = Delegate()
app.delegate = delegate
app.run()
