# gob-hidhide: прямое управление драйвером вместо HidHideCLI

Цель: `gob-hidhide` говорит с драйвером HidHide напрямую через `DeviceIoControl` и больше не запускает
`HidHideCLI.exe`. Драйвер остаётся от Nefarius (свой kernel-драйвер — вне рамок, см. ROADMAP).
**Решение (2026-10-17): нативная среда HidHide не нужна** — ни CLI, ни Configuration Client. Запасного пути
через HidHideCLI не будет; подписанный драйвер при необходимости ставим сами (раздел 6).
Публичный API крейта по возможности не меняется, чтобы `joygob/src/hidhide_view.rs` и `app.rs` почти не трогать.

Источник: `ref/HidHide` (github.com/nefarius/HidHide, master 2b950fd). Ключевые файлы:
- `Shared/HidHideIoctlContract.h` — коды IOCTL
- `DEVELOPER.md` — описание протокола и session blacklist
- `HidHideCLI/src/FilterDriverProxy.cpp` — эталонные вызовы
- `HidHideCLI/src/Volume.cpp` — перевод `C:\...` <-> `\Device\HarddiskVolumeN\...`
- `HidHideCLI/src/HID.cpp` — поиск игровых устройств и их USB-контейнера (`--dev-gaming`)
- `HidHide/src/Logic.c` — как драйвер проверяет списки (в т.ч. суффикс WTS-сессии в blacklist)

## Протокол драйвера (сводка)

- Устройство: `CreateFileW("\\\\.\\HidHide", GENERIC_READ, FILE_SHARE_READ|WRITE|DELETE, OPEN_EXISTING)`.
  `ERROR_FILE_NOT_FOUND` = драйвер не установлен, `ERROR_ACCESS_DENIED` = устройство занято/нет прав.
- Код: `CTL_CODE(32769, N, METHOD_BUFFERED, FILE_READ_DATA)` = `(32769 << 16) | (1 << 14) | (N << 2)`.

| N | IOCTL | Данные |
|---|---|---|
| 2048 / 2049 | GET / SET_WHITELIST | MULTI_SZ UTF-16, пути образов в виде `\Device\HarddiskVolumeN\...` |
| 2050 / 2051 | GET / SET_BLACKLIST | MULTI_SZ, device instance path (`HID\VID_...`), может быть суффикс WTS-сессии |
| 2052 / 2053 | GET / SET_ACTIVE | 1 байт BOOLEAN (cloak) |
| 2054 / 2055 | GET / SET_WLINVERSE | 1 байт BOOLEAN (белый список работает как чёрный) |
| 2056 | ADD_SESSION_BLACKLIST | MULTI_SZ; скрытие живёт, пока жив наш процесс, в реестр не пишется |
| 2057 | CLR_SESSION_BLACKLIST | без данных; снять свои session-записи |

- GET-списки: первый вызов без буфера возвращает нужный размер в `needed`, второй читает.
- SET-списки заменяют список целиком: read-modify-write, сравнивать без учёта регистра.
- Размер MULTI_SZ — чётный, включая оба завершающих нуля.

## Версии драйвера

- У пользователя: HidHide 1.5.230 (последний опубликованный релиз на GitHub, 2024-05), драйвер 1.4.181,
  установлен в `C:\Tools\JoystickHidHide\`.
- IOCTL 2048–2055 есть во всех версиях, 2056/2057 (session blacklist) — с 2026-04-28, тег v1.7.333.0+.
- **Подписанного 1.7.x нет и в ближайшее время не будет**: [nefarius/HidHide#217](https://github.com/nefarius/HidHide/issues/217)
  (открыт 2026-07-01). Мейнтейнеру не хватает времени тестировать, а непроверенный код он EV-подписывать
  не будет; pre-release возможен только test-signed (работает лишь в test mode Windows) — пользователям не
  годится. Последний ответ в issue — 2026-09-02.
- **Решение (2026-10-17): делаем под текущий подписанный драйвер** — только IOCTL 2048–2055, поведение как у
  HidHideCLI 1.5.230. Session blacklist (раздел 5) откладываем до выхода подписанного релиза.
- Там же описан наш случай: CLI 1.5.230 при драйвере 1.4.181 — замена драйвера не завершилась без перезагрузки.
  Версию для UI/лога берём у драйвера (`HidHide.sys`), а не у CLI.
- Поддержку session blacklist определяем на старте пробным `CLR_SESSION_BLACKLIST`:
  `ERROR_INVALID_PARAMETER` / `ERROR_INVALID_FUNCTION` = старый драйвер.

### Отслеживание #217
- Проверять issue и [Releases](https://github.com/nefarius/HidHide/releases) перед каждой сессией по gob-hidhide
  и перед релизом Joystick Gobloon: `gh issue view 217 -R nefarius/HidHide`, `gh release list -R nefarius/HidHide -L 3`.
- Когда выйдет подписанный релиз ≥ 1.7.333: обновить таблицу версий выше, сделать раздел 5, пересмотреть
  исправления из master (буфер MULTI_SZ #211, падение на сломанном перечислении #209, XUSB #210) — нужны ли
  нам обходы для старого драйвера, и поднять минимальную рекомендуемую версию в UI/инсталляторе.

## План работ (сессии)

### 1. IOCTL-ядро в gob-hidhide (сессия 1)
- [ ] `src/driver.rs`: `Driver::open()`, get/set active, inverse, whitelist, blacklist; хелперы MULTI_SZ
      (`to_multi_sz`, `from_multi_sz`) с юнит-тестами (пустой список, один элемент, кириллица в пути).
- [ ] Features `windows`: `Win32_Storage_FileSystem`, `Win32_System_IO`, `Win32_Foundation`.
- [ ] Ошибки: `Error::NotInstalled` (нет устройства), `Error::Driver(String)` вместо `Error::Cli`
      (сообщения в лог по-английски, в UI — существующие i18n-ключи).
- [ ] Пример `hidhide_info` переводим на драйвер: печать active/inverse/whitelist/blacklist. **Только чтение.**

### 2. Пути приложений (сессия 1–2)
- [ ] `src/volume.rs`: `to_image_name(C:\...) -> \Device\HarddiskVolumeN\...` и обратно
      (`FindFirstVolumeW` / `GetVolumePathNamesForVolumeNameW` / `QueryDosDeviceW`, как в Volume.cpp).
- [ ] Тест: путь к нашему exe туда и обратно совпадает; путь на несуществующем томе -> ошибка, а не паника.
- [ ] `apps()` / `set_app()` / `ensure_app()` на новом коде; учесть inverse: при включённом inverse
      свой exe в список **не** добавляем (так делает CLI).

### 3. Поиск устройств вместо `--dev-all` / `--dev-gaming` (сессия 2)
- [ ] Результат — `DeviceGroup` (уже есть в API): коллекции одного USB-контейнера, имя = friendlyName
      контейнера, как строки в HidHide Configuration Client; окно скрытия фильтрует «только игровые».
- [ ] `src/devices.rs`: все HID-интерфейсы, включая отсутствующие (SetupAPI `GUID_DEVINTERFACE_HID`,
      без `DIGCF_PRESENT`) -> `HidDevice` с теми же полями.
- [ ] Признак gaming как в HID.cpp: UsagePage 0x05, или UsagePage 0x01 с Usage 0x04/0x05, плюс Steam Controller/Deck.
- [ ] Имя: `HidD_GetProductString` для присутствующих (мы в whitelist), иначе `DEVPKEY_Device_BusReportedDeviceDesc`
      / `FriendlyName`.
- [ ] Base container: `DEVPKEY_Device_ContainerId` + подъём по родителям до корня контейнера (как `BaseContainerDeviceInstancePath`).
- [ ] Сверка: вывод нового кода == JSON `HidHideCLI --dev-all` на этой машине (одноразовый пример-сравнение).

### 4. Переключение и чистка (сессия 3)
- [ ] `HidHide::find()` открывает драйвер, `HidHideCLI.exe` больше не нужен; `version` берём из реестра как сейчас.
- [ ] Удалить `run()`, `quoted_args()`, разбор JSON и `serde_json` из зависимостей, если больше не нужны.
- [ ] Обновить CLAUDE.md (строка про gob-hidhide) и ROADMAP.
- [ ] Ручная проверка с пользователем: скрыть/показать VKB, joy.cpl, whitelist своего exe. **Состояние HidHide
      меняем только с согласия** (CLAUDE.md, Testing notes).

### 5. Session blacklist: «скрывать только пока работает Joystick Gobloon» (ОТЛОЖЕНО до подписанного 1.7.x, см. #217)
- [ ] Настройка (вкладка в settings_view.rs, ключи en/ru): скрывать выбранные устройства на время работы
      программы / активного профиля.
- [ ] При старте/активации — `ADD_SESSION_BLACKLIST`, при остановке — `CLR_SESSION_BLACKLIST`; при падении
      драйвер сам вернёт устройства. Постоянный blacklist пользователя не трогаем.
- [ ] Не забыть: драйвер скрывает устройство, только когда cloak (`ACTIVE`) включён — проверить в Logic.c
      и в UI предупреждать, если выключен.
- [ ] Старый драйвер: опция неактивна, в tooltip — какая версия HidHide нужна.

### 6. Установка драйвера своими силами (позже, отдельно)
- [ ] Кладём в наш Inno Setup пакет подписанного драйвера 1.5.230 (`HidHide.sys/.inf/.cat`, MIT), без CLI и клиента.
- [ ] Установка как в `ref/HidHide/Setup/install.cmd`: inf для HIDClass / XnaComposite / XboxComposite и
      upper class filter `HidHide` для этих классов (SetupAPI / `DiInstallDriver` + реестр `UpperFilters`,
      либо nefconw, MIT). Удаление — в обратном порядке. Нужна перезагрузка: драйвер NOT_STOPPABLE.
- [ ] Уже установленный HidHide от Nefarius не трогаем; ставим, только если драйвера нет.
- [ ] Путь к драйверу и версию берём не из ключа реестра Nefarius (его не будет), а у самого `HidHide.sys`.

## Интерфейс (сделано 2026-10-17)
- Страница «Программы» в рейле: свой список программ (`settings.apps`) + программы из whitelist HidHide.
  Сейчас у программы одна опция — «видит скрытые устройства» (whitelist); дальше — автоактивация профиля
  по программе и т.п. Скрытие (HidHide) везде опционально.
- Окно «Скрытие устройств» (кнопка-глаз справа в верхнем тулбаре; обычное, не модальное окно — JoyGob в whitelist, скрытие ему не мешает) повторяет Devices из HidHide
  Configuration Client: группа = строка с чекбоксом и замком, внизу «не показывать отключённые»,
  «только игровые» (обе в settings.toml) и «включить скрытие» (cloak).

## Открытые вопросы
1. ~~Session blacklist: сразу или после IOCTL?~~ После — и только когда выйдет подписанный 1.7.x (#217).
2. ~~Где брать 1.7.x?~~ Нигде: подписанной сборки нет, test-signed не используем.
3. ~~Fallback на HidHideCLI?~~ Нет: работаем с драйвером сами, при необходимости сами его ставим.
