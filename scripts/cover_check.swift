// For CI (.github/workflows/check-mac-cover.yml): opens an ordinary window the
// size a zoomed window takes (the main screen without the menu bar and the
// Dock) for 10 s, then a little smaller all round for 10 s, then shrinks it to
// 60% for 10 s, then quits. Clairlune, playing live meanwhile, should pause and
// then play again, and make the menu bar solid for the first two.
//   swift scripts/cover_check.swift
import AppKit

let app = NSApplication.shared
app.setActivationPolicy(.regular)
guard let screen = NSScreen.screens.first else {
    print("no screen")
    exit(3)
}
let full = screen.visibleFrame
let window = NSWindow(contentRect: full, styleMask: [.titled, .resizable, .closable], backing: .buffered, defer: false)
window.title = "Cover check"
window.setFrame(full, display: true)
window.makeKeyAndOrderFront(nil)
app.activate(ignoringOtherApps: true)
print("zoomed-size window: \(window.frame)")

func after(_ seconds: Double, _ run: @escaping () -> Void) {
    DispatchQueue.main.asyncAfter(deadline: .now() + seconds, execute: run)
}
after(10) {
    window.setFrame(full.insetBy(dx: 20, dy: 16), display: true)
    print("almost as big: \(window.frame)")
}
after(20) {
    window.setFrame(NSRect(x: full.minX + 200, y: full.minY + 150, width: full.width * 0.6, height: full.height * 0.6), display: true)
    print("smaller window: \(window.frame)")
}
after(30) {
    print("done")
    exit(0)
}
app.run()
