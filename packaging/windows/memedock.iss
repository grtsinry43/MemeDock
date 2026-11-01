; Per-user installer for the unpackaged, self-contained WinUI build.
; CI supplies the version and the application output directory through the
; environment so Windows paths containing spaces never pass through /D parsing.

#define AppVersion GetEnv("MEMEDOCK_WINDOWS_VERSION")
#define AppSourceDir GetEnv("MEMEDOCK_WINDOWS_SOURCE")
#if AppVersion == ""
#error MEMEDOCK_WINDOWS_VERSION is required
#endif
#if AppSourceDir == ""
#error MEMEDOCK_WINDOWS_SOURCE is required
#endif

[Setup]
AppId={{6F238518-5456-44A8-B35D-15D423F12395}
AppName=MemeDock
AppVersion={#AppVersion}
AppVerName=MemeDock {#AppVersion}
AppPublisher=grtsinry43
AppPublisherURL=https://github.com/grtsinry43/MemeDock
AppSupportURL=https://github.com/grtsinry43/MemeDock/issues
AppUpdatesURL=https://github.com/grtsinry43/MemeDock/releases
DefaultDirName={localappdata}\Programs\MemeDock
DefaultGroupName=MemeDock
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
MinVersion=10.0.17763
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
Compression=lzma2/ultra64
SolidCompression=yes
SetupIconFile=..\..\apps\windows\app\Assets\MemeDock.ico
OutputDir=..\..\target\distribution\windows
OutputBaseFilename=MemeDock-{#AppVersion}-windows-x64-setup
UninstallDisplayName=MemeDock
UninstallDisplayIcon={app}\MemeDock.Windows.exe
VersionInfoVersion={#AppVersion}
VersionInfoProductName=MemeDock
VersionInfoCompany=grtsinry43
VersionInfoDescription=MemeDock installer
CloseApplications=yes
RestartApplications=no

[Files]
Source: "{#AppSourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs; Excludes: "*.pdb"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Icons]
Name: "{group}\MemeDock"; Filename: "{app}\MemeDock.Windows.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\MemeDock"; Filename: "{app}\MemeDock.Windows.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\MemeDock.Windows.exe"; Description: "{cm:LaunchProgram,MemeDock}"; Flags: nowait postinstall skipifsilent
