#if VER < EncodeVer(6, 3, 0)
  #error Inno Setup 6.3 or newer is required.
#endif

[Setup]
MinVersion=10.0
AppId={{FC411E6E-89E0-4438-9D3E-2C7B2500742B}
AppName=Stillnote
AppVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Stillnote
DefaultGroupName=Stillnote
PrivilegesRequired=lowest
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseFilename}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#AppName}.exe
#if AppArch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif

[Files]
Source: "{#BundleDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Stillnote"; Filename: "{app}\{#AppName}.exe"
