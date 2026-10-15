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
device-disconnected = Не подключено
no-devices-attached = Игровые контроллеры не найдены
placeholder-hidhide = Скрытие физических устройств от игр (драйвер HidHide).
placeholder-vjoy = Настройка виртуальных джойстиков (драйвер vJoy).

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
subst-banner = Не подключено устройств профиля: { $count }
subst-keep = Оставить как есть
subst-apply = Применить
status-substituted = Устройства заменены. Сохраните профиль, чтобы закрепить изменения.
profile-modified = Есть несохранённые изменения
