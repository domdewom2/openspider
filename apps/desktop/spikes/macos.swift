// Native measurement helper. No Accessibility prompt and no changes to the product.
import AppKit
import CoreGraphics
import ApplicationServices

let args = CommandLine.arguments
guard args.count >= 3, let pid = Int32(args[2]) else {
    fatalError("Usage: macos <inspect|activate|key|restore|cover> <pid> [key]")
}
let app = NSRunningApplication(processIdentifier: pid)
var activation: [String: Any] = [:]
func pause(_ seconds: Double) {
    RunLoop.current.run(until: Date().addingTimeInterval(seconds))
}
func activate() {
    let session = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
    guard session["CGSSessionScreenIsLocked"] as? Bool != true else {
        activation["blocked"] = "screenLocked"
        return
    }
    app?.unhide()
    // A helper cannot yield Terminal's activation. Only yield if this process
    // actually owns it; otherwise use the target's AX window / native click.
    if #available(macOS 14.0, *), NSRunningApplication.current.isActive, let app {
        NSApplication.shared.yieldActivation(to: app)
        activation["yielded"] = true
    }
    activation["requested"] = app?.activate(options: [.activateAllWindows]) ?? false
    if AXIsProcessTrusted() {
        let target = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(target, 1)
        var value: CFTypeRef?
        if AXUIElementCopyAttributeValue(target, kAXWindowsAttribute as CFString, &value) == .success,
           let windows = value as? [AXUIElement], let window = windows.first {
            activation["axMain"] = AXUIElementSetAttributeValue(window, kAXMainAttribute as CFString, kCFBooleanTrue).rawValue
            activation["axRaise"] = AXUIElementPerformAction(window, kAXRaiseAction as CFString).rawValue
            activation["axFrontmost"] = AXUIElementSetAttributeValue(target, kAXFrontmostAttribute as CFString, kCFBooleanTrue).rawValue
        }
    }
    pause(0.3)
    if app?.isActive != true && CGPreflightPostEventAccess(),
       let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]],
       let window = windows.first(where: {
           ($0[kCGWindowOwnerPID as String] as? Int32) == pid &&
           ($0[kCGWindowLayer as String] as? Int) == 0
       }),
       let bounds = window[kCGWindowBounds as String] as? NSDictionary,
       let frame = CGRect(dictionaryRepresentation: bounds), frame.width > 100, frame.height > 40 {
        // Title-bar center avoids window controls and leaves fixture inputs alone.
        let point = CGPoint(x: frame.midX, y: frame.minY + 12)
        // CGWindowList is front-to-back: do not click a terminal/overlay covering
        // the target title bar if AXRaise was denied.
        let hit = windows.first(where: {
            guard ($0[kCGWindowAlpha as String] as? Double ?? 1) > 0,
                  let bounds = $0[kCGWindowBounds as String] as? NSDictionary,
                  let rect = CGRect(dictionaryRepresentation: bounds) else { return false }
            return rect.contains(point)
        })
        if (hit?[kCGWindowOwnerPID as String] as? Int32) == pid {
            activation["titlebarClick"] = ["x": point.x, "y": point.y]
            for type in [CGEventType.leftMouseDown, .leftMouseUp] {
                CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: .left)?.post(tap: .cghidEventTap)
            }
        } else {
            activation["clickBlocked"] = "target title bar is occluded"
        }
    }
    let deadline = Date().addingTimeInterval(3)
    while app?.isActive != true && Date() < deadline { pause(0.05) }
}
func report() {
    let session = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
    let value: [String: Any] = [
        "pid": pid, "active": app?.isActive ?? false, "hidden": app?.isHidden ?? true,
        "activationPolicy": app?.activationPolicy.rawValue ?? -1,
        "postEventAccess": CGPreflightPostEventAccess(),
        "accessibilityTrusted": AXIsProcessTrusted(),
        "frontmostPid": NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1,
        "helperActive": NSRunningApplication.current.isActive,
        "activation": activation,
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
    activate()
    report()
case "key":
    // Native events must traverse the NSMenu; the embedded WebDriver's JS key events cannot prove S-3.
    guard CGPreflightPostEventAccess() else { fputs("Native key injection unavailable (PostEvent access)\n", stderr); exit(2) }
    guard app?.isActive == true, NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
        fputs("Refusing native key injection: target app is not frontmost\n", stderr); exit(3)
    }
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
