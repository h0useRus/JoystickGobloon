# Версия для Linux

Цель: Joystick Gobloon на Linux с теми же возможностями, что на Windows: чтение джойстиков, профили Gremlin,
виртуальные джойстики, скрытие устройств, клавиатура/мышь, подсветка, трей. **Только Linux**, macOS не
поддерживаем (виртуальный джойстик там требует DriverKit-драйвера с разрешениями Apple, скрытия устройств нет).

Платформы: обычные дистрибутивы и **Steam Deck**, **X11 и Wayland**. Пакет: **AppImage** (один файл, работает
везде, в том числе на Steam Deck в режиме рабочего стола, где системный раздел только для чтения и .deb не
поставить; обновляется сам, как переносная версия на Windows). Flatpak — позже, если понадобится: его песочница
не ставит правила udev.

Ветка: `feat/linux` от `dev` (фаза 0), дальше по ветке на фазу. Одна сессия — один крейт (CLAUDE.md).

## Что уже переносимо

| Часть | Строк | Зависит от Windows |
|---|---|---|
| gob-core, gob-gremlin, gob-i18n, gob-plugin | ~4 200 | нет |
| gob-engine | 3 900 | `media.rs` (речь SAPI), `sendinput.rs` (клавиатура/мышь); сам движок нейтрален (трейт `Output`) |
| gob-input | 1 130 | всё: DirectInput, клавиатурный хук |
| gob-vjoy | 1 980 | всё (драйвер vJoy) |
| gob-hidhide | 1 220 | всё (драйвер HidHide) |
| gob-lighting | 980 | только `hid.rs` (HID feature reports) |
| joystick-gobloon | 11 700 | `instance.rs`, `key_capture.rs`, `logfile.rs`, `tray.rs`, `update.rs`; путь настроек, шрифты CJK |

## Что чем заменяем

| Windows | Linux |
|---|---|
| DirectInput, id = GUID экземпляра | **evdev** (`/dev/input/event*`) + udev (подключение/отключение). Id из vendor/product/serial (или пути), в формате GUID; профили с Windows — через «Сопоставить устройства» |
| Клавиатурный хук | клавиатуры evdev (без захвата; работает и в X11, и в Wayland); коды evdev → scan code set 1 таблицей |
| vJoy + его конфигурация | **uinput**: виртуальные джойстики создаются при активации профиля, драйвер и его настройка не нужны. Страница vJoy → список виртуальных устройств в настройках (оси, кнопки, хатки) |
| SendInput | клавиатура и мышь на uinput |
| HidHide | **EVIOCGRAB** — монопольный захват физического устройства на время активного профиля: игры перестают видеть его через evdev/joydev. hidraw (SDL hidapi, Steam) захват не закрывает — права на hidraw через правило udev |
| Подсветка (`HidD_SetFeature`) | hidraw, `HIDIOCSFEATURE`; новый `hid.rs` для Linux, остальное как есть |
| SAPI | speech-dispatcher; звук (rodio) уже работает |
| `%APPDATA%`, автозапуск в реестре, именованный мьютекс | `$XDG_CONFIG_HOME/JoystickGobloon`, `~/.config/autostart/*.desktop`, сокет/файл блокировки в `$XDG_RUNTIME_DIR` |
| Трей, «прозрачное» окно в трее | tray-icon (GTK/AppIndicator) или ksni (StatusNotifierItem). GNOME на Wayland без расширения трея не показывает — тогда без трея, обычное сворачивание. Поведение скрытого окна eframe проверить заново |
| Шрифты CJK из `C:\Windows\Fonts` | Noto CJK через fontconfig |
| Установщик, обновление | AppImage; обновление — сначала только уведомление со ссылкой, потом замена файла как у переносной версии |
| Плагины `.dll` | `.so` (libloading уже кроссплатформенный) |
| Страница «Приложения» (белый список HidHide) | на Linux не нужна (захват вместо списка), скрыть |

Права: `/dev/uinput` на большинстве дистрибутивов только для root, `/dev/input/event*` — для группы `input`.
Программа при первом запуске ставит правило udev (`TAG+="uaccess"` для uinput, джойстиков и их hidraw) через
`pkexec` — как настройка vJoy через UAC на Windows.

## Архитектура: платформенный слой

Всё, что работает с ОС и драйверами напрямую, живёт в **отдельных модулях для каждой платформы** за **общим
интерфейсом**. UI и бизнес-логика (gob-core, gob-gremlin, движок gob-engine, страницы GUI) пользуются только
этим интерфейсом. Тогда реализацию для Windows или Linux можно оптимизировать или переписать целиком, не трогая
UI и логику: например, заменить vJoy **своим драйвером для Windows** — это новая реализация того же интерфейса.

Сейчас это не так: GUI и движок напрямую используют `gob_vjoy` (5 модулей GUI и `runner.rs`) и `gob_hidhide`
(4 модуля GUI), а `windows::` / `cfg(windows)` есть в 7 модулях GUI и движка.

| Область | Общий интерфейс (им пользуются UI и движок) | Windows | Linux |
|---|---|---|---|
| Ввод | `gob-input`: `InputService`, `DeviceInfo`/`DeviceState`, `InputEvent` (уже есть); внутри — трейт бэкенда | DirectInput, клавиатурный хук | evdev + udev, клавиатуры evdev |
| Виртуальные устройства | новый крейт `gob-virtual`: список устройств и их раскладка (оси, кнопки, хатки), `acquire(n)` → устройство с `set_axis` / `set_button` / `set_hat` / `reset`, настройка устройств | `vjoy` (сегодняшний gob-vjoy); позже свой драйвер | uinput |
| Скрытие устройств | новый крейт `gob-hide`: состояние, скрыть/показать устройство, вкл/выкл, программы-исключения (если есть) | HidHide (сегодняшний gob-hidhide) | захват EVIOCGRAB + правило udev |
| Клавиатура и мышь (вывод) | модуль `platform` в gob-engine: трейт `Injector` | SendInput | uinput |
| Речь | модуль `platform` в gob-engine: трейт `Speech` | SAPI | speech-dispatcher |
| HID-транспорт подсветки | gob-lighting: открыть по пути, get/set feature report | SetupAPI + `HidD_*` | hidraw |
| Рабочий стол | модуль `platform` в joystick-gobloon: один экземпляр, автозапуск, папка данных, окно в трей, шрифты, обновление, захват клавиши | Win32, реестр | XDG, сокет, `.desktop` |

Правила:
1. В UI, движке и модели нет `windows::`, `libc`, `evdev`, `cfg(windows)` / `cfg(target_os)` — только в
   модулях `platform/` (`platform/windows.rs`, `platform/linux.rs`) и в крейтах-бэкендах.
2. Интерфейс сообщает **возможности** платформы, UI подстраивается под них, а не под ОС: нет программ-исключений
   у скрытия → страница «Приложения» не показывается; виртуальным устройствам не нужна настройка драйвера →
   вместо страницы настройки vJoy простой список устройств.
3. Реализация выбирается в одном месте: при сборке (`cfg`) и, если на платформе их несколько, при запуске (vJoy
   или свой драйвер, evdev или hidraw).
4. Общие ошибки (не установлено, нет прав, не поддерживается) — в интерфейсе; тексты для пользователя
   переводит UI.
5. В интерфейсах общие имена (виртуальное устройство, скрытие); vJoy, HidHide и т. п. — только внутри
   реализаций и в текстах, когда реализация сама сообщает своё имя.
6. **SteamOS** — часть Linux-реализации: отличия (системный раздел только для чтения, игровой режим gamescope,
   Steam Input, который сам захватывает устройства) определяются при запуске (`/etc/os-release`, `ID=steamos`).
   Отдельный модуль — только если отличий станет много.
7. Тесты логики не зависят от ОС; для движка и UI — поддельные реализации в памяти (как сегодняшний трейт
   `Output` в gob-engine).
8. Большие реализации, у которых есть свои примеры и тесты (gob-vjoy, gob-hidhide, будущий uinput), остаются
   отдельными крейтами-бэкендами; крейт интерфейса подключает их только для своей платформы.

## Фазы

0. **Сборка на Linux** — готово. Windows-код уже был за `cfg(windows)` с заглушками, оставались только
   предупреждения «не используется» (код, нужный лишь Windows-реализациям). Задание `linux` в CI (clippy,
   тесты), `tools\linux\check.ps1` + `tools/linux/Containerfile` — то же локально в Podman. Программа
   собирается и линкуется (GTK, AppIndicator, ALSA, OpenSSL).
1. **Платформенный слой — только Windows, поведение не меняется** — готово. Интерфейсы из таблицы выше; сегодняшний
   Windows-код переезжает за них: gob-vjoy и gob-hidhide становятся бэкендами `gob-virtual` и `gob-hide`, GUI и
   движок переходят на интерфейсы, Win32-код GUI и движка — в модули `platform/`. По сессии на область (одна
   сессия — один крейт). Проверка: на Windows всё как раньше (тесты, `tray_cycle.ps1`, `single_instance.ps1`,
   ручная проверка); в UI и движке не осталось `windows::` и `cfg(windows)`.
   - [x] Виртуальные устройства: крейт `gob-virtual` (`Backend`, `Device`, `Capabilities`, конфигурация),
     бэкенд vJoy поверх gob-vjoy; движок и GUI больше не знают gob-vjoy.
   - [x] Скрытие: крейт `gob-hide` (`Backend`: вкл/выкл, устройства с флагом `hidden`, список программ,
     `Capabilities`; `find_device`), бэкенд HidHide поверх gob-hidhide; GUI больше не знает gob-hidhide.
   - [x] Ввод: в gob-input общий `service.rs` (цикл: пересканирование, калибровки, поиск изменений, снимок
     для GUI) поверх внутренних трейтов `platform::Backend` / `Device`; `platform/windows/` — DirectInput,
     клавиатурный хук, номера WinMM; `platform/none.rs` — без устройств. API `InputService` не изменился.
   - [x] Клавиатура/мышь и речь: в gob-engine `platform/` с трейтами `Injector` и `SpeechEngine`;
     `platform/windows/` — SendInput и SAPI, `platform/none.rs` — ничего; `media.rs` (звук, очередь речи,
     список голосов) и `runner.rs` общие.
   - [x] HID-транспорт подсветки: gob-lighting `hid/` — трейт `Transport` (коллекции производителя, get/set
     feature report), `hid/windows.rs` — SetupAPI + `HidD_*`, `hid/none.rs` — без устройств.
   - [x] Рабочий стол: в joystick-gobloon `platform/` с трейтом `Desktop` (папка данных, шрифты CJK, открыть
     текстовый файл, расширения программ, время для лога, один экземпляр, клавиши; окно как непрозрачный
     `Window` — в трей, показать, свернуть, состояние; время двойного клика, размер значка трея, автозапуск;
     обновления — установленная копия, имя ассета и exe, установщик, распаковка zip, ожидание процесса).
     `platform/windows/` — Win32, Inno Setup; `platform/none.rs` — запасные варианты.
   - Итог: `windows::` / `cfg(windows)` остались только в модулях `platform/`, `hid/`, крейтах-бэкендах
     (gob-vjoy, gob-hidhide) и в одном месте выбора бэкенда (`gob-virtual`, `gob-hide`).
2. **Ввод на Linux.** evdev + udev — Linux-реализация в gob-input. Результат: страница «Устройства», просмотр
   ввода и калибровка работают. **Первым делом**: сколько кнопок Linux показывает у VKB (128 кнопок; ядро
   отдаёт джойстикам 32 кода `BTN_JOYSTICK`/`BTN_GAMEPAD` + 64 `BTN_TRIGGER_HAPPY` до `KEY_MAX`, остальное —
   как получится). Если кнопки теряются — читать hidraw и разбирать HID-отчёты самим (дескрипторы уже
   умеем, gob-vjoy).
   - [x] gob-input `platform/linux/`: джойстики из `/dev/input/event*` (как udev `input_id`: кнопки или оси
     джойстика, не мышь/тачпад/перо/акселерометр); подключение — тем же пересканированием раз в 2 с, что и на
     Windows (udev не нужен). Id: `PPPPVVVV-<шина>-0000-<FNV-1a от серийного номера или phys + имени>`.
     Оси — в слоты DirectInput (X…Rz, следующие две — ползунки), кнопки в порядке SDL, хатки
     `ABS_HATnX/Y` (диагональ — одним событием в конце отчёта), режим событий — `poll(2)`, `SYN_DROPPED` —
     перечитать состояние ioctl. `hid_path` = `/dev/hidrawN` того же HID-устройства, `joystick_ids` = номера
     `/dev/input/jsN`. Клавиатуры evdev → scan code set 1 как у хука Windows (NumLock = E0 45, F13… = 0x64…),
     без автоповтора, устройства с именем «Joystick Gobloon…» (наш вывод, фаза 3) пропускаются.
   - [x] Тест без железа: `tests/uinput.rs` (виртуальный стик на 96 кнопок и клавиатура),
     `tools\linux\check.ps1 --devices cargo test -p gob-input -- --include-ignored`.
   - [ ] Настоящий VKB через usbipd: число кнопок, оси, `hid_path`; GUI на Linux (страница «Устройства»,
     просмотр, калибровка).
3. **Вывод на Linux.** uinput-бэкенд `gob-virtual` (джойстик) и `Injector` (клавиатура/мышь). Результат:
   профили работают. Проверка: чтение вывода через evdev, `jstest`, игра через Proton. Те же вопросы о числе
   кнопок — у игр (SDL).
   - [x] Крейт-бэкенд `gob-uinput`: `VirtualDevice` (кнопки/клавиши, оси, REL), `emit` одним отчётом,
     узел event через `UI_GET_SYSNAME`, `device_names`; пример `uinput_check`.
   - [x] `gob-virtual/src/linux.rs`: устройство создаётся при `acquire` и исчезает при отпускании; раскладка
     в `virtual.gob` (TOML, папка из `gob_virtual::set_data_dir`, без неё `$XDG_CONFIG_HOME/JoystickGobloon`;
     без файла — одно устройство по умолчанию). Имя «Joystick Gobloon Joystick N», vendor/product как у
     vJoy (0x1234/0xBEAD — gob-input считает их нашим выводом). Оси — коды, которые gob-input читает в те же
     слоты (ползунки = `ABS_THROTTLE`/`ABS_RUDDER`), кнопки 1-32 = `BTN_JOYSTICK…`, 33-96 =
     `BTN_TRIGGER_HAPPY…` (до `KEY_MAX`; **пока максимум 96**: больше — только другими кодами клавиш, их
     порядок у SDL другой — решать после проверки игр). Хатки `ABS_HATnX/Y`, непрерывные — 8 направлений.
     «Занято» = устройство с таким именем есть в системе, но не наше (вторая копия программы). Нет прав
     на `/dev/uinput` → `Error::NoAccess`. Тест `tests/uinput.rs` (`check.ps1 --devices`).
   - [x] `Injector` на uinput (gob-engine `platform/linux/`): два устройства, «Joystick Gobloon Keyboard»
     (все клавиши из `gob_uinput::keys`) и «Joystick Gobloon Mouse» (REL_X/Y, колёса, 5 кнопок),
     создаются при старте профиля (`Injector::prepare`: события сразу после создания рабочий стол
     теряет) и живут до выхода. Scan code set 1 ↔ evdev — таблица `gob_uinput::keys` (обе стороны;
     ей же пользуется клавиатура gob-input). `key_pressed` — `EVIOCGKEY`
     по всем клавиатурам, включая нашу (как `GetAsyncKeyState`); нужно чтение `/dev/input`.
     Тест в `injector.rs` (ignored, `check.ps1 --devices`). Речь на Linux — пока нет (фаза 5).
   - [x] Страница vJoy на Linux: «vJoy» остаётся общим именем функции на обеих ОС; тексты про драйвер — по
     возможностям (`switchable`: «Драйвер включён», `needs_admin`: щит и «права администратора»), нет прав на
     `/dev/uinput` — свой текст (`vjoy-no-access`). Осталось: `about-credits` («Использует драйверы HidHide и
     vJoy») — когда будет скрытие на Linux (фаза 4).
   - [ ] Проверка: `jstest`, игра через Proton (SDL), профиль целиком.
4. **Скрытие на Linux.** Бэкенд `gob-hide` на захвате; установка правила udev через `pkexec`.
   - [x] gob-input `exclusive`: список устройств, которые сервис ввода читает один (`set`, `devices`,
     `supported`). На Linux поток ввода держит на них `EVIOCGRAB` (игры не получают событий ни через evdev,
     ни через joydev), пока программа работает; на Windows не поддерживается (там HidHide). Захват —
     у единственного `InputService` процесса. Тест `exclusive_hides_from_other_readers`.
   - [x] gob-hide `linux.rs`: выключатель и скрытые устройства в `hidden.gob` (папка из
     `gob_hide::set_data_dir`), отключённые скрытые остаются в списке; состояние применяется уже при
     `load`. `Capabilities::app_list` (Windows да, Linux нет): страница «Приложения» и «добавить себя в
     белый список» показываются только с ним; `gob_hide::capabilities()` — без загрузки бэкенда.
   - [x] Правило udev (gob-uinput `access`): `/etc/udev/rules.d/70-joystick-gobloon.rules` (до 73 — там
     `uaccess`), `uaccess` для `/dev/uinput` (`static_node`) и узлов event джойстиков. Ставится через
     `pkexec /bin/sh -c …` (не через запуск себя: root не видит FUSE-монтирование AppImage), затем
     `udevadm control --reload-rules` + `trigger`. GUI: `Desktop::device_access_missing` /
     `grant_device_access`, баннер «Выдать доступ» на странице «Устройства»; после успеха страницы vJoy и
     скрытия загружаются заново, клавиатура/мышь пробуют снова при старте профиля. Начат
     `joystick-gobloon/src/platform/linux/` (остальное пока из `none.rs`).
     **Клавиатуры не в правиле**: чтение всех клавиатур дало бы любой программе пользователя то, что он
     печатает. Поэтому вкладка Keyboard (клавиши как ввод) на Linux скрыта (`Desktop::keyboard_bindings`);
     Map to Keyboard и макросы работают (uinput). Вернуться, если пользователи попросят.
     **hidraw не трогаем**: по умолчанию он только у root, HOTAS через него не видны; исключение —
     устройства из правил Steam (`steam-devices`, в основном геймпады).
   - [x] «О программе»: строки «Скрытие устройств: …» и «Виртуальные джойстики: …» с именем и версией
     бэкенда (`gob_hide::capabilities`, `gob_virtual::capabilities`); на Linux «Linux evdev» / «Linux uinput»
     с версией ядра.
5. **Рабочий стол на Linux.** Linux-модули `platform/`: пути настроек, один экземпляр, автозапуск, трей, шрифты,
   речь. Подсветка — экспериментальная функция (скрытый ключ `experimental`), в перенос пока не входит.
   - [x] `joystick-gobloon/src/platform/linux/`: папка данных `$XDG_CONFIG_HOME/JoystickGobloon` (иначе
     `~/.config/…`; там же ищут gob-virtual и gob-hide), один экземпляр — абстрактный Unix-сокет
     `JoystickGobloon[.debug].<uid>` (без файла, после падения ничего не остаётся; подключение = «покажись»),
     автозапуск — `~/.config/autostart/joystick-gobloon.desktop` (для AppImage — путь из `$APPIMAGE`, `Exec`
     экранирован по спецификации), лог — `xdg-open`, время — `localtime_r`. Тексты автозапуска и темы без
     «Windows».
   - [ ] Окно и трей: tray-icon (AppIndicator/StatusNotifierItem), скрытие окна eframe; GNOME/Wayland без трея.
   - [x] Шрифты CJK: `fc-list :lang=zh-cn` / `:lang=ko`, предпочтение Noto Sans CJK / Source Han Sans
     (один файл на оба языка грузится один раз).
   - [x] Клавиши: `held_keys` → `Option` (`None` — состояния клавиатуры не видно); на Linux `EVIOCGKEY` по
     клавиатурам при группе `input`, иначе запись клавиш берёт события окна (физические клавиши egui → scan
     code; Ctrl/Shift/Alt — левые, правые egui не различает). Имена клавиш на Linux — английская таблица по
     scan code.
   - [ ] Речь через speech-dispatcher (gob-engine).
6. **Выпуск.** AppImage в `release.yml`, заметки и Discord как сейчас.

## Тестирование

- **Podman** (машина `pod-h0use`, WSL2, ядро 6.6): есть evdev, uinput, hidraw, hid-generic, usbip (модули
  грузятся `modprobe`). Проверено: виртуальный джойстик на uinput (56 кнопок, 8 осей, хатка) появляется как
  `/dev/input/event0`. Отсюда — сборка и тесты в контейнере, вывод (фаза 3) и **ввод без железа**: виртуальный
  «VKB» на uinput с теми же vendor/product и кнопками, управляемый из теста; захват (фаза 4) так же.
- **Настоящие джойстики**: `usbipd-win` (`winget install usbipd`, от администратора) пробрасывает USB в машину
  Podman, там его берёт HID-драйвер ядра. Так проверить число кнопок и подсветку через hidraw. Пока устройство
  проброшено, в Windows его нет.
- **Steam Deck, GNOME/KDE на Wayland, трей** — только на настоящем железе, в конце.

## Риски

- Число кнопок (128 у VKB и у виртуальных устройств): что видят ядро и SDL в играх.
- Трей на Wayland (GNOME), поведение скрытого окна eframe.
- Права на `/dev/uinput` и hidraw без правила udev.
- Proton: как игры под Wine видят виртуальные устройства uinput (через SDL/hidraw/evdev).
