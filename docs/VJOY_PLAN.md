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
- `ref/vJoy-brunner` — github.com/BrunnerInnovation/vJoy, **2.2.2.0** (2023-12, Win10/Win11; на него ссылается наш установщик).
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

### 1. Чтение (gob-vjoy, только чтение)
- [ ] `src/driver.rs`: поиск интерфейса, namespace, открытие `device_000`, `GET_DRV_INFO`, `GET_DRV_DEV_MAX/EN`,
      `GET_DEV_STAT` для 1..16; коды IOCTL с юнит-тестами (как в gob-hidhide).
- [ ] Устройства и возможности (оси, кнопки, хатки) — из HID caps коллекций vJoy (VID 0x1234, PID 0xBEAD).
- [ ] Пример `vjoy_info` на новом коде; сверка с текущим выводом через DLL на этой машине.

### 2. Вывод
- [ ] Захват = открытие `device_NNN`; `LOAD_POSITIONS` со структурой нужной версии; `RESET_DEV`; освобождение = закрытие.
- [ ] Тот же API `VJoy::acquire -> Device` (reset + release on drop), оси 1..8 как сейчас.
- [ ] Проверка: `monitor` + `vjoy_info --sweep <id>` (пишет только в виртуальное устройство; запускать с
      согласия пользователя), `runner_check`, профиль VKB в игре.

### 3. Настройка устройств
- [ ] Генератор HID-дескриптора (порт `CreateHidReportDesc`) + юнит-тест: побайтово равен тому, что сейчас
      лежит в реестре для устройств пользователя (чтение реестра — без изменений).
- [ ] Запись в реестр и перезапуск устройства vJoy из нашего же exe с правами администратора:
      `JoystickGobloon.exe --vjoy-config ...` через `ShellExecute "runas"` (один UAC, как сейчас с vJoyConfig).
- [ ] Чтение текущей конфигурации — из реестра/HID caps вместо разбора `vJoyConfig -t`.

### 4. Переключение и чистка
- [ ] Убрать `libloading`, загрузку DLL и запуск `vJoyConfig.exe`; путь из uninstall-ключа больше не нужен
      (наличие драйвера — по службе `vjoy` / интерфейсу).
- [ ] Установщик: проверять драйвер vJoy и ставить подписанный установщик vJoy (SHA-256 в release.yml),
      как HidHide; при необходимости — перезагрузка.
- [ ] CLAUDE.md (строка про gob-vjoy), ROADMAP.
- [ ] Ручная проверка с пользователем: Activate профиля, joy.cpl / vJoy Monitor, настройка устройства.

### 5. FFB (не планируется)
- Gremlin FFB не использует; оставить на потом.

## Открытые вопросы
1. Какой vJoy ставить установщиком: 2.2.2.0 (Brunner, Win11) или 2.1.9.1 (jshafer817)? Проверить подпись
   `vJoySetup_v2.2.2.0_Win10_Win11.exe` и работу на Windows 11 24H2+.
2. Обновлять ли у пользователя 2.1.9.1 → 2.2.2.0, или поддерживать обе версии без обновления.
3. Нужен ли перезапуск устройства после записи конфигурации или драйвер подхватывает её сам.
