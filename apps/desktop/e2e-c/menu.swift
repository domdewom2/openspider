// Native AX menu traversal; requires an unlocked macOS GUI and Accessibility permission.
import AppKit
import ApplicationServices
import Foundation
func attribute(_ element: AXUIElement, _ name: String) -> AnyObject? {
    var result: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &result) == .success else { return nil }
    return result
}
func children(_ element: AXUIElement) -> [AXUIElement] {
    return attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? []
}
func title(_ element: AXUIElement) -> String { attribute(element, kAXTitleAttribute) as? String ?? "" }
func tree(_ element: AXUIElement, _ depth: Int = 0) -> [String: Any] {
    var result: [String: Any] = ["title": title(element), "role": attribute(element, kAXRoleAttribute) as? String ?? "", "enabled": attribute(element, kAXEnabledAttribute) as? Bool ?? false]
    if depth < 8 { result["children"] = children(element).map { tree($0, depth + 1) } }
    return result
}
func find(_ element: AXUIElement, _ wanted: String, _ depth: Int = 0) -> AXUIElement? {
    if title(element) == wanted { return element }
    if depth >= 8 { return nil }
    for child in children(element) { if let match = find(child, wanted, depth + 1) { return match } }
    return nil
}
guard CommandLine.arguments.count >= 2, let pid = Int32(CommandLine.arguments[1]), AXIsProcessTrusted() else {
    fputs("U32: PID and Accessibility permission required\n", stderr); exit(1)
}
let app = AXUIElementCreateApplication(pid)
guard let raw = attribute(app, kAXMenuBarAttribute) else { fputs("U32: native menu bar missing\n", stderr); exit(1) }
let bar = raw as! AXUIElement
if CommandLine.arguments.count > 2 {
    guard let item = find(bar, CommandLine.arguments[2]), AXUIElementPerformAction(item, kAXPressAction as CFString) == .success else {
        fputs("U32: native menu item could not be pressed\n", stderr); exit(1)
    }
}
let data = try JSONSerialization.data(withJSONObject: tree(bar), options: [.sortedKeys, .prettyPrinted])
FileHandle.standardOutput.write(data)
