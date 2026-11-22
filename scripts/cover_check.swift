// For CI (.github/workflows/check-mac-cover.yml): opens an ordinary window the
// size a zoomed window takes (the main screen without the menu bar and the
// Dock) for 10 s, then shrinks it to 60% for 10 s, then quits. Clairlune,
// playing live meanwhile, should pause and then play again.
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

DispatchQueue.main.asyncAfter(deadline: .now() + 10) {
    let small = NSRect(x: full.minX + 200, y: full.minY + 150, width: full.width * 0.6, height: full.height * 0.6)
    window.setFrame(small, display: true)
    print("smaller window: \(window.frame)")
    DispatchQueue.main.asyncAfter(deadline: .now() + 10) {
        print("done")
        exit(0)
    }
}
app.run()
