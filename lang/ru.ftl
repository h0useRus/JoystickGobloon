language-name = Русский

# Navigation and toolbar tooltips
nav-devices = Устройства и назначения
nav-hidhide = Скрытие устройств
nav-vjoy = Виртуальные джойстики
nav-settings = Настройки
action-open = Импорт профиля Joystick Gremlin
action-save = Сохранить как профиль Joystick Gremlin
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
