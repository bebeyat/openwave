; OpenWave installer (Inno Setup 6). Per-user install, no administrator rights needed.
;
; Build locally (after `cargo build --release` and staging the files, see .github/workflows/release.yml):
;   ISCC.exe /DAppVersion=0.1.0 /DStageDir=C:\path\to\stage\OpenWave /DOutDir=C:\path\to\dist installer\openwave.iss
; StageDir must contain openwave.exe, README.md, LICENSE-MIT, LICENSE-APACHE, THIRD_PARTY_NOTICES.md.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef StageDir
  #define StageDir "..\target\release"
#endif
#ifndef OutDir
  #define OutDir "..\dist"
#endif

#define AppName "OpenWave"
#define AppExe "openwave.exe"
#define AppUrl "https://github.com/bebeyat/openwave"
#define WebView2Link "https://go.microsoft.com/fwlink/p/?LinkId=2124703"

[Setup]
; Unique id of this application. Never change it between releases (it links upgrades to uninstall).
AppId={{3F422F78-3A3E-418D-A445-88D3787B72A1}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=meganek31
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutDir}
OutputBaseFilename=OpenWave-{#AppVersion}-windows-x64-setup
SetupIconFile=..\assets\branding\openwave.ico
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile=..\LICENSE-MIT
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#StageDir}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; openwave:// links: "Open in OpenWave" on a share page starts the app with the link.
Root: HKA; Subkey: "Software\Classes\openwave"; ValueType: string; ValueName: ""; ValueData: "URL:OpenWave"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\openwave"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\openwave\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"",0"
Root: HKA; Subkey: "Software\Classes\openwave\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Only what the app or WebView2 may create next to the executable: never "{app}" itself recursively, because
; the user may have picked an existing folder holding other files. User data lives in %APPDATA%\OpenWave and is
; handled (with a question) in the [Code] section below.
Type: filesandordirs; Name: "{app}\openwave.exe.WebView2"
Type: files; Name: "{app}\*.log"
Type: dirifempty; Name: "{app}"

[Code]
const
  WebView2ClientKey = 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';
  WebView2ClientKeyWow = 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

function WebView2Version(): String;
var
  v: String;
begin
  v := '';
  if not RegQueryStringValue(HKLM, WebView2ClientKeyWow, 'pv', v) then
    if not RegQueryStringValue(HKCU, WebView2ClientKey, 'pv', v) then
      if not RegQueryStringValue(HKLM, WebView2ClientKey, 'pv', v) then
        v := '';
  Result := v;
end;

function IsWebView2Installed(): Boolean;
var
  v: String;
begin
  v := WebView2Version();
  Result := (v <> '') and (v <> '0.0.0.0');
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ErrorCode: Integer;
begin
  if (CurStep = ssPostInstall) and (not IsWebView2Installed()) then
  begin
    if MsgBox('OpenWave needs the Microsoft Edge WebView2 Runtime, which was not found on this PC.' + #13#10 +
              'It is included with current versions of Windows 10 and 11, but yours seems to lack it.' + #13#10#13#10 +
              'Open the Microsoft download page for the small installer now?',
              mbConfirmation, MB_YESNO) = IDYES then
      ShellExec('open', '{#WebView2Link}', '', '', SW_SHOWNORMAL, ewNoWait, ErrorCode);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    DataDir := ExpandConstant('{userappdata}\OpenWave');
    if DirExists(DataDir) then
    begin
      { Default answer is "No": keep the user's library and settings. Unattended uninstalls always keep them. }
      if (not UninstallSilent) and
         (MsgBox('Do you also want to delete your OpenWave data (library, playlists, settings, cache)?' + #13#10 +
                 DataDir + #13#10#13#10 +
                 'Choose No to keep it, for example to reinstall later.',
                 mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES) then
        DelTree(DataDir, True, True, True);
    end;
  end;
end;
