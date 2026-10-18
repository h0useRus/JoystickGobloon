# Подсветка джойстиков (LED): можно ли управлять из Joystick Gobloon

## Короткий ответ

Да, но **у каждого производителя свой протокол**. Общего стандарта, который поддерживали бы симуляторные
джойстики, сейчас нет. Поэтому подсветка делается слоем «бэкендов»: по одному на производителя, плюс общий
бэкенд для стандарта HID LampArray на будущее.

| Производитель | Как управляется | Открыто? | Источник |
|---|---|---|---|
| **VKB** (Gladiator EVO/NXT, Gunfighter, STECS…) | HID feature report `0x59`, 129 байт, команда `59 A5 0A`, до 30 LED по 4 байта (id + цвет 3×3 бит), контрольная сумма | Разобран сообществом | [pyvkb](https://pyvkb.readthedocs.io/en/latest/_modules/vkb/led.html), [vkb_led_jg_plugin](https://github.com/WesCov/vkb_led_jg_plugin) (плагин Joystick Gremlin для NXT) |
| **WinWing** (Orion2 и др.) | HID output report 14 байт: заголовок, номер LED, яркость | Драйвер в ядре Linux | [hid-winwing.c](https://next.tangled.org/tjh.dev/kernel/blob/master/drivers/hid/hid-winwing.c), [патч LKML](https://lkml.rescloud.iu.edu/2402.2/01644.html) |
| **Thrustmaster** Warthog throttle | HID report: 5 вкл/выкл + яркость 0..5 | Разобран сообществом | [whlctl](https://github.com/Thesola10/whlctl) |
| **Virpil** (VPC) | Закрытый протокол; сторонние программы вызывают `VPC_LED_Control.exe` из VPC Software | Нет | [virpil-led-helper (форум BMS)](https://forum.falcon-bms.com/post/450968) |
| **Logitech/Saitek** X52 Pro и др. | `DirectOutput.dll` (SDK Saitek/Logitech: LED и MFD) | SDK, не протокол | [форум Frontier](https://forums.frontier.co.uk/goto/post?id=6760986), [x52pro-linux](https://pypi.org/project/x52-control/) |
| **Стандарт HID LampArray** | Windows Dynamic Lighting (`Windows.Devices.Lights`), Windows 10 1809+ (фон — Windows 11) | Да, открытый стандарт | [Microsoft Learn](https://learn.microsoft.com/windows/uwp/devices-sensors/lighting-dynamic-lamparray) |

LampArray в документации Microsoft перечисляет и игровые контроллеры (flightstick), но симуляторные джойстики
с ним пока не попадались — проверять по report descriptor конкретного устройства.

## Что важно до начала

- **Сохранность настроек пользователя.** У VKB подсветка настраивается и в прошивке (VKBDevCfg). Нужно убедиться,
  что команда `0x59` меняет только текущее состояние, а не то, что записано в устройство; иначе — сохранять
  и восстанавливать исходное. Первые записи — только с согласия пользователя.
- **HidHide.** Скрытое устройство открывается только программами из списка: Joystick Gobloon в нём есть,
  поэтому feature/output report он отправить может.
- **Проверять можно только на реальном железе.** У пользователя — VKB Gladiator EVO R и EVO OT L
  (VID 0x231D, PID 0x0200 / 0x3201). Остальные бэкенды — только при наличии тестера с таким устройством.

## Как это встроить

- Новый крейт `gob-lighting`: трейт бэкенда (найти устройство по VID/PID и report descriptor, список LED,
  установить цвет/яркость/режим), реестр бэкендов. HID-код (открытие, feature/output report) общий с gob-hidhide.
- Движок (`gob-engine`): новое действие «Подсветка» (LED, цвет, режим) + привязка к событиям: смена mode,
  профиль активен/остановлен, нажатие кнопки, положение оси. Новое действие — не из Gremlin: в профиль Gremlin
  не экспортируется (или как raw XML), отметить в `docs/GREMLIN_ACTIONS.md`.
- UI: иконка-лампочка на вкладке устройства, если для него есть бэкенд; редактор действия; общие настройки
  (вкл/выкл подсветку программой) — во вкладке настроек.

## Предварительный план действия «Подсветка» (решено 2026-10-17)

Действие по образцу `play-sound`: зажечь **определённый светодиод определённым цветом**. Первым — только VKB,
чтобы отладить. Если устройство не поддерживается (нет бэкенда, не подключено), действие **молча игнорируется**
(одна строка в лог при активации профиля, без диалогов).

| Слой | Что | По образцу play-sound |
|---|---|---|
| gob-core | `ActionKind::SetLed { device, led, color }`, тип `"set-led"`; `NewAction::SetLed` в edit.rs | `ActionKind::PlaySound`, `NewAction::PlaySound` |
| Формат профиля | Своё действие, в Gremlin его нет: пишется как обычный элемент `<action type="set-led">` с атрибутами `device` (GUID), `led` (номер), `color` (`#RRGGBB`); Gremlin его пропустит, наш импорт прочитает | — |
| gob-engine | `Node::SetLed`, активация на нажатие (как play-sound); метод `Output::set_led(device, led, color)` с пустой реализацией по умолчанию | `Node::PlaySound`, `Output::play_sound` |
| Runner | Вызывает `gob-lighting` в фоновом потоке (HID-запрос не должен тормозить цикл движка) | `media.rs` |
| gob-lighting | Бэкенд VKB: найти устройство по GUID/VID/PID, отправить feature report `0x59`; неподдерживаемое — `Ok(false)` | — |
| joygob | Пункт «Подсветка» в меню добавления действия (иконка лампочки), редактор: устройство (из подключённых), номер LED, цвет (палитра VKB: 8 уровней на канал); i18n en/ru | редактор play-sound |
| docs | Строка `set-led` в `docs/GREMLIN_ACTIONS.md` как своё действие | — |

Вопросы на сессию 0: нумерация LED у VKB EVO (что значит «LED 0, 1, …» на стике), режимы (постоянно/мигание),
нужно ли «погасить» при деактивации профиля.

## План работ (сессии)

### 0. Разведка на VKB пользователя (только чтение)
- [ ] Пример `led_probe`: report descriptor и список feature report ID у обоих VKB; есть ли `0x59`.
- [ ] Чтение текущей конфигурации LED (get-команда pyvkb), сравнение с тем, что показывает VKBDevCfg.
- [ ] Выяснить, временная ли запись `0x59` (до переподключения) или пишется во flash. **Без записи.**

### 1. gob-lighting + бэкенд VKB
- [ ] Крейт, трейт, бэкенд VKB по pyvkb (структура LED, цвет 3×3 бит, контрольная сумма) с юнит-тестами
      на кодирование пакета (эталонные байты из pyvkb).
- [ ] Пример: зажечь один LED и вернуть исходное — запускать только с согласия пользователя.

### 2. Движок и UI
- [ ] Действие «Подсветка» в gob-engine, события mode/активация/кнопка; редактор в action_editor.
- [ ] Лампочка на вкладке устройства; i18n en/ru.

### 3. Другие производители (по запросу, нужен тестер)
- [ ] WinWing — по `hid-winwing.c`; Thrustmaster Warthog — по whlctl.
- [ ] Virpil — запуск `VPC_LED_Control.exe` (если установлен VPC Software) или разбор протокола по USB-дампу.
- [ ] Logitech/Saitek — через `DirectOutput.dll`, если установлен.

### 4. HID LampArray (общий)
- [ ] Бэкенд через Windows Dynamic Lighting / HID LampArray для любых устройств со стандартной подсветкой.

## Открытые вопросы
1. Протокол VKB EVO = NXT/Gladiator K? pyvkb писался под Gladiator K, плагин Gremlin — под NXT.
2. Нужна ли подсветка как действие профиля (как плагин Gremlin) или достаточно реакции на mode/активацию.
3. Какие ещё устройства есть у пользователей Joystick Gobloon — от этого зависит порядок сессии 3.
