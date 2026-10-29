language-name = Español

# Navigation and toolbar tooltips
nav-devices = Dispositivos y asignaciones
nav-hidhide = Ocultar dispositivos
nav-apps = Aplicaciones
nav-vjoy = Joysticks virtuales
nav-viewer = Visor de entradas
nav-calibration = Calibración
nav-settings = Configuración
action-open = Importar perfil de Joystick Gremlin
action-save = Guardar
label-mode = Modo

# Settings
settings-appearance = Apariencia
settings-general = General
settings-about = Acerca de

settings-lighting = Iluminación
settings-leds-hint = Al desactivar un perfil que cambió los LED de un joystick, se aplican estos colores. El joystick conserva los colores cambiados hasta que se vuelve a conectar, así que no se puede recuperar su propia configuración de LED.
settings-leds-none = No hay ningún joystick con LED controlables conectado.
settings-leds-add = Añadir LED
settings-language = Idioma
settings-reopen-last = Abrir el último perfil al iniciar
theme-light = Claro
theme-dark = Oscuro
theme-system = Sistema
theme-system-tip = Seguir el modo claro u oscuro del sistema
about-text = Ocultación de dispositivos, joysticks virtuales y asignación de entradas en una sola aplicación.

# Empty states
no-profile = Ningún perfil cargado
device-linked = Asignaciones del dispositivo del perfil con el mismo nombre. Use «Asignar controles» para guardar el vínculo.
device-disconnected = No conectado
no-devices-attached = No se encontraron mandos de juego

# Status bar
status-ready = Listo
status-loaded = { $file } cargado
status-saved = { $file } guardado
import-error = No se pudo abrir el perfil: { $error }
save-error = No se pudo guardar el perfil: { $error }
file-filter-gremlin = Perfil de Joystick Gremlin

# Import report
alert-error = Algo salió mal
alert-import = Algunas acciones de { $file } no funcionarán
alert-open-log = Abrir el registro
ok = Aceptar
hidhide-whitelist-error = No se pudo añadir Joystick Gobloon a la lista blanca de HidHide: { $error }
warn-not-executable = «{ $kind }» × { $count }: se conserva en el perfil, pero aún no se ejecuta
warn-scripts = El perfil contiene complementos de Python: se conservan, pero no se ejecutan

# Map controls (device substitution)
subst-title = Asignar controles
subst-auto = Automático
subst-auto-tip = Asignar los dispositivos restantes a los conectados con nombres parecidos; los dudosos quedan para que usted elija
subst-banner = Los ID de dispositivo del perfil no coinciden con este PC: { $count }
virtual-banner = Este perfil usa dispositivos virtuales que faltan en este sistema o están configurados más pequeños: { $devices }
virtual-setup = Configurar
virtual-setup-tip = Crear los dispositivos virtuales que faltan y añadir a los existentes lo que les falta. No se quita nada.
virtual-setup-admin = Crear los dispositivos virtuales que faltan y añadir a los existentes lo que les falta. No se quita nada. Cada dispositivo pide permisos de administrador.
virtual-setup-active = Detenga el perfil para cambiar los dispositivos virtuales.
virtual-setup-done = Los dispositivos virtuales están configurados para el perfil.
access-banner = Joystick Gobloon necesita permiso para leer los mandos y crear dispositivos virtuales.
access-grant = Conceder acceso
access-granted = Acceso a dispositivos concedido.
access-error = No se pudo conceder el acceso a dispositivos: { $error }
keyboard-access-banner = Las asignaciones de teclado necesitan permiso para leer los teclados. Entonces cualquier programa que ejecutes podrá leer lo que escribes.
keyboard-access-allow = Permitir la entrada de teclado
keyboard-access-remove = Quitar el acceso al teclado
keyboard-access-allowed = Entrada de teclado permitida.
keyboard-access-removed = Acceso al teclado quitado. Termina cuando se vuelve a conectar el teclado o se reinicia el equipo.
keyboard-access-error = No se pudo cambiar el acceso al teclado: { $error }
all-buttons-banner = { $device } tiene { $total } botones, pero Linux solo transmite los primeros { $shown }. Los programas que leen el dispositivo directamente, como Steam, también podrán leer este stick, aunque esté oculto.
all-buttons-allow = Leer todos los botones
all-buttons-allowed = Ahora se leen todos los botones del stick.
all-buttons-error = No se pudo permitir leer todos los botones: { $error }
subst-keep = Dejar como está
subst-apply = Aplicar
status-substituted = Controles asignados. Guarde el perfil para conservar el cambio.

# vJoy
vjoy-missing = vJoy no está instalado o no se pudo cargar.
vjoy-no-access = No hay permiso para crear dispositivos vJoy.
vjoy-enabled = Controlador activado
vjoy-disabled = Controlador desactivado
vjoy-no-devices = Aún no hay dispositivos vJoy configurados.
vjoy-axes = Ejes
vjoy-buttons = Botones
vjoy-hats = Crucetas
vjoy-test-hint = Probar la salida: bloquee el dispositivo para usar los controles de abajo.
vjoy-test-pending = Aplique los cambios para probarlos.
vjoy-lock = Tomar el control exclusivo de este dispositivo para probar su salida
vjoy-unlock = Liberar el dispositivo
vjoy-free = Libre
vjoy-own = Controlado por Joystick Gobloon
vjoy-busy = Usado por otro programa u otra copia de Joystick Gobloon
vjoy-unknown = Estado desconocido
vjoy-error = No se pudo tomar el dispositivo: { $error }

# Device hiding
hidhide-missing = HidHide no está instalado.
hidhide-on = La ocultación de dispositivos está activada
hidhide-off = La ocultación de dispositivos está desactivada
hidhide-hidden = Oculto: solo lo ven las aplicaciones de la lista blanca
hidhide-visible = Visible para todas las aplicaciones
hidhide-filter-disconnected = Ocultar dispositivos desconectados
hidhide-gaming-only = Solo mandos de juego
hidhide-enable = Activar la ocultación de dispositivos
hidhide-confirm-on-title = ¿Activar la ocultación de dispositivos?
hidhide-confirm-on-text = Solo los programas de la lista blanca verán los dispositivos ocultos.
hidhide-confirm-off-title = ¿Desactivar la ocultación de dispositivos?
hidhide-confirm-off-text = Todos los programas vuelven a ver todos los dispositivos, también los ocultos.
hidhide-confirm-hint = Haz doble clic en el ojo para cambiarlo sin esta pregunta.
action-turn-on = Activar
action-turn-off = Desactivar
hidhide-hint = Haga clic en el ojo para ocultar un dispositivo a otros programas (ojo cerrado). Después vuelva a conectarlo para que el cambio surta efecto.
hidhide-exposed = Sigue siendo visible para programas que leen el dispositivo en bruto (hidraw), p. ej. Steam o Wine/Proton: en este sistema la ocultación no puede bloquearlos. Desactive hidraw para este dispositivo en esos programas o los juegos podrían verlo dos veces.
hidhide-no-devices = No hay dispositivos que mostrar.
action-refresh = Actualizar
settings-see-hidden = Ver los dispositivos ocultos
settings-see-hidden-tip = Al iniciar, el programa se añade a los programas que siguen viendo los dispositivos ocultos, para que los perfiles sigan funcionando.
settings-group-startup = Inicio
settings-group-devices = Dispositivos
settings-group-editor = Editor
settings-focus-input = Seguir la entrada en uso
settings-focus-input-tip = Al usar un eje, botón o hat, la página «Dispositivos y asignaciones» selecciona la pestaña de su dispositivo y se desplaza hasta la entrada.
settings-focus-skip-modifiers = No saltar a los modificadores
settings-focus-skip-modifiers-tip = Pulsar un botón o tecla modificador no desplaza la lista hasta él, así la entrada que configuras sigue a la vista.
settings-input-events = Leer los mandos al cambiar
settings-input-events-tip = Menos latencia y uso de CPU: los mandos se leen cuando informan de un cambio y las pulsaciones cortas nunca se pierden. Desactivado: cada mando se lee en cada intervalo de sondeo.
settings-poll-interval = Intervalo de sondeo
settings-poll-interval-tip = Cada cuánto se sondea un mando: todos los mandos si «Leer los mandos al cambiar» está desactivado; si no, solo los que no pueden informar de cambios. Más corto significa menos latencia y algo más de CPU.
settings-poll-interval-unused = Ahora no se usa: todos los mandos conectados informan de sus cambios, así que ninguno se sondea. Se aplica si «Leer los mandos al cambiar» está desactivado o un mando no puede informar de cambios.
settings-monotonic-curves = Las curvas solo suben o bajan
settings-monotonic-curves-tip = Editor de curvas de respuesta: un punto o tirador se detiene donde la curva daría la vuelta, así cruza el cero una sola vez. Las curvas que ya dan la vuelta se editan libremente.
settings-curve-right-click-invert = El clic derecho invierte las curvas
settings-curve-right-click-invert-tip = Editor de curvas de respuesta: un clic derecho en un espacio vacío invierte la curva. El botón Invertir funciona siempre.
settings-warn-gremlin = Avisar si al guardar cambia el formato del perfil
settings-warn-gremlin-tip = Los perfiles con modificadores o capas se guardan en un formato que Joystick Gremlin no puede abrir.
gremlin-format-title = Joystick Gremlin no lo abrirá
gremlin-format-text = { $file } usa funciones que solo tiene Joystick Gobloon (modificadores y capas). Una vez guardado, Joystick Gremlin ya no podrá abrirlo.
gremlin-format-dont-ask = No volver a preguntar
status-whitelisted = Joystick Gobloon se añadió a la lista blanca de HidHide.

# Applications
apps-add = Añadir aplicación
apps-empty = Aún no hay aplicaciones. Añada un juego o programa para configurarlo.
apps-missing = Archivo no encontrado
apps-missing-text = El programa ya no está. ¿Quitarlo de la lista?
apps-sees = Ve los dispositivos ocultos
apps-blind = No ve los dispositivos ocultos
apps-inverse = Invertir la lista de aplicaciones
apps-inverse-tip = Los programas de la lista no ven los dispositivos ocultos; todos los demás sí
apps-remove = Quitar de la lista

# Engine
action-activate = Activar el perfil
action-deactivate = Detener el perfil
title-active = Activo
title-inactive = Inactivo
status-active = Perfil activo
status-stopped = Perfil detenido
activate-error = No se pudo activar el perfil: { $error }

# Editing
edit-add = Añadir acción
edit-remove = Quitar
edit-up = Subir
edit-down = Bajar
edit-not-editable = Esta acción se conserva en el perfil, pero aún no se puede editar.
edit-on-press = Al pulsar
edit-on-release = Al soltar
edit-on-both = Al pulsar y al soltar
edit-inverted = Invertido
edit-threshold = Umbral de pulsación larga
edit-short = Pulsación corta
edit-long = Pulsación larga
edit-deadzone = Zona muerta
edit-invert = Invertir
edit-inherited = Heredado del modo «{ $mode }»
edit-unbind = Quitar todas las acciones de esta entrada
edit-override = Sobrescribir en este modo
input-axis-kind = Eje
input-button-kind = Botón
input-hat-kind = Cruceta
kind-map-to-vjoy = Asignar a vJoy
kind-response-curve = Curva de respuesta
kind-change-mode = Cambiar modo
kind-tempo = Pulsación corta / larga
kind-description = Descripción
edit-rename = Haz clic para cambiar el nombre
edit-description-hint = Haz clic para escribir una nota
edit-click-to-edit = Haz clic para editar

# Files and modes
action-new = Nuevo perfil
wizard-open = Nuevo perfil con el asistente: cada mando a un joystick virtual
wizard-title = Asistente de nuevo perfil
wizard-name = Nombre del perfil
wizard-name-hint = p. ej. Mi configuración de vuelo
wizard-create = Crear
wizard-devices = Elige el joystick virtual de cada mando. Cada eje, botón y hat recibe un Map to vJoy.
wizard-none = Sin asignar
wizard-taken = Otro mando ya usa este joystick virtual
wizard-no-vjoy = Aún no hay joysticks virtuales: añádelos primero en la página Joysticks virtuales
wizard-fewer = { $vjoy } tiene menos entradas que { $device }: { $missing } quedan sin asignar
wizard-axes = { $n ->
    [one] 1 eje
   *[other] { $n } ejes
}
wizard-buttons = { $n ->
    [one] 1 botón
   *[other] { $n } botones
}
wizard-hats = { $n ->
    [one] 1 hat
   *[other] { $n } hats
}
status-wizard = Perfil creado: { $name }
action-save-as = Guardar como…
status-new = Nuevo perfil
unsaved-title = Cambios sin guardar
unsaved-text = ¿Guardar los cambios en { $file }?
unsaved-save = Guardar
unsaved-discard = No guardar
cancel = Cancelar
modes-edit = Editar modos
modifiers-edit = Modificadores
modifiers-none = Aún no hay modificadores. Un modificador es un botón que, mientras se mantiene pulsado, cambia las demás entradas a otra capa de acciones.
modifiers-add = Añadir un modificador
modifiers-press = Pulsa un botón de un mando…
modifiers-assign = Haz clic y luego pulsa el botón que debe usar este modificador
modifiers-remove = Quitar el modificador y las acciones de sus capas
modifier-input = Modificador { $name }: mientras se mantiene pulsado, las demás entradas usan su capa; él mismo no ejecuta acciones
layer-base = Base
layer-set = Esta capa tiene acciones
layer-empty = No hay acciones en esta capa
layer-additive = Añadir a la base
layer-additive-tip = Activado: estas acciones se ejecutan junto con la capa base. Desactivado: la sustituyen mientras se mantienen los modificadores.
layer-keep-value = Mantener el último valor
layer-keep-value-tip = Activado: cuando esta capa deja de actuar, sus ejes mantienen el último valor. Desactivado: vuelven al reposo.
layers-tip = Capas: { $layers }
modes-title = Modos
modes-name = Nombre
modes-parent = Hereda de
modes-remove = Quitar el modo y sus asignaciones
modes-new = Nuevo modo
modes-add = Añadir modo

# Undo
action-undo = Deshacer (Ctrl+Z)
action-redo = Rehacer (Ctrl+Y)
status-undone = Deshecho
status-redone = Rehecho
edit-curve-help = Arrastre un punto o tirador para moverlo · Doble clic para añadir un punto · Clic derecho en un punto para quitarlo
edit-curve-help-invert = Clic derecho en un espacio vacío para invertir

# vJoy mapping
vjoy-device-n = vJoy #{ $n }
vjoy-axis-1 = Eje X
vjoy-axis-2 = Eje Y
vjoy-axis-3 = Eje Z
vjoy-axis-4 = Rotación X
vjoy-axis-5 = Rotación Y
vjoy-axis-6 = Rotación Z
vjoy-axis-7 = Deslizador
vjoy-axis-8 = Rueda
vjoy-button-n = Botón { $n }
vjoy-hat-n = Cruceta { $n }
vjoy-quickmap = Elegir una salida
output-profile = Perfil: { $name }
output-free = { $output }: libre
output-used = { $output }: { $n ->
    [one] usada por 1 entrada
   *[other] usada por { $n } entradas
}
output-conflict = La escriben entradas activas a la vez
output-more = …y { $n } más
output-legend-used = En uso
output-legend-conflict = Conflicto
output-legend-tip = Mostrar estos controles (el perfil no cambia)
output-same-input = Otra acción de esta entrada ya la escribe
output-same-input-clash = Otra acción de esta entrada escribe la misma salida: elige otra
vjoy-absolute = Absoluto
vjoy-relative = Relativo
vjoy-speed = Velocidad

# Mouse
kind-map-to-mouse = Asignar al ratón
mouse-mode-button = Botón del ratón
mouse-mode-motion = Movimiento del cursor
mouse-left = Botón izquierdo
mouse-right = Botón derecho
mouse-middle = Botón central
mouse-forward = Botón Adelante
mouse-back = Botón Atrás
mouse-wheel-up = Rueda arriba
mouse-wheel-down = Rueda abajo
mouse-direction = Dirección
mouse-min-speed = Velocidad mínima
mouse-max-speed = Velocidad máxima
mouse-time-to-max = Tiempo hasta la velocidad máxima

# Toggle, split, pause, run
kind-smart-toggle = Interruptor inteligente
kind-split-axis = Dividir eje
kind-pause-resume = Pausar / reanudar
kind-run-command = Ejecutar comando
edit-delay = Tiempo de pulsación para modo momentáneo
edit-split = Dividir en
edit-lower = Parte inferior
edit-upper = Parte superior
pause-op-pause = Pausar
pause-op-resume = Reanudar
pause-op-toggle = Alternar
run-executable = Programa
run-arguments = Argumentos
run-browse = Elegir programa
status-paused = Perfil en pausa
status-latency = Latencia desde la lectura de un mando hasta la salida virtual, último segundo: media { $average } ms, máximo { $max } ms. No incluye el retardo propio del mando.

# Double tap, chain
kind-double-tap = Doble toque
kind-chain = Cadena
edit-tap-threshold = Tiempo hasta el segundo toque
edit-tap-exclusive = Exclusivo: un toque simple espera para descartar un doble toque
edit-tap-combined = Combinado: un doble toque también ejecuta las acciones del toque simple
edit-single = Toque simple
edit-double = Doble toque
edit-chain-timeout = Volver al primer paso tras este tiempo de inactividad (0 = nunca)
edit-chain-step = Paso { $n }

# Hat as buttons
kind-hat-buttons = Cruceta como botones
edit-hat-four = 4 direcciones
edit-hat-eight = 8 direcciones (con diagonales)
hat-north = Norte
hat-north-east = Noreste
hat-east = Este
hat-south-east = Sureste
hat-south = Sur
hat-south-west = Suroeste
hat-west = Oeste
hat-north-west = Noroeste
hat-center = Centro

# Merge axis
kind-merge-axis = Combinar ejes
edit-reuse = Añadir aquí también esta acción existente (compartida, como Gremlin la vincula a ambos ejes)
edit-merge-full = Ya combina otros dos ejes; cambie uno de ellos en su editor.
edit-merge-name = Nombre
edit-merge-no-device = (sin dispositivo)
edit-merged = Eje combinado
merge-average = Promedio
merge-minimum = Mínimo
merge-maximum = Máximo
merge-sum = Suma
merge-bidirectional = Bidireccional (eje 2 menos eje 1)
merge-maximum-deflection = Desviación máxima
merge-closest-to-center = Más cercano al centro

# Axis delta
kind-axis-delta = Variación de eje
edit-delta-threshold = Cambio del eje que dispara un pulso
edit-delta-positive = El eje aumentó
edit-delta-negative = El eje disminuyó

# Dual axis deadzone
kind-dual-axis-deadzone = Zona muerta de dos ejes
edit-dual-inner = Zona muerta interior (círculo alrededor del centro)
edit-dual-outer = Límite exterior (desviación completa a partir de aquí)
edit-dual-close = La zona muerta interior y la exterior están demasiado cerca.
edit-dual-first = Salida del primer eje
edit-dual-second = Salida del segundo eje

# Existing actions (Gremlin's reference action)
edit-link-existing = Vincular una acción existente
edit-copy-existing = Copiar una acción existente

# Condition
kind-condition = Condición
edit-cond-all = Deben cumplirse todas las condiciones
edit-cond-any = Basta con una condición
edit-cond-add = Condición
edit-cond-true = Si se cumple
edit-cond-false = Si no
cond-current = Esta entrada
cond-joystick = Entrada del joystick
cond-vjoy = Salida de vJoy
cond-keyboard = Teclas pulsadas
cond-range = Eje entre
cond-pressed = Pulsado
cond-released = Soltado

# Sound, speech, load profile
kind-play-sound = Reproducir sonido
kind-text-to-speech = Texto a voz
kind-load-profile = Cargar perfil
file-filter-sound = Archivo de sonido
edit-volume = Volumen
edit-file-missing = Archivo no encontrado
edit-tts-hint = Texto que se dirá; $current_mode se sustituye por el modo activo
edit-tts-rate = Velocidad
edit-tts-pitch = Tono
edit-tts-back = Poner en cola después de todo lo demás
edit-tts-front = Poner a continuación (voz del sistema: igual que después de todo lo demás)
edit-tts-interrupt = Interrumpir lo que se está diciendo
edit-preview = Vista previa

kind-light-led = Iluminación LED
edit-led-device = Joystick con LED
edit-led-no-device = Este joystick no está conectado o sus LED no se pueden controlar
edit-led-id = LED
led-vkb-grip = RGB en la parte superior de la empuñadura
led-vkb-hat = Pequeño rojo junto a la cruceta
edit-led-color = Color

edit-led-color2 = Segundo color
edit-led-blend = Colores
led-blend-color1 = Un color
led-blend-color2 = Solo el segundo color
led-blend-alternate = Alternar colores
led-blend-alternate-reverse = Alternar, primero el segundo
led-blend-mix = Mezclar colores
led-vkb-base-blue = Base, azul (derecha)
led-vkb-base-red = Base, rojo (izquierda)
edit-led-mode = Cómo se ilumina
led-mode-on = Encendido
led-mode-slow-blink = Parpadeo lento
led-mode-fast-blink = Parpadeo rápido
led-mode-fastest-blink = Parpadeo muy rápido
led-mode-off = Apagado
edit-led-test = Encender ahora
edit-led-on-activate = Al activar el perfil o el modo
edit-tts-voice = Voz
edit-tts-default-voice = Voz predeterminada
edit-tts-voice-missing = no instalada
kind-map-to-keyboard = Asignar al teclado

# Key recording
keys-record = Grabar teclas: púlselas juntas, la grabación termina al soltarlas todas
keys-press = Pulse las teclas…
keys-none = Sin teclas
keys-cancel = Cancelar la grabación
edit-chain-add = Añadir paso
edit-chain-remove = Quitar este paso y sus acciones
kind-macro = Macro

# Macro editor
macro-repeat = Repetir
macro-repeat-single = Una vez
macro-repeat-count = Número de veces
macro-repeat-hold = Mientras se mantenga
macro-repeat-toggle = Hasta volver a pulsar
macro-repeat-count-tip = Cuántas veces
macro-repeat-delay = Pausa entre repeticiones
macro-add-step = Paso
macro-step-key = Tecla
macro-step-pause = Pausa
macro-step-mouse-button = Botón del ratón
macro-step-mouse-motion = Movimiento del ratón
macro-step-vjoy-button = Botón de vJoy
macro-step-vjoy-axis = Eje de vJoy
macro-step-vjoy-hat = Cruceta de vJoy
macro-press = Pulsar
macro-release = Soltar

# Logical device
logical-device = Dispositivo lógico
logical-device-tip = Entradas virtuales de este perfil: asigne entradas a ellas y luego vincúlelas como un joystick
logical-add-axis = Añadir un eje
logical-add-button = Añadir un botón
logical-add-hat = Añadir una cruceta
logical-remove = Quitar esta entrada y sus asignaciones
logical-empty = Aún no hay entradas lógicas: añada una con los botones de arriba.
kind-map-to-logical-device = Asignar al dispositivo lógico
cond-logical = Entrada del dispositivo lógico

# Tray and startup
settings-tray = Mostrar el icono en la bandeja
settings-close-to-tray = Cerrar a la bandeja
settings-minimize-to-tray = Minimizar a la bandeja
settings-tray-double-click = Doble clic activa el perfil
settings-tray-title = Icono de la bandeja
settings-autostart = Iniciar al iniciar sesión (minimizado)
settings-activate-on-start = Activar el perfil al iniciar
tray-show = Mostrar Joystick Gobloon
tray-hiding = Ocultar dispositivos
tray-activate = Activar «{ $name }»
tray-activate-none = Activar el perfil
tray-quit = Salir
autostart-error = No se pudo cambiar el inicio al iniciar sesión: { $error }

# About
about-alpha = versión alfa
about-created = Creado por { $name }
about-support = Apoyar el desarrollo
about-support-tip = Contribuir mediante PayPal
about-source = Código fuente
about-issues = Informar de un problema
about-releases = Versiones
about-hiding = Ocultación de dispositivos: { $backend }
about-virtual = Joysticks virtuales: { $backend }
about-not-installed = no instalado
about-gremlin = Los perfiles son compatibles con Joystick Gremlin.
about-license = Libre y de código abierto bajo la licencia MIT.

# Updates
settings-check-updates = Buscar una versión nueva al iniciar e instalarla
update-check = Buscar actualizaciones
update-checking = Buscando una versión nueva…
update-latest = Tiene la versión más reciente
update-available = La versión { $version } está disponible
update-install = Actualizar ahora
update-downloading = Descargando la versión { $version }…
update-error = No se pudo buscar actualizaciones
update-debug = Las versiones de depuración no se actualizan
update-install-error = No se pudo instalar la actualización: { $error }

# Input viewer
viewer-history = Ejes a lo largo del tiempo
viewer-current = Ejes ahora
viewer-buttons = Botones y crucetas
viewer-axis = Eje { $n }

# vJoy configuration
vjoy-add = Añadir un dispositivo vJoy
vjoy-add-max = vJoy admite como máximo 16 dispositivos.
vjoy-new-device = Nuevo vJoy #{ $n }: aún no creado
vjoy-enable = Activar el controlador vJoy
vjoy-disable = Desactivar el controlador vJoy
vjoy-hat-continuous = Continua
vjoy-hat-discrete = 4 direcciones
vjoy-ffb = Force feedback
vjoy-ffb-enable = Activado
vjoy-ffb-const = Constante
vjoy-ffb-ramp = Rampa
vjoy-ffb-sq = Cuadrada
vjoy-ffb-sine = Senoidal
vjoy-ffb-tr = Triangular
vjoy-ffb-stup = Diente de sierra ascendente
vjoy-ffb-stdn = Diente de sierra descendente
vjoy-ffb-spr = Muelle
vjoy-ffb-dm = Amortiguador
vjoy-ffb-inr = Inercia
vjoy-ffb-fric = Fricción
vjoy-config-create = Crear
vjoy-config-apply = Aplicar
vjoy-config-revert = Revertir
vjoy-config-remove = Quitar dispositivo
vjoy-config-admin = Windows pide permisos de administrador para cambiar vJoy
vjoy-config-locked = Detenga el perfil para cambiar los dispositivos vJoy.
vjoy-config-reading = Leyendo la configuración de vJoy...
vjoy-config-label = configurar vJoy #{ $n }
vjoy-config-done = Configuración de vJoy cambiada.
vjoy-config-error = No se pudo cambiar la configuración de vJoy: { $error }
vjoy-config-reboot = Configuración de vJoy cambiada. Reinicie Windows para que surta efecto.
vjoy-remove-title = ¿Quitar vJoy #{ $n }?
vjoy-remove-text = Los juegos y perfiles que usan este dispositivo lo pierden hasta que se vuelva a crear.

# Response curve
curve-type = Tipo de curva
curve-linear = Lineal por tramos
curve-cubic = Spline cúbica
curve-monotone = Suave, sin sobreoscilación
curve-bezier = Spline de Bézier cúbica
curve-symmetric = Simétrica
curve-symmetric-tip = Reflejar la curva respecto al centro al editarla
curve-point = Punto
curve-handle = Tirador

# Keyboard device
keyboard-device = Teclado
keyboard-tip = Teclas como entradas: vincule acciones a teclas como a botones de joystick
keyboard-add = Vincular una tecla
keyboard-empty = Aún no hay teclas vinculadas: grabe una con el botón de arriba.
edit-binding-description = Descripción de esta entrada
edit-unbound = Aún no hay acciones: añada una.
viewer-detach = Abrir en una ventana aparte
viewer-attach = Volver a la ventana principal
viewer-in-window = El visor de entradas está abierto en su propia ventana.

# Calibration
calibration-device = Dispositivo a calibrar
calibration-raw = Sin procesar
calibration-with-center = Con centro
calibration-reset = Restablecer
calibration-save = Guardar esta calibración
calibration-center = Calibrar el centro
calibration-center-tip = Deje el joystick en reposo, muévalo ligeramente alrededor del centro y vuelva a hacer clic
calibration-extrema = Calibrar los extremos
calibration-extrema-tip = Mueva el eje a ambos extremos y vuelva a hacer clic
calibration-jitter = Filtro de temblor
calibration-jitter-tip = Ignora los cambios menores que esta parte del recorrido, para ejes gastados que tiemblan. Sin retardo; los extremos y el centro siempre se alcanzan. La línea en la barra muestra lo que se envía. Guarda para aplicarlo
calibration-debounce = Antirrebote
calibration-debounce-tip = Botones y hats de este dispositivo: tras pulsar o soltar, se ignoran los cambios durante este tiempo, para interruptores que rebotan. El primer cambio pasa al instante
calibration-in-window = La calibración está abierta en su propia ventana.

# Device info
device-info = Información del dispositivo

device-leds = LED controlables (acción Iluminación LED)
device-info-name = Nombre del dispositivo
device-info-axes = Ejes
device-info-buttons = Botones
device-info-hats = Crucetas
device-info-vid = VID
device-info-pid = PID
device-info-joystick-id = ID del joystick
device-info-guid = GUID del dispositivo
device-info-copy = Copiar al portapapeles

# Virtual gamepads
kind-map-to-gamepad = Asignar a gamepad
gamepad-n = Gamepad { $n }
gamepad-tip = Gamepad virtual estilo Xbox: los juegos, Steam y Proton ven un mando de Xbox 360.
gamepad-add = Añadir un gamepad virtual
gamepad-add-max = Como máximo { $n } gamepads.
gamepad-add-n = Añadir gamepad { $n }
gamepad-delete-n = Eliminar gamepad { $n }
gamepad-missing = El gamepad { $n } aún no existe: añádalo en la página Joysticks virtuales.
pad-a = A
pad-b = B
pad-x = X
pad-y = Y
pad-lb = LB
pad-rb = RB
pad-back = Back
pad-start = Start
pad-guide = Guide
pad-ls = Pulsar stick izquierdo
pad-rs = Pulsar stick derecho
pad-dpad-up = Cruceta arriba
pad-dpad-down = Cruceta abajo
pad-dpad-left = Cruceta izquierda
pad-dpad-right = Cruceta derecha
pad-left-x = Stick izquierdo X
pad-left-y = Stick izquierdo Y
pad-right-x = Stick derecho X
pad-right-y = Stick derecho Y
pad-lt = LT (gatillo izquierdo)
pad-rt = RT (gatillo derecho)
pad-ls-short = LS
pad-rs-short = RS
pad-dpad = Cruceta
