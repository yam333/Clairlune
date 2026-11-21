// Loads Clairlune.saver the way the screen saver host does (its principal
// class, initWithFrame:isPreview:), plays it full screen for a few seconds
// and a preview-sized one beside it, then takes two screenshots a second and
// a half apart: <out.png> and <out>-later.png. They must differ (the scene
// moves), or it exits with 5.
//   swift scripts/saver_check.swift <path/Clairlune.saver> <out.png>
import AppKit
import ScreenSaver

let args = CommandLine.arguments
guard args.count == 3 else {
    print("usage: saver_check <Clairlune.saver> <out.png>")
    exit(64)
}
guard let bundle = Bundle(path: args[1]), bundle.load() else {
    print("couldn't load \(args[1])")
    exit(1)
}
guard let cls = bundle.principalClass as? ScreenSaverView.Type else {
    print("the principal class isn't a ScreenSaverView: \(String(describing: bundle.principalClass))")
    exit(2)
}
print("principal class: \(cls)")

let app = NSApplication.shared
app.setActivationPolicy(.regular)
guard let screen = NSScreen.main else {
    print("no screen")
    exit(3)
}

func show(_ frame: NSRect, preview: Bool, level: NSWindow.Level) -> ScreenSaverView? {
    let window = NSWindow(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
    window.level = level
    guard let view = cls.init(frame: NSRect(origin: .zero, size: frame.size), isPreview: preview) else {
        print("init failed (preview: \(preview))")
        return nil
    }
    window.contentView = view
    window.makeKeyAndOrderFront(nil)
    view.startAnimation()
    print("started: preview \(preview), \(Int(frame.width))x\(Int(frame.height)) points, interval \(view.animationTimeInterval)")
    return view
}

guard let full = show(screen.frame, preview: false, level: .screenSaver) else { exit(4) }
let small = NSRect(x: 40, y: 40, width: 320, height: 200)
let preview = show(small, preview: true, level: NSWindow.Level(rawValue: NSWindow.Level.screenSaver.rawValue + 1))
app.activate(ignoringOtherApps: true)

// The host drives frames; here a timer does.
var frames = 0
let timer = Timer.scheduledTimer(withTimeInterval: full.animationTimeInterval, repeats: true) { _ in
    full.animateOneFrame()
    preview?.animateOneFrame()
    frames += 1
}
func capture(_ path: String) {
    let shot = Process()
    shot.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    shot.arguments = ["-x", path]
    try? shot.run()
    shot.waitUntilExit()
}

/// How many bytes of two screenshots differ.
func difference(_ a: String, _ b: String) -> Int {
    guard let ra = NSBitmapImageRep(data: (try? Data(contentsOf: URL(fileURLWithPath: a))) ?? Data()),
          let rb = NSBitmapImageRep(data: (try? Data(contentsOf: URL(fileURLWithPath: b))) ?? Data()),
          let pa = ra.bitmapData, let pb = rb.bitmapData,
          ra.bytesPerRow == rb.bytesPerRow, ra.pixelsHigh == rb.pixelsHigh else {
        return -1
    }
    var n = 0
    for i in 0..<(ra.bytesPerRow * ra.pixelsHigh) where pa[i] != pb[i] {
        n += 1
    }
    return n
}

let later = (args[2] as NSString).deletingPathExtension + "-later.png"
DispatchQueue.main.asyncAfter(deadline: .now() + 4) {
    capture(args[2])
    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
        capture(later)
        timer.invalidate()
        full.stopAnimation()
        preview?.stopAnimation()
        let changed = difference(args[2], later)
        print("played \(frames) frames; screenshots \(args[2]), \(later); \(changed) bytes changed between them")
        exit(changed > 0 ? 0 : 5)
    }
}
app.run()
