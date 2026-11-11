#ifndef AppVersion
  #error AppVersion must be provided
#endif
#ifndef PayloadDir
  #error PayloadDir must be provided
#endif
#ifndef OutputDir
  #error OutputDir must be provided
#endif
[Setup]
AppId={{6E542AC8-09FB-4613-9CA3-032F0B5EEAF1}
AppName=Gavnel
AppVersion={#AppVersion}
AppPublisher=Software by Brox
AppPublisherURL=https://github.com/Nuqb/gavnel
AppSupportURL=https://github.com/Nuqb/gavnel/issues
AppUpdatesURL=https://github.com/Nuqb/gavnel/releases
DefaultDirName={localappdata}\Programs\Gavnel
DefaultGroupName=Gavnel
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19045
AppMutex=Local\Gavnel.DesktopApp
CloseApplications=no
RestartApplications=no
RestartIfNeededByRun=no
UninstallDisplayIcon={app}\Gavnel.exe
UninstallDisplayName=Gavnel
OutputDir={#OutputDir}
OutputBaseFilename=Gavnel-Setup-{#AppVersion}-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile={#PayloadDir}\LICENSE
SetupIconFile=..\assets\gavnel.ico
[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked
[Files]
Source: "{#PayloadDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{group}\Gavnel"; Filename: "{app}\Gavnel.exe"; WorkingDir: "{app}"; Check: CreateShortcuts
Name: "{autodesktop}\Gavnel"; Filename: "{app}\Gavnel.exe"; WorkingDir: "{app}"; Tasks: desktopicon; Check: CreateShortcuts
[Run]
Filename: "{app}\Gavnel.exe"; Description: "Launch Gavnel"; Flags: nowait postinstall skipifsilent; Check: LaunchAfterSetup
[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Gavnel"; Flags: uninsdeletevalue; Check: ManageStartupKey
[Code]
function CreateShortcuts: Boolean;
begin
  Result := ExpandConstant('{param:NOSHORTCUTS|0}') <> '1';
end;
function LaunchAfterSetup: Boolean;
begin
  Result := ExpandConstant('{param:NOLAUNCH|0}') <> '1';
end;

function ManageStartupKey: Boolean;
begin
  Result := ExpandConstant('{param:NOREGISTRY|0}') <> '1';
end;
