; Swoopdl's Windows installer (Inno Setup 6). Built by tools/installer/build.ps1,
; which passes /DAppVersion=... and, when a certificate is set up, /DSign with /Ssdlsign=...
;
; Install for the current user (no administrator rights, %LOCALAPPDATA%\Programs) or for
; everyone (Program Files); the wizard asks. Tasks: browser integration (native messaging host
; for Chrome, Edge, Brave and Firefox), .torrent files and magnet links, start with Windows, a
; desktop shortcut. A running SDL is asked to save and quit first (/exit). Uninstalling asks
; whether to keep the download list and settings; removing them also removes SDL's sign-ins from
; Windows Credential Manager (swoopdl --forget-sign-ins).

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#define AppName "Swoopdl"
#define AppExe "swoopdl.exe"
#define HostExe "swoopdl-nmhost.exe"
#define Root "..\.."
#define Bin Root + "\target\release"

[Setup]
; Never change AppId: upgrades and the uninstaller find the installation by it.
AppId={{667DB4D8-32C2-4487-A0E5-5825BF29778C}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
; Swoopdl-owned links are supplied by explicit ISCC /D defines.
; Keep original author copyright attribution; do not claim upstream URLs are Swoopdl-owned.
#ifdef SDLPublisherURL
AppPublisherURL={#SDLPublisherURL}
#endif
#ifdef SDLSupportURL
AppSupportURL={#SDLSupportURL}
#endif
#ifdef SDLUpdatesURL
AppUpdatesURL={#SDLUpdatesURL}
#endif
AppCopyright=Copyright (C) 2026 Ahmed Nefzaoui
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
; The Run entry is the installing user's, also when installing for everyone.
UsedUserAreasWarning=no
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
WizardStyle=modern
SetupIconFile={#Root}\crates\app\assets\icons\app\sdl.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir={#Root}\target\dist
OutputBaseFilename=Swoopdl-{#AppVersion}-setup
; A 64 MB dictionary (#25): an 8 MB one (lzma2/max) only finds what repeats close by in the
; solid archive. A few MB smaller; building needs about 750 MB of memory, installing 64 MB.
Compression=lzma2/ultra64
SolidCompression=yes
ChangesAssociations=yes
; A running SDL is asked to quit by [Code] (it holds the mutex Swoopdl.Running);
; AppMutex isn't used because it would refuse to start before that. Restart Manager is the
; fallback for anything else holding the files (a browser running the host).
CloseApplications=yes
VersionInfoVersion={#AppVersion}
VersionInfoDescription={#AppName} setup
VersionInfoProductName={#AppName}
#ifdef Sign
SignTool=sdlsign
SignedUninstaller=yes
  #define SignFlag "signonce"
#else
  #define SignFlag ""
#endif

; Every interface language (i18n/languages.tsv): tools/installer/languages.py writes the list and
; the translations of the messages below. The wizard starts in Windows' display language.
#include "languages.iss"

[CustomMessages]
lang_en.TaskTorrents=Open .&torrent files and magnet links with {#AppName}
lang_en.TaskStartup=Start {#AppName} when &Windows starts
lang_en.TaskGroupOther=Other:
lang_en.Launch=Start {#AppName}
lang_en.KeepData=Also delete your download list and settings?%n%nYour downloaded files are not touched either way.
lang_en.TaskBrowser=Set up browser integration (Chrome/Firefox Native Messaging)

[Tasks]
Name: "torrents"; Description: "{cm:TaskTorrents}"
Name: "startup"; Description: "{cm:TaskStartup}"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:TaskGroupOther}"; Flags: unchecked
Name: "browser"; Description: "{cm:TaskBrowser}"

[Files]
Source: "{#Bin}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion {#SignFlag}
Source: "{#Bin}\{#HostExe}"; DestDir: "{app}"; Flags: ignoreversion {#SignFlag}
Source: "{#Bin}\swoopdl-cli.exe"; DestDir: "{app}"; Flags: ignoreversion {#SignFlag}
Source: "{#Bin}\swoopdl-mcp.exe"; DestDir: "{app}"; Flags: ignoreversion {#SignFlag}
Source: "{#Root}\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "{#Root}\ATTRIBUTION.md"; DestDir: "{app}"; DestName: "ATTRIBUTION.txt"; Flags: ignoreversion
Source: "{#Root}\target\extension\bundle\*"; DestDir: "{app}\extension"; Flags: ignoreversion recursesubdirs; Tasks: browser

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon
Name: "{autoprograms}\{#AppName} Browser Setup"; Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoLogo -NoProfile -ExecutionPolicy Bypass -File ""{app}\extension\install-local.ps1"""; Tasks: browser

[Registry]
; The language chosen in the wizard ({language}: "lang_" + a languages.tsv code with "_" for "-",
; see languages.py inno_name). The app
; takes it on its first start, when it has no language setting yet (settings.rs), then deletes it.
Root: HKCU; Subkey: "Software\Swoopdl"; ValueType: string; ValueName: "InstallLanguage"; ValueData: "{language}"; Flags: uninsdeletevalue uninsdeletekeyifempty
; Start with Windows: the same value Options -> "Launch on startup" writes and reads, with the
; argument that starts SDL in the notification area (#226; win32.rs startup_command).
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Swoopdl"; ValueData: """{app}\{#AppExe}"" /startup"; Flags: uninsdeletevalue; Tasks: startup
; .torrent files and magnet links (HKA: the user's or everyone's classes, as installed).
Root: HKA; Subkey: "Software\Classes\Swoopdl.torrent"; ValueType: string; ValueName: ""; ValueData: "Torrent file"; Flags: uninsdeletekey; Tasks: torrents
Root: HKA; Subkey: "Software\Classes\Swoopdl.torrent\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe},0"; Tasks: torrents
Root: HKA; Subkey: "Software\Classes\Swoopdl.torrent\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: torrents
Root: HKA; Subkey: "Software\Classes\.torrent"; ValueType: string; ValueName: ""; ValueData: "Swoopdl.torrent"; Flags: uninsdeletekeyifempty; Tasks: torrents
Root: HKA; Subkey: "Software\Classes\.torrent\OpenWithProgids"; ValueType: string; ValueName: "Swoopdl.torrent"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty; Tasks: torrents
Root: HKA; Subkey: "Software\Classes\magnet"; ValueType: string; ValueName: ""; ValueData: "URL:Magnet link"; Tasks: torrents
Root: HKA; Subkey: "Software\Classes\magnet"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""; Tasks: torrents
Root: HKA; Subkey: "Software\Classes\magnet\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe},0"; Tasks: torrents
Root: HKA; Subkey: "Software\Classes\magnet\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: torrents

[Run]
Filename: "{app}\{#HostExe}"; Parameters: "--register --all-users"; Flags: runhidden; Tasks: browser; Check: IsAdminInstallMode
Filename: "{app}\{#HostExe}"; Parameters: "--register"; Flags: runhidden; Tasks: browser; Check: not IsAdminInstallMode
Filename: "{app}\{#AppExe}"; Description: "{cm:Launch}"; Flags: nowait postinstall skipifsilent
; Update now in SDL runs this setup silently with /relaunch=1: SDL starts again once installed,
; as the user (not as the administrator an all-users update asked for).
Filename: "{app}\{#AppExe}"; Parameters: "/updated"; Flags: nowait runasoriginaluser; Check: Relaunch
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoLogo -NoProfile -ExecutionPolicy Bypass -File ""{app}\extension\install-local.ps1"""; Description: "Set up the bundled browser extension (browser approval required)"; Flags: nowait postinstall skipifsilent runasoriginaluser; Tasks: browser

[UninstallRun]
Filename: "{app}\{#HostExe}"; Parameters: "--unregister"; Flags: runhidden; RunOnceId: "UnregisterUser"; Check: not IsAdminInstallMode
Filename: "{app}\{#HostExe}"; Parameters: "--unregister --all-users"; Flags: runhidden; RunOnceId: "UnregisterAll"; Check: IsAdminInstallMode

[UninstallDelete]
Type: files; Name: "{app}\com.swoopdl.host*.json"

[Code]
const
  RunningMutex = 'Swoopdl.Running';

{ /relaunch=1 on the command line (SDL's Update now): start SDL again after installing. }
function Relaunch: Boolean;
begin
  Result := ExpandConstant('{param:relaunch|0}') = '1';
end;

{ Ask a running SDL to save and quit, and wait up to 15 s for it. }
procedure ExitRunningApp(Exe: String);
var
  Code, Waited: Integer;
begin
  if CheckForMutexes(RunningMutex) and FileExists(Exe) then
  begin
    Exec(Exe, '/exit', '', SW_HIDE, ewWaitUntilTerminated, Code);
    Waited := 0;
    while CheckForMutexes(RunningMutex) and (Waited < 15000) do
    begin
      Sleep(250);
      Waited := Waited + 250;
    end;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  ExitRunningApp(ExpandConstant('{app}\{#AppExe}'));
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  ExitRunningApp(ExpandConstant('{app}\{#AppExe}'));
  Result := True;
end;

{ Remove a class key only while it still points at us (another program may have taken over
  magnet links or .torrent files since). }
procedure ForgetIfOurs(Key, Ours: String);
var
  Value: String;
begin
  if RegQueryStringValue(HKA, Key + '\shell\open\command', '', Value) and (Pos(Lowercase(Ours), Lowercase(Value)) > 0) then
    RegDeleteKeyIncludingSubkeys(HKA, Key);
end;

{ Asked before the files go (the sign-ins are removed by SDL itself), the list and settings
  removed after. A silent uninstall keeps both. }
var
  RemoveData: Boolean;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Value, Data: String;
  Code: Integer;
begin
  Data := ExpandConstant('{userappdata}\swoopdl');
  if CurUninstallStep = usUninstall then
  begin
    ForgetIfOurs('Software\Classes\magnet', ExpandConstant('{app}\{#AppExe}'));
    if RegQueryStringValue(HKA, 'Software\Classes\.torrent', '', Value) and (Value = 'Swoopdl.torrent') then
      RegDeleteValue(HKA, 'Software\Classes\.torrent', '');
    RemoveData := False;
    if DirExists(Data) and not UninstallSilent() then
      RemoveData := MsgBox(CustomMessage('KeepData'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES;
    { The sign-ins SDL keeps in Windows Credential Manager (only SDL's own entries). }
    if RemoveData then
      Exec(ExpandConstant('{app}\{#AppExe}'), '--forget-sign-ins', '', SW_HIDE, ewWaitUntilTerminated, Code);
  end;
  if (CurUninstallStep = usPostUninstall) and RemoveData then
    DelTree(Data, True, True, True);
end;
