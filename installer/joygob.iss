; Joystick Gobloon installer (Inno Setup 6).
; Build: iscc /DAppVersion=0.1.0 installer\joygob.iss  (after `cargo build --release`)
; Needs the HidHide and vJoy installers in target\installer\deps (release.yml downloads them
; and checks their hashes). Output: target\installer\JoystickGobloon-<version>-setup.exe

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "Joystick Gobloon"
#define AppExe "JoystickGobloon.exe"
; Latest signed HidHide release (https://github.com/nefarius/HidHide/releases, MIT).
#define HidHideSetup "HidHide_1.5.230_x64.exe"
; Official signed vJoy 2.1.9.1 (https://github.com/jshafer817/vJoy/releases, MIT): the driver
; version Joystick Gobloon's vJoy code is written and tested for.
#define VJoySetup "vJoySetup.exe"

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
; Installs into Program Files and, when needed, the HidHide and vJoy drivers: asks for
; administrator rights.
PrivilegesRequired=admin

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
; Official only in newer Inno Setup releases: used when the installed compiler has them.
#if FileExists(CompilerPath + "Languages\ChineseSimplified.isl")
  #define HaveZh
Name: "zh"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
#endif
#if FileExists(CompilerPath + "Languages\Korean.isl")
  #define HaveKo
Name: "ko"; MessagesFile: "compiler:Languages\Korean.isl"
#endif

[CustomMessages]
en.DesktopIcon=Create a desktop shortcut
ru.DesktopIcon=Создать ярлык на рабочем столе
en.Drivers=Drivers:
ru.Drivers=Драйверы:
en.InstallHidHide=Install the HidHide driver (hides game controllers from other programs; a restart may be needed)
ru.InstallHidHide=Установить драйвер HidHide (скрывает игровые устройства от других программ; может понадобиться перезагрузка)
en.InstallingHidHide=Installing the HidHide driver...
ru.InstallingHidHide=Установка драйвера HidHide...
en.HidHideFailed=The HidHide driver was not installed (code %1). Device hiding will not work until HidHide is installed.
ru.HidHideFailed=Драйвер HidHide не установлен (код %1). Скрытие устройств не будет работать, пока HidHide не установлен.
en.InstallVJoy=Install the vJoy driver (virtual joysticks that profiles output to; a restart may be needed)
ru.InstallVJoy=Установить драйвер vJoy (виртуальные джойстики для вывода профилей; может понадобиться перезагрузка)
en.InstallingVJoy=Installing the vJoy driver...
ru.InstallingVJoy=Установка драйвера vJoy...
en.VJoyFailed=The vJoy driver was not installed (code %1). Profiles cannot output to virtual joysticks until vJoy is installed.
ru.VJoyFailed=Драйвер vJoy не установлен (код %1). Профили не смогут выводить на виртуальные джойстики, пока vJoy не установлен.

de.DesktopIcon=Verknüpfung auf dem Desktop erstellen
de.Drivers=Treiber:
de.InstallHidHide=HidHide-Treiber installieren (verbirgt Gamecontroller vor anderen Programmen; evtl. ist ein Neustart nötig)
de.InstallingHidHide=HidHide-Treiber wird installiert...
de.HidHideFailed=Der HidHide-Treiber wurde nicht installiert (Code %1). Geräte verbergen funktioniert erst, wenn HidHide installiert ist.
de.InstallVJoy=vJoy-Treiber installieren (virtuelle Joysticks für die Ausgabe von Profilen; evtl. ist ein Neustart nötig)
de.InstallingVJoy=vJoy-Treiber wird installiert...
de.VJoyFailed=Der vJoy-Treiber wurde nicht installiert (Code %1). Profile können erst auf virtuelle Joysticks ausgeben, wenn vJoy installiert ist.
es.DesktopIcon=Crear un acceso directo en el escritorio
es.Drivers=Controladores:
es.InstallHidHide=Instalar el controlador HidHide (oculta los mandos de juego a otros programas; puede requerir reiniciar)
es.InstallingHidHide=Instalando el controlador HidHide...
es.HidHideFailed=El controlador HidHide no se instaló (código %1). La ocultación de dispositivos no funcionará hasta que se instale HidHide.
es.InstallVJoy=Instalar el controlador vJoy (joysticks virtuales a los que envían los perfiles; puede requerir reiniciar)
es.InstallingVJoy=Instalando el controlador vJoy...
es.VJoyFailed=El controlador vJoy no se instaló (código %1). Los perfiles no podrán enviar a joysticks virtuales hasta que se instale vJoy.
fr.DesktopIcon=Créer un raccourci sur le Bureau
fr.Drivers=Pilotes :
fr.InstallHidHide=Installer le pilote HidHide (masque les manettes de jeu aux autres programmes ; un redémarrage peut être nécessaire)
fr.InstallingHidHide=Installation du pilote HidHide...
fr.HidHideFailed=Le pilote HidHide n’a pas été installé (code %1). Le masquage des périphériques ne fonctionnera pas tant que HidHide n’est pas installé.
fr.InstallVJoy=Installer le pilote vJoy (joysticks virtuels vers lesquels les profils envoient ; un redémarrage peut être nécessaire)
fr.InstallingVJoy=Installation du pilote vJoy...
fr.VJoyFailed=Le pilote vJoy n’a pas été installé (code %1). Les profils ne pourront pas envoyer vers des joysticks virtuels tant que vJoy n’est pas installé.
#ifdef HaveZh
zh.DesktopIcon=创建桌面快捷方式
zh.Drivers=驱动程序：
zh.InstallHidHide=安装 HidHide 驱动（对其他程序隐藏游戏控制器；可能需要重启）
zh.InstallingHidHide=正在安装 HidHide 驱动...
zh.HidHideFailed=HidHide 驱动未安装（代码 %1）。安装 HidHide 之前，设备隐藏将无法使用。
zh.InstallVJoy=安装 vJoy 驱动（配置文件输出所用的虚拟摇杆；可能需要重启）
zh.InstallingVJoy=正在安装 vJoy 驱动...
zh.VJoyFailed=vJoy 驱动未安装（代码 %1）。安装 vJoy 之前，配置文件无法输出到虚拟摇杆。
#endif
#ifdef HaveKo
ko.DesktopIcon=바탕 화면 바로 가기 만들기
ko.Drivers=드라이버:
ko.InstallHidHide=HidHide 드라이버 설치 (다른 프로그램에서 게임 컨트롤러를 숨김; 다시 시작이 필요할 수 있음)
ko.InstallingHidHide=HidHide 드라이버를 설치하는 중...
ko.HidHideFailed=HidHide 드라이버가 설치되지 않았습니다(코드 %1). HidHide를 설치할 때까지 장치 숨기기가 작동하지 않습니다.
ko.InstallVJoy=vJoy 드라이버 설치 (프로필이 출력하는 가상 조이스틱; 다시 시작이 필요할 수 있음)
ko.InstallingVJoy=vJoy 드라이버를 설치하는 중...
ko.VJoyFailed=vJoy 드라이버가 설치되지 않았습니다(코드 %1). vJoy를 설치할 때까지 프로필이 가상 조이스틱으로 출력할 수 없습니다.
#endif
[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; Flags: unchecked
; Offered (and ticked) only when the driver is missing.
Name: "hidhide"; Description: "{cm:InstallHidHide}"; GroupDescription: "{cm:Drivers}"; Check: NeedHidHide
Name: "vjoy"; Description: "{cm:InstallVJoy}"; GroupDescription: "{cm:Drivers}"; Check: NeedVJoy

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\lang\*.ftl"; DestDir: "{app}\lang"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
; Extracted to {tmp} only when needed (InstallHidHide / InstallVJoy); not installed or
; uninstalled themselves.
Source: "..\target\installer\deps\{#HidHideSetup}"; Flags: dontcopy
Source: "..\target\installer\deps\{#VJoySetup}"; Flags: dontcopy

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; Joystick Gobloon writes this value itself ("Start with Windows"); only remove it on uninstall.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "JoystickGobloon"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Code]
var
  RestartNeeded: Boolean;

{ The HidHide filter driver is installed (Joystick Gobloon talks to it directly). }
function HidHideInstalled(): Boolean;
begin
  Result := RegKeyExists(HKLM, 'SYSTEM\CurrentControlSet\Services\HidHide')
    or FileExists(ExpandConstant('{sys}\drivers\HidHide.sys'));
end;

function NeedHidHide(): Boolean;
begin
  Result := not HidHideInstalled();
end;

{ The vJoy driver is installed (Joystick Gobloon talks to it directly, like gob-vjoy checks). }
function VJoyInstalled(): Boolean;
begin
  Result := RegKeyExists(HKLM, 'SYSTEM\CurrentControlSet\Services\vjoy')
    or FileExists(ExpandConstant('{sys}\drivers\vjoy.sys'));
end;

function NeedVJoy(): Boolean;
begin
  Result := not VJoyInstalled();
end;

{ Runs the signed HidHide installer and waits; 3010/1641 mean it wants a restart. }
procedure InstallHidHide();
var
  Params: String;
  Code: Integer;
begin
  WizardForm.StatusLabel.Caption := CustomMessage('InstallingHidHide');
  ExtractTemporaryFile('{#HidHideSetup}');
  Params := '';
  if WizardSilent() then
    Params := '/exenoui /qn /norestart';
  if not Exec(ExpandConstant('{tmp}\{#HidHideSetup}'), Params, '', SW_SHOWNORMAL, ewWaitUntilTerminated, Code) then
    Code := -1;
  Log(Format('HidHide installer exit code: %d', [Code]));
  if (Code = 3010) or (Code = 1641) then
    RestartNeeded := True
  else if (Code <> 0) and not WizardSilent() then
    MsgBox(FmtMessage(CustomMessage('HidHideFailed'), [IntToStr(Code)]), mbError, MB_OK);
end;

{ Runs the signed vJoy installer (Inno Setup) and waits; it shows its own wizard unless this
  setup runs silently. Afterwards the driver service tells whether it worked. }
procedure InstallVJoy();
var
  Params: String;
  Code: Integer;
begin
  WizardForm.StatusLabel.Caption := CustomMessage('InstallingVJoy');
  ExtractTemporaryFile('{#VJoySetup}');
  Params := '/NORESTART';
  if WizardSilent() then
    Params := '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART';
  if not Exec(ExpandConstant('{tmp}\{#VJoySetup}'), Params, '', SW_SHOWNORMAL, ewWaitUntilTerminated, Code) then
    Code := -1;
  Log(Format('vJoy installer exit code: %d', [Code]));
  if (Code <> 0) and not VJoyInstalled() and not WizardSilent() then
    MsgBox(FmtMessage(CustomMessage('VJoyFailed'), [IntToStr(Code)]), mbError, MB_OK);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep <> ssPostInstall then
    Exit;
  if WizardIsTaskSelected('hidhide') then
    InstallHidHide();
  if WizardIsTaskSelected('vjoy') then
    InstallVJoy();
end;

function NeedRestart(): Boolean;
begin
  Result := RestartNeeded;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Code: Integer;
begin
  // Joystick Gobloon adds itself to HidHide's application list; it takes itself off again.
  // The HidHide driver stays: other programs may use it.
  if CurUninstallStep <> usUninstall then
    Exit;
  Exec(ExpandConstant('{app}\{#AppExe}'), '--hidhide-unregister', '', SW_HIDE, ewWaitUntilTerminated, Code);
end;
