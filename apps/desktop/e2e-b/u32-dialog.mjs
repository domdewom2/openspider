// @ts-check
// OS observation must succeed before Escape is sent; never substitute a browser file chooser.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
const [binary, title, action = "cancel", pid] = process.argv.slice(2);
assert.ok(binary && title);
if (process.platform === "darwin") {
  const script = `on run argv
set wanted to item 1 of argv
set acceptDialog to item 2 of argv is "accept"
set targetPID to (item 3 of argv) as integer
tell application "System Events"
repeat 100 times
repeat with p in (application processes whose unix id is targetPID)
repeat with w in windows of p
if name of w contains wanted then
set frontmost of p to true
if acceptDialog then
key code 36
else
key code 53
end if
return "{\\"visible\\":true}"
end if
repeat with s in sheets of w
if name of s contains wanted or description of s contains "dialog" then
set frontmost of p to true
if acceptDialog then
key code 36
else
key code 53
end if
return "{\\"visible\\":true}"
end if
end repeat
end repeat
end repeat
delay 0.1
end repeat
error "U32 native dialog not observed"
end tell
end run`;
  process.stdout.write(execFileSync("osascript", ["-e", script, title, action, pid ?? ""], { encoding: "utf8", timeout: 20_000 }));
} else if (process.platform === "win32") {
  process.stdout.write(execFileSync("powershell.exe", ["-NoProfile", "-File", fileURLToPath(new URL("./u32-dialog.ps1", import.meta.url)), "-Binary", binary, "-Title", title, "-Action", action], { encoding: "utf8", timeout: 20_000 }));
} else throw new Error("Native file dialogs require macOS or Windows");
