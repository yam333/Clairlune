// Loads Clairlune.saver the way the screen saver host does (its principal
// class, initWithFrame:isPreview:), plays it full screen for a few seconds
// and a preview-sized one beside it, then takes a screenshot.
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
DispatchQueue.main.asyncAfter(deadline: .now() + 5) {
    let shot = Process()
    shot.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    shot.arguments = ["-x", args[2]]
    try? shot.run()
    shot.waitUntilExit()
    timer.invalidate()
    full.stopAnimation()
    preview?.stopAnimation()
    print("played \(frames) frames; screenshot \(args[2])")
    exit(0)
}
app.run()
