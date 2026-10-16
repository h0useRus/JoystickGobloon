; Joystick Gobloon installer (Inno Setup 6).
; Build: iscc /DAppVersion=0.1.0 installer\joygob.iss  (after `cargo build --release`)
; Output: target\installer\JoystickGobloon-<version>-setup.exe

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "Joystick Gobloon"
#define AppExe "JoystickGobloon.exe"
#define HidHideUrl "https://github.com/nefarius/HidHide/releases"
#define VJoyUrl "https://github.com/BrunnerInnovation/vJoy/releases"

[Setup]
AppId={{31D3CD9B-BF83-41A1-8D79-FDD6912A6458}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=h0use
AppPublisherURL=https://github.com/h0useRus/JoystickGobloon
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
OutputDir=..\target\installer
OutputBaseFilename=JoystickGobloon-{#AppVersion}-setup
SetupIconFile=..\assets\icon\joygob.ico
UninstallDisplayIcon={app}\{#AppExe}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"

[CustomMessages]
en.DesktopIcon=Create a desktop shortcut
ru.DesktopIcon=Создать ярлык на рабочем столе
en.Missing=Joystick Gobloon uses the drivers of these programs, which are not installed:%n%n%1%nOpen their download pages now? You can install them later too.
ru.Missing=Joystick Gobloon использует драйверы этих программ, но они не установлены:%n%n%1%nОткрыть страницы загрузки сейчас? Их можно установить и позже.

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\lang\*.ftl"; DestDir: "{app}\lang"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; JoyGob writes this value itself ("Start with Windows"); only remove it on uninstall.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "JoystickGobloon"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Code]
function HidHideCli(): String;
var
  Dir: String;
begin
  Result := '';
  if RegQueryStringValue(HKLM, 'SOFTWARE\Nefarius Software Solutions e.U.\HidHide', 'Path', Dir) then
    Result := AddBackslash(Dir) + 'x64\HidHideCLI.exe';
end;

function VJoyInstalled(): Boolean;
begin
  Result := RegKeyExists(HKLM, 'SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{8E31F76F-74C3-47F1-9550-E041EEDC5FBB}_is1')
    or FileExists(ExpandConstant('{commonpf64}\vJoy\x64\vJoyInterface.dll'));
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Missing: String;
  Code: Integer;
begin
  if (CurStep <> ssPostInstall) or WizardSilent() then
    Exit;
  Missing := '';
  if (HidHideCli() = '') or not FileExists(HidHideCli()) then
    Missing := Missing + '- HidHide' + #13#10;
  if not VJoyInstalled() then
    Missing := Missing + '- vJoy' + #13#10;
  if Missing = '' then
    Exit;
  if MsgBox(FmtMessage(CustomMessage('Missing'), [Missing]), mbConfirmation, MB_YESNO) = IDYES then
  begin
    if Pos('HidHide', Missing) > 0 then
      ShellExec('open', '{#HidHideUrl}', '', '', SW_SHOWNORMAL, ewNoWait, Code);
    if Pos('vJoy', Missing) > 0 then
      ShellExec('open', '{#VJoyUrl}', '', '', SW_SHOWNORMAL, ewNoWait, Code);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Cli: String;
  Code: Integer;
begin
  // JoyGob adds itself to the HidHide whitelist; take it off again.
  if CurUninstallStep <> usUninstall then
    Exit;
  Cli := HidHideCli();
  if (Cli <> '') and FileExists(Cli) then
    Exec(Cli, '--app-unreg "' + ExpandConstant('{app}\{#AppExe}') + '"', '', SW_HIDE, ewWaitUntilTerminated, Code);
end;
