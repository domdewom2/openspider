param(
    [Parameter(Mandatory = $true)][ValidateSet("activate", "inspect")][string]$Action,
    [Parameter(Mandatory = $true)][string]$Binary
)
$ErrorActionPreference = "Stop"
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class S02Focus {
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("user32.dll")] public static extern bool AllowSetForegroundWindow(uint pid);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr h, int command);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
}
'@
# Identify only the binary started by this run, never a matching window title.
$fullPath = [IO.Path]::GetFullPath($Binary)
$candidates = @(Get-Process | Where-Object {
    try { $_.Path -eq $fullPath -and $_.MainWindowHandle -ne [IntPtr]::Zero } catch { $false }
})
if ($candidates.Count -ne 1) { throw "Expected one app window for $fullPath; found $($candidates.Count)" }
$target = $candidates[0]
$window = $target.MainWindowHandle
$before = [S02Focus]::GetForegroundWindow()
$activation = @{}
if ($Action -eq "activate") {
    $show = if ([S02Focus]::IsIconic($window)) { 9 } else { 5 }
    $activation.showRequested = [S02Focus]::ShowWindowAsync($window, $show)
    $activation.allowed = [S02Focus]::AllowSetForegroundWindow([uint32]$target.Id)
    $currentThread = [S02Focus]::GetCurrentThreadId()
    [uint32]$foregroundPid = 0
    [uint32]$targetPid = 0
    $foregroundThread = [S02Focus]::GetWindowThreadProcessId($before, [ref]$foregroundPid)
    $targetThread = [S02Focus]::GetWindowThreadProcessId($window, [ref]$targetPid)
    $attached = @()
    try {
        foreach ($thread in (@($foregroundThread, $targetThread) | Select-Object -Unique)) {
            if ($thread -ne 0 -and $thread -ne $currentThread) {
                if ([S02Focus]::AttachThreadInput($currentThread, $thread, $true)) { $attached += $thread }
            }
        }
        $activation.attachedThreads = $attached
        $activation.requested = [S02Focus]::SetForegroundWindow($window)
    } finally {
        foreach ($thread in $attached) { [void][S02Focus]::AttachThreadInput($currentThread, $thread, $false) }
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(3)
    while ([S02Focus]::GetForegroundWindow() -ne $window -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 50
    }
}
[uint32]$frontPid = 0
$front = [S02Focus]::GetForegroundWindow()
[void][S02Focus]::GetWindowThreadProcessId($front, [ref]$frontPid)
@{
    pid = $target.Id; window = $window.ToInt64(); frontmostPid = $frontPid
    active = ($front -eq $window); visible = [S02Focus]::IsWindowVisible($window)
    minimized = [S02Focus]::IsIconic($window); activation = $activation
} | ConvertTo-Json -Depth 4 -Compress
