; DGMOS Drive Windows kurulumu (Inno Setup 6).
; Kullanıcı bazında kurar: yönetici şifresi istemez, güncellemeler UAC'siz kurulur.
; CI'da: ISCC /DAppVersion=1.2.3 /DSourceExe=...\dgmos-drive.exe /DOutputDir=dist scripts\windows-installer.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\target\release\dgmos-drive.exe"
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif

#define AppName "DGMOS Drive"
#define AppExe "DGMOS Drive.exe"

[Setup]
; AppId değişmemeli: yeni sürüm eskisinin üzerine kurulur.
AppId={{9BF0B252-2E94-4504-BEB3-BB829E2A16ED}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=DG Regtech Co.
AppPublisherURL=https://go.dgmos.com
AppSupportURL=https://github.com/dgmosdev/drive
VersionInfoVersion={#AppVersion}
PrivilegesRequired=lowest
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=yes
OutputDir={#OutputDir}
OutputBaseFilename=DGMOS-Drive-Setup
SetupIconFile=..\assets\app-icon.ico
UninstallDisplayIcon={app}\app-icon.ico
UninstallDisplayName={#AppName}
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Açık uygulamayı kapatıp günceller.
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "{#AppExe}"; Flags: ignoreversion
Source: "..\assets\app-icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; IconFilename: "{app}\app-icon.ico"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; IconFilename: "{app}\app-icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
