; Inno Setup script for MW5 Remap.
; Per-user install (no admin) so the in-app auto-updater can replace the exe freely.
; Build with:  ISCC.exe installer.iss   (output: dist\MW5-Remap-Setup.exe)

#define MyAppName "MW5 Remap"
#define MyAppVersion "0.3.0"
#define MyAppExe "MW5-Remap.exe"

[Setup]
AppId={{8F2A6C31-9B4D-4E27-AA10-3C7E9D2B5F08}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=Falkoro
DefaultDirName={localappdata}\Programs\MW5-Remap
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=dist
OutputBaseFilename=MW5-Remap-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExe}
UninstallDisplayName={#MyAppName}

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional icons:"

[Files]
Source: "MW5-Remap.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "libunwind.dll"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
