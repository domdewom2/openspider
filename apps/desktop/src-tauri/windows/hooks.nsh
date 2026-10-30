!include "LogicLib.nsh"
!include "FileFunc.nsh"

!macro NSIS_HOOK_POSTINSTALL
  StrCpy $0 "1"
  ExecWait '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\installer-maintenance.ps1" -Mode Install -InstallDir "$INSTDIR"' $0
  ${If} $0 != 0
    SetErrorLevel 1
    Abort "PATH konnte nicht eingerichtet werden."
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Tauri's updater uses /UPDATE: keep registrations and user files during replacement.
  ${If} $UpdateMode != 1
    StrCpy $1 "0"
    ${GetOptions} $CMDLINE "/REMOVEUSERDATA=" $1
    IfSilent r02_cleanup
    MessageBox MB_YESNO|MB_DEFBUTTON2 "Benutzerdaten entfernen? Crawls und Einstellungen werden dauerhaft gelöscht." IDNO r02_cleanup
    StrCpy $1 "1"
    r02_cleanup:
    StrCpy $0 "1"
    ExecWait '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\installer-maintenance.ps1" -Mode Uninstall -InstallDir "$INSTDIR" -RemoveUserData "$1"' $0
    ${If} $0 != 0
      SetErrorLevel 1
      Abort "Deinstallationsbereinigung fehlgeschlagen."
    ${EndIf}
  ${EndIf}
!macroend
