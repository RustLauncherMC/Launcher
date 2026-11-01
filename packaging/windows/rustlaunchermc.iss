; RustLauncherMC's Windows installer (Inno Setup 6).
; Build: iscc /DVersion=0.1.0 packaging\windows\rustlaunchermc.iss
; It installs for the current user only, into %LOCALAPPDATA%\Programs\RustLauncherMC: no
; administrator approval is needed, and the launcher can update itself there.

#ifndef Version
  #define Version "0.0.0"
#endif

[Setup]
AppId={{34285BDE-2CB6-4903-A903-E493F296794B}
AppName=RustLauncherMC
AppVersion={#Version}
AppVerName=RustLauncherMC {#Version}
AppPublisher=RustLauncherMC (fan project)
AppPublisherURL=https://github.com/RustLauncherMC/Launcher
AppSupportURL=https://github.com/RustLauncherMC/Launcher/issues
AppUpdatesURL=https://github.com/RustLauncherMC/Launcher/releases
DefaultDirName={localappdata}\Programs\RustLauncherMC
DisableDirPage=auto
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=RustLauncherMC-Setup-x64
SetupIconFile=..\..\assets\logo.ico
UninstallDisplayIcon={app}\rustlauncher.exe
UninstallDisplayName=RustLauncherMC
LicenseFile=..\..\LICENSE
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\target\release\rustlauncher.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\..\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\RustLauncherMC"; Filename: "{app}\rustlauncher.exe"
Name: "{autodesktop}\RustLauncherMC"; Filename: "{app}\rustlauncher.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\rustlauncher.exe"; Description: "{cm:LaunchProgram,RustLauncherMC}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Executables a self-update moved aside. Settings, accounts and game files stay.
Type: files; Name: "{app}\rustlauncher.exe.old-*"
