# gob-vjoy: прямое управление драйвером vJoy

Цель: `gob-vjoy` говорит с драйвером vJoy напрямую через `DeviceIoControl` и реестр, как `gob-hidhide` с HidHide.
`vJoyInterface.dll` и `vJoyConfig.exe` больше не нужны; нужен только установленный драйвер vJoy.
Свой kernel-драйвер — вне рамок (см. ROADMAP). Публичный API крейта (`VJoy::acquire -> Device`, `config`)
по возможности не меняется, чтобы `joygob` и `gob-engine` почти не трогать.

## Сейчас

| Что | Через что | Где |
|---|---|---|
| Вывод: оси, кнопки, хатки, reset, захват устройства | `vJoyInterface.dll`, грузится в runtime, путь из uninstall-ключа vJoy | `crates/gob-vjoy/src/lib.rs` |
| Настройка устройств (оси, кнопки, хатки, FFB, добавить/удалить) | `vJoyConfig.exe` с UAC; текущая конфигурация — разбор `vJoyConfig -t` | `crates/gob-vjoy/src/config.rs` |

## Источники

- `ref/vJoy` — github.com/jshafer817/vJoy, **2.1.9.1** (подписан для Win10, стоит у пользователя: `C:\Program Files\vJoy\`).
- `ref/vJoy-brunner` — github.com/BrunnerInnovation/vJoy, **2.2.2.0** (2023-12, Win10/Win11; не поддерживается, см. вопрос 4).
- Обе ветки — MIT (Shaul Eizikovich), установщики можно класть в наш setup, как HidHide.
- Ключевые файлы:
  - `inc/public.h` — IOCTL, `JOYSTICK_POSITION_V2` (2.1.9), `..._V3` (2.2.x), GUID интерфейса;
  - `apps/common/vJoyInterface.cpp` — эталонные вызовы (`OpenDeviceInterface`, `CreateDevicePath`, `Update`,
    `GetDevStat`, `GetDrvStat`, `ResetVJD`);
  - `apps/vJoyConf/vJoyConfig/vJoyConfig.cpp`, `apps/vJoyConf/vJoyConf.cpp` — `CreateHidReportDesc`, запись в реестр;
  - `apps/vJoyInstall/vJoyInstallLib.cpp` — enable/disable/restart устройства (`DIF_PROPERTYCHANGE`);
  - `driver/` — проверить, как драйвер принимает позицию и как определяет владельца устройства.

## Протокол драйвера (сводка, проверить в сессии 1)

- Интерфейс `GUID_DEVINTERFACE_VJOY = {781EF630-72B2-11D2-B852-00C04FAD5101}`: из пути первого интерфейса
  берётся namespace (`ExtractNamespace`), устройство N открывается как `<namespace>device_NNN`
  (`device_000` — общий канал управления, `device_001..016` — виртуальные джойстики).
- Открытие устройства `GENERIC_READ | GENERIC_WRITE`, `FILE_FLAG_OVERLAPPED` = **захват**: пока хэндл открыт,
  устройство «наше». Чужой захват виден через `GET_DEV_STAT` (статус и PID владельца).
- Коды: `CTL_CODE(FILE_DEVICE_UNKNOWN, F, METHOD_BUFFERED, access)`, F = 0x910…0x920:

| F | IOCTL | Данные |
|---|---|---|
| 0x910 | LOAD_POSITIONS | `JOYSTICK_POSITION_V2` (2.1.9) / V3 (2.2.x) — вся позиция за раз |
| 0x917 | GET_DEV_INFO | id устройства (проверка, что открыли то самое) |
| 0x91A / 0x91B | GET_DRV_DEV_MAX / GET_DRV_DEV_EN | сколько устройств максимум / включено |
| 0x91D | GET_DEV_STAT | статус устройства, PID владельца |
| 0x91E | GET_DRV_INFO | версия драйвера, флаги (FFB) |
| 0x91F | RESET_DEV | сброс в исходное положение |
| 0x920 | GET_POSITIONS | текущая позиция (для проверки) |
| 0x912–0x916 | FFB | вне рамок (Gremlin FFB не использует) |

- Диапазоны осей — из HID caps коллекции vJoy (`HidP_GetSpecificValueCaps`, код HID уже есть в gob-hidhide);
  по умолчанию 0..0x8000.
- Конфигурация: `HKLM\SYSTEM\CurrentControlSet\services\vjoy\Parameters\DeviceNN`, значения
  `HidReportDesctiptor` (именно так, с опечаткой; REG_BINARY) и `HidReportDesctiptorSize` (REG_DWORD).
  После записи драйвер перечитывает описание при перезапуске устройства vJoy (`DIF_PROPERTYCHANGE` / `DICS_PROPCHANGE`).
  Нужны права администратора.

## Версии

- 2.1.9.1 (у пользователя) и 2.2.2.0 (Brunner): одинаковые коды 0x910–0x920, в 2.2.x добавлены
  `F_GET/SET_FFB_PID_DATA` (0x914/0x915) и `JOYSTICK_POSITION_V3`. Какую структуру принимает каждый драйвер
  (проверка размера в `driver/`) — выяснить в сессии 1; версию берём из `GET_DRV_INFO` / HID VersionNumber.
- Поддерживаем обе; по умолчанию в установщик — 2.2.2.0 (Win10/Win11), если он корректно подписан.

## План работ (сессии)

### 1. Чтение (gob-vjoy, только чтение) — сделано 2026-10-17
- [x] `src/direct.rs`: поиск интерфейса, namespace, канал `device_000`, `GET_DRV_INFO` (FFB, max, сколько есть),
      `GET_DEV_STAT` (есть / открыто / PID владельца -> Own/Free/Busy/Missing как у DLL); коды IOCTL с юнит-тестами.
- [x] Устройства и возможности (оси с диапазонами, кнопки, непрерывные/дискретные хатки) и версия — из HID caps
      коллекций vJoy (VID 0x1234, PID 0xBEAD, report id = номер устройства).
- [x] `vjoy_info` печатает оба варианта и сравнивает: на машине пользователя (2.1.9, два устройства)
      вывод драйвера == вывод DLL. Оси 2.1.9: 0..32767. Статус Own/Busy при захвате проверить в сессии 2.

### 2. Вывод — сделано 2026-10-17
- [x] Захват = открытие `device_NNN`; `LOAD_POSITIONS` с `JOYSTICK_POSITION_V2` (108 байт, проверено тестом);
      `RESET_DEV`; освобождение = закрытие. `vJoyInterface.dll` больше не грузится.
- [x] Тот же API `VJoy::acquire -> Device` (reset + release on drop), оси 1..8 как раньше.
- [x] Проверено `monitor` + `vjoy_info --sweep 2`: ось X во всём диапазоне, кнопка 1, хатка 1 по кругу; vJoy 1 не тронут.
      Найдено и исправлено: `RESET_DEV` меняет только копию в драйвере, HID-отчёт уходит лишь на `LOAD_POSITIONS`, —
      после сброса состояние читается (`GET_POSITIONS`) и отправляется, иначе при остановке профиля игра видела
      последнее положение (например, зажатую кнопку).
- [ ] Ручная проверка с пользователем: Activate профиля VKB в Joystick Gobloon, игра / joy.cpl.

### 3. Настройка устройств
- [x] Генератор HID-дескриптора (порт `CreateHidReportDesc` 2.1.9, `src/descriptor.rs`; FFB-шаблон
      `src/ffb_desc.rs` сгенерирован из `hidReportDescFfb.h`) + обратный разбор дескриптора в `DeviceConfig`.
      Юнит-тесты: побайтово равен Device01/02 пользователя (8 осей, 128/127 кнопок, 4 непрерывные хатки), и тест,
      что всё в реестре пересобирается байт в байт. FFB-ветка сверена только по структуре (у пользователя FFB нет). — 2026-10-18
- [x] Запись в реестр и перезапуск устройства vJoy из нашего же exe с правами администратора:
      `JoystickGobloon.exe --vjoy-config ...` через `ShellExecute "runas"` (один UAC, как сейчас с vJoyConfig).
      В крейте готово (`src/configure.rs`): `Change` с аргументами как у vJoyConfig (`<id> -f ...`, `-d <id>`,
      `enable on|off`), `apply` пишет `DeviceNN` и делает `DIF_PROPERTYCHANGE` на `root\VID_1234&PID_BEAD`
      (`DICS_PROPCHANGE` = `devcon restart`; enable/disable — `DICS_ENABLE`/`DICS_DISABLE`). `Repair` из vJoyConfig
      (переустановка драйвера) не нужен: драйвер перечитывает реестр на каждый запрос дескриптора
      (`UpdateCollections`, driver/sys/hid.c). Пример `vjoy_config` перезапускает себя с `--admin` через UAC;
      код 3 = нужна перезагрузка (устройство занято). Проверено 2026-10-18: перезапись Device02 той же конфигурацией —
      UAC, `done`, оба устройства OK, реестр == `vJoyConfig -t`. Изменение подхватывается сразу: Device02 -> 126 кнопок / 0 хаток, драйвер (HID caps) показал то же. В joygob: `--vjoy-config [--result <file>] <args>`
      (до проверки единственного экземпляра, без лога; текст ошибки — через временный файл), окно настройки vJoy
      зовёт `configure::run_elevated`, «нужна перезагрузка» — отдельное сообщение. Проверено в приложении 2026-10-18.
- [x] Чтение текущей конфигурации — из реестра (`descriptor::read_registry`, прав не нужно), и в joygob тоже.

### 4. Переключение и чистка
- [x] Убрать загрузку DLL и запуск `vJoyConfig.exe`; путь из uninstall-ключа больше не нужен
      (наличие драйвера — интерфейс vJoy или служба `vjoy`). `vjoy_info` сверяет драйвер с реестром. — 2026-10-18
- [x] Установщик: задача «Install the vJoy driver», отмечена, только если нет службы `vjoy`; ставит официальный
      `vJoySetup.exe` 2.1.9.1 (github.com/jshafer817/vJoy, подпись Justin Shafer, SHA-256
      F103CED4…074654A закреплён в release.yml); свой мастер vJoy, при тихой установке — `/VERYSILENT`.
      Ссылка на Brunner убрана. Собрать и проверить установщик можно только в CI (локально нет Inno Setup). — 2026-10-18
- [x] CLAUDE.md (строка про gob-vjoy), ROADMAP.
- [ ] Ручная проверка с пользователем: Activate профиля, joy.cpl / vJoy Monitor, настройка устройства.

### 5. FFB (не планируется)
- Gremlin FFB не использует; оставить на потом.

## Открытые вопросы
1. ~~Какой vJoy ставить установщиком~~ — решено 2026-10-18: официальный 2.1.9.1 (jshafer817).
2. ~~Обновлять ли до 2.2.2.0~~ — нет, поддерживаем 2.1.9.x.
3. ~~Нужен ли перезапуск устройства~~ — да, `DICS_PROPCHANGE`; драйвер перечитывает реестр при перезапуске.
4. Дескриптор 2.2.x (Brunner) другой: до 16 осей (Wheel, Accelerator, ... Throttle), `VJOY_AXIS_MAX_VALUE`,
   иной FFB-блок (`FFB_USE_XY_AXES`, ещё одна usage 0xA7). Генератор сейчас только для 2.1.9; для 2.2.x —
   отдельная ветка по версии драйвера, когда решим вопрос 1.
