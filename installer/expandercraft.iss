; Inno Setup script of the ExpanderCraft installer (Inno Setup 6.3 or later).
;
; Build it with installer\build.ps1, which reads the version from Cargo.toml. By hand:
;   ISCC.exe /DAppVersion=0.5.1 installer\expandercraft.iss
; Optional defines: /DExeDir=<folder of expandercraft.exe> (default: target\release) and
; /DIconFile=<.ico file> (the icon of the setup program, default: the Inno Setup one).
;
; The installer is per-user by default (no administrator rights, no UAC prompt): the program lives in
; %LOCALAPPDATA%\Programs\ExpanderCraft. The dialog at the start lets the user install for all users instead.
; The data of the user (%LOCALAPPDATA%\ExpanderCraft: combos, settings, backups) is never touched, not even by
; the uninstaller.

#ifndef AppVersion
  #error AppVersion is not defined: pass /DAppVersion=x.y.z (installer\build.ps1 reads it from Cargo.toml)
#endif
#ifndef ExeDir
  #define ExeDir "..\target\release"
#endif

#define AppName "ExpanderCraft"
#define AppExe "expandercraft.exe"
#define AppUrl "https://github.com/echidistanti/expandercraft"

[Setup]
; Never change the AppId: it is what makes a new version replace the previous one.
AppId={{9F76FD54-68FA-4B68-998C-BDA0B4BE0160}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=echidistanti
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
VersionInfoVersion={#AppVersion}.0
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} setup

DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0

; ExpanderCraft keeps this mutex while it runs, in the tray too: Setup and the uninstaller ask the user to quit
; it instead of trying to replace a program in use. Same name as MUTEX_NAME in src/platform/windows.rs.
AppMutex=Local\ExpanderCraft.SingleInstance.6f1c2a4e
SetupMutex=ExpanderCraftSetup

UninstallDisplayIcon={app}\{#AppExe}
OutputDir=Output
OutputBaseFilename=ExpanderCraft-{#AppVersion}-windows-x64-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
#ifdef IconFile
SetupIconFile={#IconFile}
#endif

; The same languages as the program. Setup picks the one of Windows, English when it is not among these.
[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#ExeDir}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; "Launch ExpanderCraft at login" (a preference of the program) writes this value: remove it on uninstall,
; so that Windows does not look for a program that is gone.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "{#AppName}"; ValueType: none; Flags: dontcreatekey uninsdeletevalue

[Run]
; runasoriginaluser: with an all-users install Setup is elevated, but the program has to run with the rights of
; the user (an elevated process would only be able to type into the windows of other elevated programs).
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent runasoriginaluser
