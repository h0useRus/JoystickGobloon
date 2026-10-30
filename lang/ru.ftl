language-name = Русский

# Navigation and toolbar tooltips
nav-devices = Устройства и назначения
nav-hidhide = Скрытие устройств
nav-apps = Программы
nav-vjoy = Виртуальные джойстики
nav-viewer = Просмотр входов
nav-calibration = Калибровка
nav-settings = Настройки
action-open = Импорт профиля Joystick Gremlin
action-save = Сохранить
label-mode = Режим

# Settings
settings-appearance = Внешний вид
settings-general = Общие
settings-about = О программе

settings-lighting = Подсветка
settings-leds-hint = Когда профиль, менявший подсветку джойстика, деактивируется, ставятся эти цвета. Джойстик держит изменённые цвета до переподключения, поэтому вернуть его собственные настройки подсветки нельзя.
settings-leds-none = Нет подключённых джойстиков с управляемой подсветкой.
settings-leds-add = Добавить светодиод
settings-language = Язык
settings-reopen-last = Открывать последний профиль при запуске
theme-light = Светлая
theme-dark = Тёмная
theme-system = Как в системе
theme-system-tip = Светлая или тёмная, как выбрано в системе
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
hidhide-whitelist-error = Не удалось добавить Joystick Gobloon в белый список HidHide: { $error }
warn-not-executable = «{ $kind }» × { $count }: сохраняется в профиле, но пока не выполняется
warn-scripts = В профиле есть Python-плагины: они сохраняются, но не запускаются

# Map controls (device substitution)
subst-title = Сопоставить контроллеры
subst-auto = Авто
subst-auto-tip = Сопоставить оставшиеся устройства с подключёнными по похожему имени; неоднозначные останутся на ваш выбор
subst-banner = ID устройств в профиле не совпадают с этим ПК: { $count }
virtual-banner = Профиль использует виртуальные устройства, которых в системе нет или они настроены скромнее: { $devices }
virtual-setup = Настроить
virtual-setup-tip = Создать недостающие виртуальные устройства и добавить существующим то, чего им не хватает. Ничего не удаляется.
virtual-setup-admin = Создать недостающие виртуальные устройства и добавить существующим то, чего им не хватает. Ничего не удаляется. Для каждого устройства потребуются права администратора.
virtual-setup-active = Остановите профиль, чтобы изменить виртуальные устройства.
virtual-setup-done = Виртуальные устройства настроены для профиля.
access-banner = Joystick Gobloon нужны права на чтение игровых контроллеров и создание виртуальных устройств.
access-grant = Выдать доступ
access-granted = Доступ к устройствам выдан.
access-error = Не удалось выдать доступ к устройствам: { $error }
keyboard-access-banner = Для привязок клавиатуры нужно разрешение читать клавиатуры. Тогда любая запущенная вами программа сможет читать то, что вы печатаете.
keyboard-access-allow = Разрешить ввод с клавиатуры
keyboard-access-remove = Убрать доступ к клавиатуре
keyboard-access-allowed = Ввод с клавиатуры разрешён.
keyboard-access-removed = Доступ к клавиатуре убран. Он закончится после переподключения клавиатуры или перезагрузки компьютера.
keyboard-access-error = Не удалось изменить доступ к клавиатуре: { $error }
all-buttons-banner = У { $device } { $total ->
    [one] { $total } кнопка
    [few] { $total } кнопки
   *[other] { $total } кнопок
}, но Linux передаёт только первые { $shown }. После разрешения программы, читающие устройство напрямую (например, Steam), тоже смогут читать этот стик, даже когда он скрыт.
all-buttons-allow = Читать все кнопки
all-buttons-allowed = Теперь читаются все кнопки стика.
all-buttons-error = Не удалось разрешить чтение всех кнопок: { $error }
subst-keep = Оставить как есть
subst-apply = Применить
status-substituted = Контроллеры сопоставлены. Сохраните профиль, чтобы закрепить изменения.

# vJoy
vjoy-missing = vJoy не установлен или не загружается.
vjoy-no-access = Нет прав на создание устройств vJoy.
vjoy-enabled = Драйвер включён
vjoy-disabled = Драйвер выключен
vjoy-no-devices = Устройства vJoy ещё не настроены.
vjoy-axes = Оси
vjoy-buttons = Кнопки
vjoy-hats = Хатки
vjoy-test-hint = Проверка вывода: заблокируйте устройство, чтобы пользоваться элементами ниже.
vjoy-test-pending = Примените изменения, чтобы проверить их.
vjoy-lock = Взять устройство в монопольное управление для проверки вывода
vjoy-unlock = Отпустить устройство
vjoy-free = Свободно
vjoy-own = Управляется Joystick Gobloon
vjoy-busy = Занято другой программой или другой копией Joystick Gobloon
vjoy-unknown = Неизвестное состояние
vjoy-error = Не удалось взять устройство: { $error }

# Device hiding
hidhide-missing = HidHide не установлен.
hidhide-on = Скрытие устройств включено
hidhide-off = Скрытие устройств выключено
hidhide-hidden = Скрыто: его видят только программы из белого списка
hidhide-visible = Видно всем программам
hidhide-filter-disconnected = Не показывать отключённые устройства
hidhide-gaming-only = Только игровые устройства
hidhide-enable = Включить скрытие устройств
hidhide-confirm-on-title = Включить скрытие устройств?
hidhide-confirm-on-text = Скрытые устройства увидят только программы из белого списка.
hidhide-confirm-off-title = Выключить скрытие устройств?
hidhide-confirm-off-text = Все программы снова увидят все устройства, в том числе скрытые.
hidhide-confirm-hint = Двойной щелчок по глазу переключает без этого вопроса.
action-turn-on = Включить
action-turn-off = Выключить
hidhide-hint = Нажмите на глаз, чтобы скрыть устройство от других программ (закрытый глаз). Затем переподключите его, чтобы изменение вступило в силу.
hidhide-exposed = Всё ещё видно программам, которые читают устройство напрямую (hidraw), например Steam или Wine/Proton: чтобы закрыть его, нужен пароль администратора, а он не был введён. Покажите и снова скройте устройство, чтобы запрос появился ещё раз.
hidhide-no-devices = Нет устройств для показа.
action-refresh = Обновить
settings-see-hidden = Видеть скрытые устройства
settings-see-hidden-tip = При запуске программа добавляет себя к программам, которые видят скрытые устройства, чтобы профили продолжали работать.
settings-group-startup = Запуск
settings-group-devices = Устройства
settings-group-editor = Редактор
settings-focus-input = Следовать за используемым вводом
settings-focus-input-tip = Когда используется ось, кнопка или хатка, страница «Устройства и назначения» выбирает вкладку её устройства и прокручивает список к ней.
settings-focus-skip-modifiers = Не переходить к модификаторам
settings-focus-skip-modifiers-tip = Нажатие кнопки или клавиши-модификатора не прокручивает к ней список, и настраиваемый вход остаётся на виду.
settings-input-events = Считывать контроллеры по изменению
settings-input-events-tip = Меньше задержка и нагрузка на процессор: контроллеры считываются, когда сообщают об изменении, и короткие нажатия кнопок не теряются. Выкл.: каждый контроллер опрашивается с интервалом опроса.
settings-poll-interval = Интервал опроса
settings-poll-interval-tip = Как часто опрашивается контроллер: все контроллеры, если «Считывать контроллеры по изменению» выключено, иначе только те, что не умеют сообщать об изменениях. Меньше — ниже задержка и чуть больше нагрузка на процессор.
settings-poll-interval-unused = Сейчас не используется: все подключённые контроллеры сообщают об изменениях, поэтому ни один не опрашивается. Действует, если «Считывать контроллеры по изменению» выключено или контроллер не умеет сообщать об изменениях.
settings-monotonic-curves = Кривые только растут или падают
settings-monotonic-curves-tip = Редактор кривой отклика: точка или рычаг останавливается там, где кривая повернула бы назад, поэтому она пересекает ноль только один раз. Уже немонотонные кривые правятся свободно.
settings-curve-right-click-invert = Правый щелчок инвертирует кривую
settings-curve-right-click-invert-tip = Редактор кривой отклика: правый щелчок по пустому месту инвертирует кривую. Кнопка «Инвертировать» работает всегда.
settings-warn-gremlin = Предупреждать, если сохранение меняет формат профиля
settings-warn-gremlin-tip = Профили с модификаторами или слоями сохраняются в формате, который Joystick Gremlin не открывает.
gremlin-format-title = Joystick Gremlin не откроет этот профиль
gremlin-format-text = В { $file } есть возможности только Joystick Gobloon (модификаторы и слои). После сохранения Joystick Gremlin больше не сможет его открыть.
gremlin-format-dont-ask = Больше не спрашивать
status-whitelisted = Joystick Gobloon добавлен в белый список HidHide.

# Applications
apps-add = Добавить программу
apps-empty = Программ пока нет. Добавьте игру или программу, чтобы настроить её.
apps-missing = Файл не найден
apps-missing-text = Программы больше нет на этом месте. Убрать её из списка?
apps-sees = Видит скрытые устройства
apps-blind = Не видит скрытые устройства
apps-inverse = Инвертировать список программ
apps-inverse-tip = Программы из списка не видят скрытые устройства, все остальные видят
apps-remove = Убрать из списка

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
edit-rename = Нажмите, чтобы переименовать
edit-description-hint = Нажмите, чтобы написать заметку
edit-click-to-edit = Нажмите, чтобы изменить

# Files and modes
action-new = Новый профиль
wizard-open = Новый профиль с мастером: каждый контроллер на свой виртуальный джойстик
wizard-title = Мастер нового профиля
wizard-name = Имя профиля
wizard-name-hint = например, Мой полёт
wizard-create = Создать
wizard-devices = Выберите виртуальный джойстик для каждого контроллера. Каждая ось, кнопка и шляпка получит Map to vJoy.
wizard-none = Не назначен
wizard-taken = На этот виртуальный джойстик уже назначен другой контроллер
wizard-no-vjoy = Виртуальных джойстиков пока нет: сначала добавьте их на странице «Виртуальные джойстики»
wizard-fewer = У { $vjoy } меньше входов, чем у { $device }: не назначены { $missing }
wizard-axes = { $n ->
    [one] { $n } ось
    [few] { $n } оси
   *[other] { $n } осей
}
wizard-buttons = { $n ->
    [one] { $n } кнопка
    [few] { $n } кнопки
   *[other] { $n } кнопок
}
wizard-hats = { $n ->
    [one] { $n } шляпка
    [few] { $n } шляпки
   *[other] { $n } шляпок
}
status-wizard = Профиль создан: { $name }
action-save-as = Сохранить как…
status-new = Новый профиль
unsaved-title = Несохранённые изменения
unsaved-text = Сохранить изменения в { $file }?
unsaved-save = Сохранить
unsaved-discard = Не сохранять
cancel = Отмена
modes-edit = Изменить режимы
modifiers-edit = Модификаторы
modifiers-none = Модификаторов пока нет. Модификатор — это кнопка, пока она зажата, остальные вводы переключаются на другой слой действий.
modifiers-add = Добавить модификатор
modifiers-press = Нажмите кнопку на контроллере…
modifiers-assign = Нажмите здесь, затем кнопку, которую должен использовать модификатор
modifiers-remove = Удалить модификатор и действия его слоёв
modifier-input = Модификатор { $name }: пока он зажат, остальные вводы используют его слой; сам он действий не выполняет
layer-base = Основной
layer-set = На этом слое есть действия
layer-empty = На этом слое нет действий
layer-additive = Добавлять к основному
layer-additive-tip = Вкл.: эти действия работают вместе с основным слоем. Выкл.: пока зажаты модификаторы, они заменяют его.
layer-keep-value = Сохранять последнее значение
layer-keep-value-tip = Вкл.: когда слой перестаёт действовать, его оси сохраняют последнее значение. Выкл.: они возвращаются в покой.
layers-tip = Слои: { $layers }
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
edit-curve-help = Перетащите точку или маркер · Двойной щелчок добавляет точку · Правый щелчок по точке удаляет её
edit-curve-help-invert = Правый щелчок по пустому месту инвертирует

# vJoy mapping
vjoy-device-n = vJoy #{ $n }
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
vjoy-quickmap = Выбрать выход
output-profile = Профиль: { $name }
output-free = { $output }: свободен
output-used = { $output }: { $n ->
    [one] назначен { $n } входу
    [few] назначен { $n } входам
   *[other] назначен { $n } входам
}
output-conflict = Его пишут входы, работающие одновременно
output-more = …и ещё { $n }
output-legend-used = Занят
output-legend-conflict = Конфликт
output-legend-tip = Показывать эти элементы (профиль не меняется)
output-same-input = Его уже пишет другое действие этого входа
output-same-input-clash = Другое действие этого входа пишет тот же выход: выберите другой
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
status-latency = Задержка от считывания контроллера до виртуального выхода за последнюю секунду: в среднем { $average } мс, максимум { $max } мс. Собственная задержка контроллера не учитывается.

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
edit-merge-full = Оно уже объединяет две другие оси; смените одну из них в его редакторе.
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
edit-tts-front = Следующим (в речи системы — как в конец очереди)
edit-tts-interrupt = Прервать текущую речь
edit-preview = Прослушать

kind-light-led = Подсветка
edit-led-device = Джойстик с подсветкой
edit-led-no-device = Джойстик не подключён или его подсветкой нельзя управлять
edit-led-id = Светодиод
led-vkb-grip = RGB на верху рукояти
led-vkb-hat = Маленький красный у хатки
edit-led-color = Цвет

edit-led-color2 = Второй цвет
edit-led-blend = Цвета
led-blend-color1 = Один цвет
led-blend-color2 = Только второй цвет
led-blend-alternate = Чередовать цвета
led-blend-alternate-reverse = Чередовать, сначала второй
led-blend-mix = Смешать цвета
led-vkb-base-blue = База, синий (справа)
led-vkb-base-red = База, красный (слева)
edit-led-mode = Как светится
led-mode-on = Горит
led-mode-slow-blink = Мигает медленно
led-mode-fast-blink = Мигает быстро
led-mode-fastest-blink = Мигает очень быстро
led-mode-off = Выключен
edit-led-test = Зажечь сейчас
edit-led-on-activate = При активации профиля или режима
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
settings-tray = Показывать значок в трее
settings-close-to-tray = Закрывать в трей
settings-minimize-to-tray = Сворачивать в трей
settings-tray-double-click = Двойной клик активирует профиль
settings-tray-title = Значок в трее
settings-autostart = Запускать при входе в систему (свёрнутым)
settings-activate-on-start = Активировать профиль при запуске
tray-show = Показать Joystick Gobloon
tray-hiding = Скрытие устройств
tray-activate = Активировать «{ $name }»
tray-activate-none = Активировать профиль
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
about-hiding = Скрытие устройств: { $backend }
about-virtual = Виртуальные джойстики: { $backend }
about-not-installed = не установлено
about-gremlin = Профили совместимы с Joystick Gremlin.
about-license = Свободная программа с открытым кодом под лицензией MIT.

# Updates
settings-check-updates = Проверять новую версию при запуске и устанавливать её
update-check = Проверить обновления
update-checking = Поиск новой версии…
update-latest = У вас последняя версия
update-available = Доступна версия { $version }
update-install = Обновить
update-downloading = Загрузка версии { $version }…
update-error = Не удалось проверить обновления
update-debug = Отладочные сборки не обновляются
update-install-error = Не удалось установить обновление: { $error }

# Input viewer
viewer-history = Оси во времени
viewer-current = Оси сейчас
viewer-buttons = Кнопки и хатки
viewer-axis = Ось { $n }

# vJoy configuration
vjoy-add = Добавить устройство vJoy
vjoy-add-max = vJoy поддерживает не больше 16 устройств.
vjoy-new-device = Новый vJoy #{ $n }: ещё не создан
vjoy-enable = Включить драйвер vJoy
vjoy-disable = Выключить драйвер vJoy
vjoy-hat-continuous = Плавные
vjoy-hat-discrete = 4 направления
vjoy-ffb = Обратная связь (FFB)
vjoy-ffb-enable = Включена
vjoy-name = Имя
vjoy-name-suggest = Предложить имя: по контроллеру, который профиль назначает на это устройство, или по раскладке
vjoy-name-invalid = До 79 символов, без пробелов по краям, не как у других устройств.
vjoy-name-renamed = Переименованное устройство игры видят как новое: назначения в них, возможно, придётся задать заново.
vjoy-product = Product ID
vjoy-product-tip = По нему игры различают устройства. Пусто — выбирается по номеру устройства; введите ID прежней установки, чтобы сохранить назначения в игре.
vjoy-product-invalid = Шестнадцатеричное число от BE01 до BEFF (кроме BEAD), не занятое другим устройством.
vjoy-ffb-const = Постоянная сила
vjoy-ffb-ramp = Нарастание
vjoy-ffb-sq = Меандр
vjoy-ffb-sine = Синус
vjoy-ffb-tr = Треугольник
vjoy-ffb-stup = Пила вверх
vjoy-ffb-stdn = Пила вниз
vjoy-ffb-spr = Пружина
vjoy-ffb-dm = Демпфер
vjoy-ffb-inr = Инерция
vjoy-ffb-fric = Трение
vjoy-config-create = Создать
vjoy-config-apply = Применить
vjoy-config-revert = Отменить изменения
vjoy-config-remove = Удалить устройство
vjoy-config-admin = Для изменения vJoy Windows запросит права администратора
vjoy-config-locked = Остановите профиль, чтобы менять устройства vJoy.
vjoy-config-reading = Читаю конфигурацию vJoy...
vjoy-config-label = настройка vJoy #{ $n }
vjoy-config-done = Конфигурация vJoy изменена.
vjoy-config-error = Не удалось изменить конфигурацию vJoy: { $error }
vjoy-config-reboot = Конфигурация vJoy изменена. Перезагрузите Windows, чтобы она вступила в силу.
vjoy-remove-title = Удалить vJoy #{ $n }?
vjoy-remove-text = Игры и профили, использующие это устройство, потеряют его, пока оно не будет создано снова.

# Response curve
curve-type = Тип кривой
curve-linear = Ломаная
curve-cubic = Кубический сплайн
curve-monotone = Плавная, без выбросов
curve-bezier = Кубический сплайн Безье
curve-symmetric = Симметричная
curve-symmetric-tip = При правке отражать кривую относительно центра
curve-point = Точка
curve-handle = Маркер

# Keyboard device
keyboard-device = Клавиатура
keyboard-tip = Клавиши как входы: назначайте действия на клавиши, как на кнопки джойстика
keyboard-add = Назначить клавишу
keyboard-empty = Клавиши ещё не назначены: запишите клавишу кнопкой выше.
edit-binding-description = Описание этого входа
edit-unbound = Действий пока нет: добавьте действие.
viewer-detach = Открыть в отдельном окне
viewer-attach = Вернуть в главное окно
viewer-in-window = Просмотр входов открыт в отдельном окне.

# Calibration
calibration-device = Устройство для калибровки
calibration-raw = Сырое
calibration-with-center = С центром
calibration-reset = Сбросить
calibration-save = Сохранить калибровку
calibration-center = Калибровать центр
calibration-center-tip = Отпустите ручку, слегка покачайте её у центра, затем нажмите ещё раз
calibration-extrema = Калибровать края
calibration-extrema-tip = Доведите ось до обоих краёв, затем нажмите ещё раз
calibration-jitter = Фильтр дрожания
calibration-jitter-tip = Игнорирует изменения меньше этой доли хода — для изношенных осей, которые дрожат. Без задержки; края и центр достигаются всегда. Черта на полосе показывает, что отправляется. Сохраните, чтобы применить
calibration-debounce = Антидребезг
calibration-debounce-tip = Кнопки и шляпки этого устройства: после нажатия или отпускания следующие изменения в течение этого времени игнорируются — для дребезжащих контактов. Первое изменение проходит сразу
calibration-in-window = Калибровка открыта в отдельном окне.

# Device info
device-info = Сведения об устройстве

device-leds = Подсветкой можно управлять (действие «Подсветка»)
device-info-name = Название
device-info-axes = Оси
device-info-buttons = Кнопки
device-info-hats = Хатки
device-info-vid = VID
device-info-pid = PID
device-info-joystick-id = ID джойстика
device-info-guid = GUID устройства
device-info-copy = Копировать в буфер обмена

# Virtual gamepads
kind-map-to-gamepad = Вывод на геймпад
gamepad-n = Геймпад { $n }
gamepad-tip = Виртуальный геймпад в стиле Xbox: игры, Steam и Proton видят контроллер Xbox 360.
gamepad-add = Добавить виртуальный геймпад
gamepad-add-max = Не больше { $n } геймпадов.
gamepad-add-n = Добавить геймпад { $n }
gamepad-delete-n = Удалить геймпад { $n }
gamepad-missing = Геймпада { $n } ещё нет: добавьте его на странице «Виртуальные джойстики».
pad-a = A
pad-b = B
pad-x = X
pad-y = Y
pad-lb = LB
pad-rb = RB
pad-back = Back
pad-start = Start
pad-guide = Guide
pad-ls = Нажатие левого стика
pad-rs = Нажатие правого стика
pad-dpad-up = Крестовина вверх
pad-dpad-down = Крестовина вниз
pad-dpad-left = Крестовина влево
pad-dpad-right = Крестовина вправо
pad-left-x = Левый стик X
pad-left-y = Левый стик Y
pad-right-x = Правый стик X
pad-right-y = Правый стик Y
pad-lt = LT (левый курок)
pad-rt = RT (правый курок)
pad-ls-short = LS
pad-rs-short = RS
pad-dpad = Крестовина

system-banner-install = Установите SystemGobloon: скрытые стики не видит ни одна программа, а виртуальные устройства остаются на месте, пока идёт игра, как с HidHide и vJoy в Windows.
system-banner-stopped = SystemGobloon установлена, но не работает.
system-banner-update = Установлена SystemGobloon { $version }; эта программа несёт другую версию.
system-install = Установить
system-update = Обновить
system-start = Запустить
system-remove = Удалить
system-not-now = Не сейчас
system-admin = Спросит пароль администратора
system-about = Скрывает стики от всех программ и держит виртуальные джойстики, геймпады, клавиатуру и мышь, пока идёт игра. После установки пароль не нужен.
system-status-missing = Не установлена.
system-status-stopped = Установлена, не работает.
system-status-outdated = Работает версия { $version }: обновите её до версии программы.
system-status-current = Работает версия { $version }.
system-remove-title = Удалить SystemGobloon?
system-remove-text = Тогда скрытые стики скрыты, только пока работает Joystick Gobloon, а виртуальные устройства есть, только пока активен профиль. Скрытые устройства и раскладки виртуальных устройств сохранятся.
system-installed = SystemGobloon установлена.
system-removed = SystemGobloon удалена.
system-error = SystemGobloon: { $error }
