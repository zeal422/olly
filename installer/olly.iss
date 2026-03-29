#define MyAppName "Olly"
#define MyAppVersion GetEnv("OLLY_APP_VERSION")
#define MyAppPublisher "Olly Contributors"
#define MyAppExeName "olly.exe"
#define MySourceDir GetEnv("OLLY_SOURCE_DIR")
#define MyOutputDir GetEnv("OLLY_OUTPUT_DIR")
#define MyIconPath GetEnv("OLLY_ICON_PATH")

#if MyAppVersion == ""
  #error "OLLY_APP_VERSION is required"
#endif
#if MySourceDir == ""
  #error "OLLY_SOURCE_DIR is required"
#endif
#if MyOutputDir == ""
  #error "OLLY_OUTPUT_DIR is required"
#endif
#if MyIconPath == ""
  #error "OLLY_ICON_PATH is required"
#endif

[Setup]
AppId={{D36F87A8-5B72-4FA3-9836-EF4480768E22}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={localappdata}\Programs\Olly
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
LicenseFile={#MySourceDir}\LICENSE
OutputDir={#MyOutputDir}
OutputBaseFilename=Olly-Setup-{#MyAppVersion}
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
SetupIconFile={#MyIconPath}
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#MySourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
