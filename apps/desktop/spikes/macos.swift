// Native measurement helper. No Accessibility prompt and no changes to the product.
import AppKit
import CoreGraphics

let args = CommandLine.arguments
guard args.count >= 3, let pid = Int32(args[2]) else {
    fatalError("Usage: macos <inspect|activate|key|restore|cover> <pid> [key]")
}
let app = NSRunningApplication(processIdentifier: pid)
func report() {
    let session = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
    let value: [String: Any] = [
        "pid": pid, "active": app?.isActive ?? false, "hidden": app?.isHidden ?? true,
        "activationPolicy": app?.activationPolicy.rawValue ?? -1,
        "postEventAccess": CGPreflightPostEventAccess(),
        "session": [
            "screenLocked": session["CGSSessionScreenIsLocked"] ?? false,
            "onConsole": session["kCGSessionOnConsoleKey"] ?? false,
            "loginDone": session["kCGSessionLoginDoneKey"] ?? false
        ]
    ]
    print(String(data: try! JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]), encoding: .utf8)!)
}
switch args[1] {
case "inspect": report()
case "activate", "restore":
    app?.unhide()
    _ = app?.activate(options: [.activateAllWindows])
    RunLoop.current.run(until: Date().addingTimeInterval(0.5))
    report()
case "key":
    // Native events must traverse the NSMenu; the embedded WebDriver's JS key events cannot prove S-3.
    guard CGPreflightPostEventAccess() else { fputs("Native key injection unavailable (PostEvent access)\n", stderr); exit(2) }
    let keys: [String: CGKeyCode] = ["a": 0, "c": 8, "v": 9, "x": 7, "z": 6]
    guard args.count == 4, let code = keys[args[3]] else { exit(2) }
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)!
        event.flags = .maskCommand
        event.postToPid(pid)
    }
case "cover":
    // A separate opaque native window covers the app without minimizing or hiding it.
    let application = NSApplication.shared
    application.setActivationPolicy(.regular)
    let frame = NSScreen.screens.reduce(NSRect.zero) { $0.union($1.frame) }
    let window = NSWindow(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
    window.backgroundColor = .black
    window.isOpaque = true
    window.level = .floating
    window.makeKeyAndOrderFront(nil)
    application.activate(ignoringOtherApps: true)
    application.run()
default: exit(2)
}
