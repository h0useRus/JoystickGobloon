language-name = Русский

# Navigation and toolbar tooltips
nav-devices = Устройства и назначения
nav-hidhide = Скрытие устройств
nav-vjoy = Виртуальные джойстики
nav-settings = Настройки
action-open = Импорт профиля Joystick Gremlin
action-save = Сохранить
action-report = Отчёт об импорте
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
device-linked = Назначения устройства профиля с тем же именем. Нажмите «Замена устройств», чтобы сохранить связь.
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
import-title = Отчёт об импорте
import-summary = Версия формата { $version }: входов — { $inputs }, действий — { $actions }, устройств — { $devices }, режимов — { $modes }.
import-no-warnings = Проблем не найдено.
warn-dangling = Нет действия { $to } (ссылка из { $from })
warn-unknown-mode = Входы используют необъявленный режим «{ $mode }»
warn-not-executable = «{ $kind }» × { $count }: сохраняется в профиле, но пока не выполняется
warn-scripts = В профиле есть Python-плагины: они сохраняются, но не запускаются

# Device substitution
subst-title = Замена устройств
subst-banner = ID устройств в профиле не совпадают с этим ПК: { $count }
subst-keep = Оставить как есть
subst-apply = Применить
status-substituted = Устройства заменены. Сохраните профиль, чтобы закрепить изменения.
profile-modified = Есть несохранённые изменения

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
modes-title = Режимы
modes-name = Название
modes-parent = Наследует от
modes-startup = Режим при запуске
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
