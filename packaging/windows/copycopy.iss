; Windows installer, built by Inno Setup 6 from the release workflow:
;
;   ISCC.exe /DAppVersion=0.1.0 packaging\windows\copycopy.iss
;
; Per-user by design: PrivilegesRequired=lowest installs into
; %LOCALAPPDATA%\Programs and asks for no administrator. A clipboard utility
; has no business demanding elevation.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "copycopy"
#define AppExe "copycopy.exe"

[Setup]
AppId={{8A73CE75-C256-448D-B33D-EF5BEDE3DBA4}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Théo Delmas
AppSupportURL=https://github.com/USER/copycopy
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\..\LICENSE
SetupIconFile=copycopy.ico
UninstallDisplayIcon={app}\{#AppExe}
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir=..\..\dist
OutputBaseFilename=copycopy-windows-x86_64
; The resident holds its own executable open, so the Restart Manager is asked
; to close it rather than letting the copy fail halfway.
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked
Name: "autostart"; Description: "Lancer copycopy à l'ouverture de session"

[Files]
Source: "..\..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
; The application writes its own autostart entry, so the installer asks it to
; rather than duplicating the registry work.
Filename: "{app}\{#AppExe}"; Parameters: "--autostart on"; Tasks: autostart; Flags: runhidden
Filename: "{app}\{#AppExe}"; Description: "Démarrer copycopy"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Stop the resident and undo the autostart entry before the files go: an
; uninstall that leaves a process running and a login entry behind is not one.
Filename: "{app}\{#AppExe}"; Parameters: "--autostart off"; RunOnceId: "autostartoff"; Flags: runhidden
Filename: "{app}\{#AppExe}"; Parameters: "--quit"; RunOnceId: "stopresident"; Flags: runhidden
