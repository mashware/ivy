; Build with Inno Setup 6.4 or later; all paths are supplied by package-windows.py.
[Setup]
AppId={{76C75D36-98A7-4E49-B1BB-CDA73E068F76}
AppName=Ivy
AppVersion={#ReleaseVersion}
AppPublisher=Mashware
AppPublisherURL=https://github.com/mashware/ivy
DefaultDirName={localappdata}\Programs\Ivy
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.22000
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\Ivy.exe
SetupIconFile={#PayloadDir}\Ivy.ico
LicenseFile={#PayloadDir}\licenses\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename=Ivy-{#ReleaseVersion}-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
WizardStyle=modern

[Files]
Source: "{#PayloadDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\Ivy"; Filename: "{app}\Ivy.exe"; IconFilename: "{app}\Ivy.ico"

[Run]
Filename: "{app}\Ivy.exe"; Description: "Open Ivy"; Flags: nowait postinstall skipifsilent
