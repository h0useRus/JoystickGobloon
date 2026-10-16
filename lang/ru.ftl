language-name = Русский

# Navigation and toolbar tooltips
nav-devices = Устройства и назначения
nav-hidhide = Скрытие устройств
nav-vjoy = Виртуальные джойстики
nav-settings = Настройки
action-open = Импорт профиля Joystick Gremlin
action-save = Сохранить
label-mode = Режим

# Settings
settings-appearance = Внешний вид
settings-general = Общие
settings-about = О программе
settings-language = Язык
settings-reopen-last = Открывать последний профиль при запуске
theme-light = Светлая
theme-dark = Тёмная
about-text = Скрытие устройств, виртуальные джойстики и назначения в одной программе.

# Empty states
no-profile = Профиль не загружен
device-linked = Назначения устройства профиля с тем же именем. Нажмите «Сопоставить контроллеры», чтобы сохранить связь.
device-disconnected = Не подключено
no-devices-attached = Игровые контроллеры не найдены

# Status bar
status-ready = Готово
status-loaded = Загружен { $file }
status-saved = Сохранён { $file }
import-error = Не удалось открыть профиль: { $error }
save-error = Не удалось сохранить профиль: { $error }
file-filter-gremlin = Профиль Joystick Gremlin

# Import report
alert-error = Что-то пошло не так
alert-import = Часть действий в { $file } не будет работать
alert-open-log = Открыть журнал
ok = ОК
hidhide-whitelist-error = Не удалось добавить JoyGob в белый список HidHide: { $error }
warn-not-executable = «{ $kind }» × { $count }: сохраняется в профиле, но пока не выполняется
warn-scripts = В профиле есть Python-плагины: они сохраняются, но не запускаются

# Map controls (device substitution)
subst-title = Сопоставить контроллеры
subst-auto = Авто
subst-auto-tip = Сопоставить оставшиеся устройства с подключёнными по похожему имени; неоднозначные останутся на ваш выбор
subst-banner = ID устройств в профиле не совпадают с этим ПК: { $count }
subst-keep = Оставить как есть
subst-apply = Применить
status-substituted = Контроллеры сопоставлены. Сохраните профиль, чтобы закрепить изменения.

# vJoy
vjoy-missing = vJoy не установлен или не загружается.
vjoy-enabled = Драйвер включён
vjoy-disabled = Драйвер выключен
vjoy-no-devices = Устройства vJoy не настроены. Добавьте их в «Configure vJoy».
vjoy-axes = Оси
vjoy-buttons = Кнопки
vjoy-hats = Хатки
vjoy-test = Проверка вывода: взять управление устройством
vjoy-test-hint = Нажмите на колбу, чтобы взять управление устройством и проверить его вывод.
vjoy-free = Свободно
vjoy-own = Управляется JoyGob
vjoy-busy = Занято другой программой (например, Joystick Gremlin)
vjoy-unknown = Неизвестное состояние
vjoy-error = Не удалось взять устройство: { $error }

# Device hiding
hidhide-missing = HidHide не установлен.
hidhide-toggle = Включить или выключить скрытие устройств
hidhide-on = Скрытие включено
hidhide-off = Скрытие выключено
hidhide-devices = Устройства
hidhide-apps = Программы
hidhide-hidden = Скрыто: его видят только программы из белого списка
hidhide-visible = Видно всем программам
hidhide-add-app = Добавить программу
hidhide-remove-app = Убрать из белого списка
hidhide-app-missing = Файл не найден
action-refresh = Обновить
settings-hidhide-self = Добавлять JoyGob в белый список HidHide при запуске
status-whitelisted = JoyGob добавлен в белый список HidHide.

# Engine
action-activate = Активировать профиль
action-deactivate = Остановить профиль
title-active = Активен
title-inactive = Неактивен
status-active = Профиль активен
status-stopped = Профиль остановлен
activate-error = Не удалось активировать профиль: { $error }

# Editing
edit-add = Добавить действие
edit-remove = Удалить
edit-up = Выше
edit-down = Ниже
edit-not-editable = Это действие сохраняется в профиле, но пока не редактируется.
edit-on-press = При нажатии
edit-on-release = При отпускании
edit-on-both = При нажатии и отпускании
edit-inverted = Инвертировать
edit-threshold = Порог долгого нажатия
edit-short = Короткое нажатие
edit-long = Долгое нажатие
edit-deadzone = Мёртвая зона
edit-invert = Инвертировать
edit-inherited = Унаследовано из режима «{ $mode }»
edit-unbind = Удалить все действия этого входа
edit-override = Переопределить в этом режиме
input-axis-kind = Ось
input-button-kind = Кнопка
input-hat-kind = Хатка
kind-map-to-vjoy = Назначить на vJoy
kind-response-curve = Кривая отклика
kind-change-mode = Смена режима
kind-tempo = Короткое / долгое нажатие
kind-description = Описание

# Files and modes
action-new = Новый профиль
action-save-as = Сохранить как…
status-new = Новый профиль
unsaved-title = Несохранённые изменения
unsaved-text = Сохранить изменения в { $file }?
unsaved-save = Сохранить
unsaved-discard = Не сохранять
cancel = Отмена
modes-edit = Изменить режимы
modes-title = Режимы
modes-name = Название
modes-parent = Наследует от
modes-remove = Удалить режим и его назначения
modes-new = Новый режим
modes-add = Добавить режим

# Undo
action-undo = Отменить (Ctrl+Z)
action-redo = Повторить (Ctrl+Y)
status-undone = Отменено
status-redone = Повторено
edit-curve-help = Тяните точку, чтобы сдвинуть · Клик по пустому месту — инверсия · Двойной клик — новая точка · Правый клик по точке — удалить
edit-curve-select = Кликните по точке, чтобы изменить её координаты.

# vJoy mapping
vjoy-device-n = vJoy устройство { $n }
vjoy-axis-1 = Ось X
vjoy-axis-2 = Ось Y
vjoy-axis-3 = Ось Z
vjoy-axis-4 = Вращение X
vjoy-axis-5 = Вращение Y
vjoy-axis-6 = Вращение Z
vjoy-axis-7 = Ползунок
vjoy-axis-8 = Колёсико
vjoy-button-n = Кнопка { $n }
vjoy-hat-n = Хатка { $n }
vjoy-absolute = Абсолютная
vjoy-relative = Относительная
vjoy-speed = Скорость

# Mouse
kind-map-to-mouse = Назначить на мышь
mouse-mode-button = Кнопка мыши
mouse-mode-motion = Движение курсора
mouse-left = Левая кнопка
mouse-right = Правая кнопка
mouse-middle = Средняя кнопка
mouse-forward = Кнопка «Вперёд»
mouse-back = Кнопка «Назад»
mouse-wheel-up = Колесо вверх
mouse-wheel-down = Колесо вниз
mouse-direction = Направление
mouse-min-speed = Минимальная скорость
mouse-max-speed = Максимальная скорость
mouse-time-to-max = Время разгона

# Toggle, split, pause, run
kind-smart-toggle = Умный переключатель
kind-split-axis = Разделить ось
kind-pause-resume = Пауза / продолжить
kind-run-command = Запустить программу
edit-delay = Удержание для режима «пока нажато»
edit-split = Разделить в точке
edit-lower = Нижняя часть
edit-upper = Верхняя часть
pause-op-pause = Пауза
pause-op-resume = Продолжить
pause-op-toggle = Переключить
run-executable = Программа
run-arguments = Аргументы
run-browse = Выбрать программу
status-paused = Профиль на паузе

# Double tap, chain
kind-double-tap = Двойное нажатие
kind-chain = Цепочка
edit-tap-threshold = Время до второго нажатия
edit-tap-exclusive = Раздельно: одиночное нажатие ждёт, не будет ли второго
edit-tap-combined = Вместе: двойное нажатие выполняет и действия одиночного
edit-single = Одиночное нажатие
edit-double = Двойное нажатие
edit-chain-timeout = Начать с первого шага после такой паузы (0 = никогда)
edit-chain-step = Шаг { $n }

# Hat as buttons
kind-hat-buttons = Хатка как кнопки
edit-hat-four = 4 направления
edit-hat-eight = 8 направлений (с диагоналями)
hat-north = Север
hat-north-east = Северо-восток
hat-east = Восток
hat-south-east = Юго-восток
hat-south = Юг
hat-south-west = Юго-запад
hat-west = Запад
hat-north-west = Северо-запад
hat-center = Центр

# Merge axis
kind-merge-axis = Объединение осей
edit-reuse = Добавить сюда и это действие (общее, как в Gremlin для обеих осей)
edit-merge-name = Название
edit-merge-no-device = (нет устройства)
edit-merged = Объединённая ось
merge-average = Среднее
merge-minimum = Минимум
merge-maximum = Максимум
merge-sum = Сумма
merge-bidirectional = Двунаправленное (ось 2 минус ось 1)
merge-maximum-deflection = Наибольшее отклонение
merge-closest-to-center = Ближайшее к центру

# Axis delta
kind-axis-delta = Шаг оси
edit-delta-threshold = Изменение оси, после которого срабатывает импульс
edit-delta-positive = Ось увеличилась
edit-delta-negative = Ось уменьшилась

# Dual axis deadzone
kind-dual-axis-deadzone = Мёртвая зона двух осей
edit-dual-inner = Внутренняя мёртвая зона (круг вокруг центра)
edit-dual-outer = Внешний предел (отсюда полное отклонение)
edit-dual-close = Внутренняя и внешняя зоны слишком близки.
edit-dual-first = Выход первой оси
edit-dual-second = Выход второй оси

# Existing actions (Gremlin's reference action)
edit-link-existing = Связать с существующим действием
edit-copy-existing = Копировать существующее действие

# Condition
kind-condition = Условие
edit-cond-all = Должны выполняться все условия
edit-cond-any = Достаточно любого условия
edit-cond-add = Условие
edit-cond-true = Если да
edit-cond-false = Иначе
cond-current = Этот вход
cond-joystick = Вход джойстика
cond-vjoy = Выход vJoy
cond-keyboard = Зажатые клавиши
cond-range = Ось в диапазоне
cond-pressed = Нажата
cond-released = Отпущена

# Sound, speech, load profile
kind-play-sound = Воспроизвести звук
kind-text-to-speech = Озвучить текст
kind-load-profile = Загрузить профиль
file-filter-sound = Звуковой файл
edit-volume = Громкость
edit-file-missing = Файл не найден
edit-tts-hint = Текст для озвучивания; $current_mode заменяется активным режимом
edit-tts-rate = Скорость
edit-tts-pitch = Высота голоса
edit-tts-back = В конец очереди
edit-tts-front = Следующим (в речи Windows — как в конец очереди)
edit-tts-interrupt = Прервать текущую речь
edit-preview = Прослушать
edit-tts-voice = Голос
edit-tts-default-voice = Голос по умолчанию
edit-tts-voice-missing = не установлен
kind-map-to-keyboard = Назначить клавиши

# Key recording
keys-record = Записать клавиши: нажмите их вместе, запись закончится, когда все будут отпущены
keys-press = Нажмите клавиши…
keys-none = Нет клавиш
keys-cancel = Отменить запись
edit-chain-add = Добавить шаг
edit-chain-remove = Удалить шаг и его действия
kind-macro = Макрос

# Macro editor
macro-repeat = Повтор
macro-repeat-single = Один раз
macro-repeat-count = Несколько раз
macro-repeat-hold = Пока нажата
macro-repeat-toggle = До повторного нажатия
macro-repeat-count-tip = Сколько раз
macro-repeat-delay = Пауза между повторами
macro-add-step = Шаг
macro-step-key = Клавиша
macro-step-pause = Пауза
macro-step-mouse-button = Кнопка мыши
macro-step-mouse-motion = Движение мыши
macro-step-vjoy-button = Кнопка vJoy
macro-step-vjoy-axis = Ось vJoy
macro-step-vjoy-hat = Хатка vJoy
macro-press = Нажать
macro-release = Отпустить

# Logical device
logical-device = Логическое устройство
logical-device-tip = Виртуальные входы профиля: назначьте на них входы, затем настройте их как джойстик
logical-add-axis = Добавить ось
logical-add-button = Добавить кнопку
logical-add-hat = Добавить хатку
logical-remove = Удалить вход и его назначения
logical-empty = Логических входов пока нет: добавьте кнопками выше.
kind-map-to-logical-device = Назначить на логическое устройство
cond-logical = Вход логического устройства

# Tray and startup
settings-tray = Показывать значок в области уведомлений
settings-close-to-tray = Кнопка закрытия прячет JoyGob в область уведомлений
settings-autostart = Запускать вместе с Windows (свёрнутым)
settings-activate-on-start = Активировать профиль при запуске
tray-show = Показать Joystick Gobloon
tray-quit = Выход
autostart-error = Не удалось изменить автозапуск: { $error }

# About
about-alpha = альфа-версия
about-created = Автор: { $name }
about-support = Поддержать разработку
about-support-tip = Внести вклад через PayPal
about-source = Исходный код
about-issues = Сообщить о проблеме
about-releases = Выпуски
about-credits = Использует драйверы HidHide и vJoy; профили совместимы с Joystick Gremlin.
about-license = Свободная программа с открытым кодом под лицензией MIT.
