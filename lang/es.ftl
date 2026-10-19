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
theme-system-tip = Seguir el modo claro u oscuro de Windows
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
subst-keep = Dejar como está
subst-apply = Aplicar
status-substituted = Controles asignados. Guarde el perfil para conservar el cambio.

# vJoy
vjoy-missing = vJoy no está instalado o no se pudo cargar.
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
vjoy-busy = Usado por otro programa (p. ej., Joystick Gremlin)
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
hidhide-hint = Haga clic en el ojo para ocultar un dispositivo a otros programas (ojo cerrado). Después vuelva a conectarlo para que el cambio surta efecto.
hidhide-no-devices = No hay dispositivos que mostrar.
action-refresh = Actualizar
settings-hidhide-self = Añadir Joystick Gobloon a la lista blanca de HidHide al iniciar
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

# Files and modes
action-new = Nuevo perfil
action-save-as = Guardar como…
status-new = Nuevo perfil
unsaved-title = Cambios sin guardar
unsaved-text = ¿Guardar los cambios en { $file }?
unsaved-save = Guardar
unsaved-discard = No guardar
cancel = Cancelar
modes-edit = Editar modos
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
edit-curve-help = Arrastre un punto o tirador para moverlo · Clic en un espacio vacío para invertir · Doble clic para añadir un punto · Clic derecho en un punto para quitarlo

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
edit-tts-front = Poner a continuación (voz de Windows: igual que después de todo lo demás)
edit-tts-interrupt = Interrumpir lo que se está diciendo
edit-preview = Vista previa
kind-light-led = Iluminación LED
edit-led-device = Joystick con LED
edit-led-no-device = Este joystick no está conectado o sus LED no se pueden controlar
edit-led-id = LED
led-vkb-grip = RGB en la parte superior de la empuñadura
led-vkb-hat = Pequeño rojo junto a la cruceta
led-vkb-base = Base (azul y rojo)
edit-led-color = Color
edit-led-color2 = Segundo color
edit-led-blend = Colores
led-blend-color1 = Un color
led-blend-color2 = Solo el segundo color
led-blend-alternate = Alternar colores
led-blend-alternate-reverse = Alternar, primero el segundo
led-blend-mix = Mezclar colores
led-vkb-base-blue = Azul
led-vkb-base-red = Rojo
led-vkb-base-both = Azul y rojo
led-vkb-base-alternate = Azul y rojo por turnos
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
settings-tray-title = Icono de la bandeja
settings-autostart = Iniciar con Windows (minimizado)
settings-activate-on-start = Activar el perfil al iniciar
tray-show = Mostrar Joystick Gobloon
tray-hiding = Ocultar dispositivos
tray-activate = Activar «{ $name }»
tray-activate-none = Activar el perfil
tray-quit = Salir
autostart-error = No se pudo cambiar el inicio con Windows: { $error }

# About
about-alpha = versión alfa
about-created = Creado por { $name }
about-support = Apoyar el desarrollo
about-support-tip = Contribuir mediante PayPal
about-source = Código fuente
about-issues = Informar de un problema
about-releases = Versiones
about-credits = Usa los controladores HidHide y vJoy; los perfiles son compatibles con Joystick Gremlin.
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
