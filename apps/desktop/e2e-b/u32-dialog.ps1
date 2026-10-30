param([Parameter(Mandatory=$true)][string]$Binary, [Parameter(Mandatory=$true)][string]$Title, [ValidateSet("accept","cancel")][string]$Action="cancel")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class U32Dialog { [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l); }
'@
$expected = (Resolve-Path $Binary).Path
for ($attempt = 0; $attempt -lt 100; $attempt++) {
  $apps = @(Get-Process | Where-Object { $_.Path -eq $expected })
  $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
  foreach ($window in $windows) {
    if ($window.Current.Name -eq $Title -and $apps.Id -contains $window.Current.ProcessId) {
      if ($Action -eq 'accept') {
        $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::AutomationIdProperty, '1')
        $button = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
        if ($null -eq $button) { throw 'Native dialog default button not found' }
        $pattern = $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $pattern.Invoke()
      } else {
      [void][U32Dialog]::PostMessage([IntPtr]$window.Current.NativeWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
      }
      '{"visible":true}'
      exit 0
    }
  }
  Start-Sleep -Milliseconds 100
}
throw 'U32 native file dialog not observed'
