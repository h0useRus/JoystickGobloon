# Joystick Gobloon — план разработки

## Context
Сейчас используются три программы: HidHide (скрытие устройств), vJoy (виртуальный джойстик), Joystick Gremlin (маппинг/макросы, профили Star Citizen, напр. Subs-Curated-Bindings VKB Dual Gladiator NXT). Цель — одна программа на Rust с современным GUI (egui), которая заменяет UI и логику всех трёх, **используя существующие драйверы HidHide и vJoy** (свои kernel-драйверы требуют EV-сертификата — вне рамок). Требования: UX, знакомый пользователям Gremlin; 100% импорт профилей Gremlin; английский по умолчанию + языковые файлы; темы Light/Dark. Бюджет — подписка Pro, поэтому план разбит на маленькие независимые сессии.

### Что показал анализ целевого профиля (`...[R15].xml`)
- Формат **Gremlin R14+** (`<profile version="14">`): `<inputs>` (устройство по DirectInput GUID → input-type button/axis/hat → mode → root action-id), `<library>` (действия по UUID с вложенными `<actions>` и `<property type=...>`), `<modes>`, `<settings>` (`startup-mode`, `macro-default-delay`), `<logical-device/>`.
- Типы действий: `root`, `map-to-vjoy`, `change-mode` (Temporary/Cycle), `response-curve` (PiecewiseLinear), `tempo`, `description`. Свойство `activation-mode`.
- **Python-плагинов нет** → профиль импортируется полностью без плагинов. Плагины: делаем только API/точку расширения, без реализации.
- Старый формат R13 (version ≤ 13) не поддерживается: только R14+.

## Архитектура (Cargo workspace)
```
JoystickGobloon/
  Cargo.toml                (workspace)
  CLAUDE.md                 (краткие правила проекта для ИИ — экономия токенов)
  crates/
    gob-core/      модель профиля (Profile, Mode, Input, Action enum, Property), без зависимостей от Windows
    gob-gremlin/   парсер/сериализатор Gremlin XML (quick-xml + serde), R14+
    gob-input/     чтение устройств: DirectInput8 через crate `windows` (GUID совпадают с Gremlin)
    gob-vjoy/      драйвер vJoy напрямую (IOCTL + реестр, docs/VJOY_PLAN.md): вывод осей/кнопок/хаток, конфиг устройств
    gob-hidhide/   IOCTL к \\.\HidHide: blacklist/whitelist, cloak on/off, авто-whitelist JoystickGobloon.exe
    gob-engine/    runtime: event loop (отдельный поток), режимы, исполнение дерева действий, кривые, tempo, макросы
    gob-plugin/    trait `ActionPlugin` + регистрация (API для будущих плагинов; реализаций нет)
    gob-i18n/      Fluent (`fluent-bundle`), файлы `lang/en.ftl` (встроен) + внешние `lang/*.ftl`
    joygob/        bin: eframe/egui GUI → JoystickGobloon.exe
  lang/en.ftl, lang/ru.ftl
  tests/fixtures/  реальные профили Gremlin (включая VKB NXT R15)
```
Ключевые решения:
- **Action** — enum в `gob-core` + вариант `Unknown { type, raw_xml }`, чтобы неизвестные действия **сохранялись при round-trip** (гарантия «100%»: import→export даёт эквивалентный XML).
- Engine общается с GUI через `crossbeam-channel`; GUI никогда не блокирует ввод.
- Настройки приложения — `serde` + TOML в `%APPDATA%\JoystickGobloon\`.
- Тема: `egui::Visuals::light()/dark()` + переключатель в меню, сохраняется в настройках.

## GUI (знакомый пользователям Gremlin)
- Вкладки устройств сверху (как Gremlin) + отдельные вкладки **"Devices / HidHide"** и **"vJoy"**.
- Слева — список входов устройства (Axis/Button/Hat) с подсветкой при нажатии в реальном времени; справа — дерево действий выбранного входа (root → дочерние), редакторы для каждого типа.
- Селектор режима (mode) в тулбаре, кнопка Activate (как в Gremlin), иконка в трее.
- Редактор кривой отклика (точки, перетаскивание) — `egui_plot`.
- Итоги импорта и технические предупреждения — в лог-файл `%APPDATA%\JoystickGobloon\joygob.log`; пользователю — только важные предупреждения/ошибки в диалоге.

## Фазы (каждая = 1–3 короткие сессии Claude)
| # | Фаза | Результат / проверка |
|---|------|----------------------|
| 0 | Скелет workspace, CLAUDE.md, eframe окно, Light/Dark, Fluent i18n (en/ru) | `cargo run` → окно, переключение темы и языка |
| 1 | `gob-core` модель + `gob-gremlin` парсер R14 | unit-тест: парсинг VKB NXT R15, round-trip XML эквивалентен |
| 2 | `gob-input`: перечисление устройств DirectInput, live-состояние | вкладка устройств показывает оси/кнопки VKB в реальном времени |
| 3 | `gob-vjoy`: FFI, вывод осей/кнопок/hat | тест-кнопка в GUI двигает ось vJoy (проверка в joy.cpl) |
| 4 | `gob-hidhide`: список устройств, скрыть/показать, whitelist | скрытый VKB исчезает из joy.cpl, но виден Joystick Gobloon |
| 5 | `gob-engine`: root, map-to-vjoy, change-mode, response-curve, tempo, description, activation-mode | загрузить VKB-профиль → Star Citizen видит vJoy с корректными биндами |
| 6 | GUI-редакторы профиля (дерево действий, кривые, режимы), сохранение в Gremlin-XML | редактирование + сохранение, открывается в самом Gremlin |
| 7 | Остальные действия Gremlin R14 (chain, macro, map-to-keyboard/mouse, condition, smart-toggle, split/merge-axis, hat-buttons, load-profile и т.д.) | fixture-тест на каждый тип |
| 8 | `gob-plugin` API (trait + реестр, загрузка dll через `libloading` — заготовка) | пример-заглушка компилируется |
| 9 | Трей, автозапуск, инсталлятор (cargo-wix/Inno Setup с ссылками на HidHide/vJoy) | установка на чистую систему |

MVP для замены всех трёх программ на вашем профиле = фазы 0–5 (~8–12 сессий).

## Стратегия экономии лимита Pro
1. **CLAUDE.md** (≤40 строк): структура crates, правила кода, команды `cargo test -p <crate>`. Это заменяет повторные объяснения.
2. Одна сессия = одна подзадача фазы; после её завершения — `/clear`. Не просить «сделай всё».
3. Давать ИИ точные файлы/crate («работай только в crates/gob-vjoy»), не просить сканировать весь проект.
4. Рутинные задачи (тесты, i18n-строки, мелкие UI-правки) — модель Sonnet; Opus — для engine и парсера.
5. Скачивать исходники Gremlin/HidHide/vJoy локально (`git clone` в `ref/`, в .gitignore) и указывать конкретный файл — дешевле, чем веб-поиск.
6. Проверку (cargo build/test/clippy) запускать самим; ИИ показывать только ошибки.
7. Коммит после каждой зелёной фазы — откат без затрат токенов.

## Верификация
- `cargo test --workspace` — парсер на реальных fixtures (round-trip), модульные тесты кривых/режимов.
- Ручная: joy.cpl / vJoy Monitor — вывод vJoy; joy.cpl — эффект HidHide; Star Citizen — профиль VKB NXT работает идентично Gremlin.
- Сравнение: запустить Gremlin и Joystick Gobloon поочерёдно с тем же профилем, сверить значения в vJoy Monitor.

## Первый шаг после одобрения
Фаза 0: `git init`, workspace, CLAUDE.md, окно eframe с темами и Fluent (en/ru).
