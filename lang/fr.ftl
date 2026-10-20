language-name = Français

# Navigation and toolbar tooltips
nav-devices = Périphériques et affectations
nav-hidhide = Masquage des périphériques
nav-apps = Applications
nav-vjoy = Joysticks virtuels
nav-viewer = Visualiseur d’entrées
nav-calibration = Calibrage
nav-settings = Paramètres
action-open = Importer un profil Joystick Gremlin
action-save = Enregistrer
label-mode = Mode

# Settings
settings-appearance = Apparence
settings-general = Général
settings-about = À propos

settings-lighting = Éclairage
settings-leds-hint = Quand un profil qui a modifié les LED d'un joystick est désactivé, ces couleurs sont appliquées. Le joystick garde les couleurs modifiées jusqu'à sa reconnexion : ses propres réglages de LED ne peuvent donc pas être rétablis.
settings-leds-none = Aucun joystick avec LED pilotables n'est connecté.
settings-leds-add = Ajouter une LED
settings-language = Langue
settings-reopen-last = Rouvrir le dernier profil au démarrage
theme-light = Clair
theme-dark = Sombre
theme-system = Système
theme-system-tip = Suivre le mode clair ou sombre du système
about-text = Masquage des périphériques, joysticks virtuels et affectation des entrées dans une seule application.

# Empty states
no-profile = Aucun profil chargé
device-linked = Affectations du périphérique du profil portant le même nom. Utilisez « Associer les commandes » pour enregistrer le lien.
device-disconnected = Non connecté
no-devices-attached = Aucune manette de jeu trouvée

# Status bar
status-ready = Prêt
status-loaded = { $file } chargé
status-saved = { $file } enregistré
import-error = Impossible d’ouvrir le profil : { $error }
save-error = Impossible d’enregistrer le profil : { $error }
file-filter-gremlin = Profil Joystick Gremlin

# Import report
alert-error = Un problème est survenu
alert-import = Certaines actions de { $file } ne fonctionneront pas
alert-open-log = Ouvrir le journal
ok = OK
hidhide-whitelist-error = Impossible d’ajouter Joystick Gobloon à la liste blanche de HidHide : { $error }
warn-not-executable = « { $kind } » × { $count } : conservé dans le profil, mais pas encore exécuté
warn-scripts = Le profil contient des modules Python : ils sont conservés, mais pas exécutés

# Map controls (device substitution)
subst-title = Associer les commandes
subst-auto = Auto
subst-auto-tip = Associer les périphériques restants aux périphériques branchés de nom similaire ; les cas douteux vous sont laissés
subst-banner = Les identifiants de périphériques du profil ne correspondent pas à ce PC : { $count }
access-banner = Joystick Gobloon a besoin de l’autorisation de lire les manettes et de créer des périphériques virtuels.
access-grant = Accorder l’accès
access-granted = Accès aux périphériques accordé.
access-error = Impossible d’accorder l’accès aux périphériques : { $error }
subst-keep = Laisser tel quel
subst-apply = Appliquer
status-substituted = Commandes associées. Enregistrez le profil pour conserver la modification.

# vJoy
vjoy-missing = vJoy n’est pas installé ou n’a pas pu être chargé.
vjoy-no-access = Pas l’autorisation de créer des périphériques vJoy.
vjoy-enabled = Pilote activé
vjoy-disabled = Pilote désactivé
vjoy-no-devices = Aucun périphérique vJoy configuré pour l’instant.
vjoy-axes = Axes
vjoy-buttons = Boutons
vjoy-hats = Chapeaux
vjoy-test-hint = Tester la sortie : verrouillez le périphérique pour utiliser les commandes ci-dessous.
vjoy-test-pending = Appliquez les modifications pour les tester.
vjoy-lock = Prendre le contrôle exclusif de ce périphérique pour tester sa sortie
vjoy-unlock = Libérer le périphérique
vjoy-free = Libre
vjoy-own = Contrôlé par Joystick Gobloon
vjoy-busy = Utilisé par un autre programme ou une autre copie de Joystick Gobloon
vjoy-unknown = État inconnu
vjoy-error = Impossible de prendre le périphérique : { $error }

# Device hiding
hidhide-missing = HidHide n’est pas installé.
hidhide-on = Le masquage des périphériques est activé
hidhide-off = Le masquage des périphériques est désactivé
hidhide-hidden = Masqué : seules les applications de la liste blanche le voient
hidhide-visible = Visible par toutes les applications
hidhide-filter-disconnected = Masquer les périphériques déconnectés
hidhide-gaming-only = Manettes de jeu uniquement
hidhide-enable = Activer le masquage des périphériques
hidhide-hint = Cliquez sur l’œil pour masquer un périphérique aux autres programmes (œil fermé). Reconnectez-le ensuite pour que la modification prenne effet.
hidhide-no-devices = Aucun périphérique à afficher.
action-refresh = Actualiser
settings-hidhide-self = Ajouter Joystick Gobloon à la liste blanche de HidHide au démarrage
settings-focus-input = Aller à l’entrée utilisée sur la page « Périphériques et affectations »
settings-input-events = Lire les manettes lors d’un changement
settings-input-events-tip = Moins de latence et de charge CPU : les manettes sont lues quand elles signalent un changement, et les appuis courts ne sont jamais perdus. Désactivé : chaque manette est lue toutes les 4 ms.
status-whitelisted = Joystick Gobloon a été ajouté à la liste blanche de HidHide.

# Applications
apps-add = Ajouter une application
apps-empty = Aucune application pour l’instant. Ajoutez un jeu ou un programme pour le configurer.
apps-missing = Fichier introuvable
apps-missing-text = Le programme n’existe plus. Le retirer de la liste ?
apps-sees = Voit les périphériques masqués
apps-blind = Ne voit pas les périphériques masqués
apps-inverse = Inverser la liste des applications
apps-inverse-tip = Les programmes listés ne voient pas les périphériques masqués ; tous les autres les voient
apps-remove = Retirer de la liste

# Engine
action-activate = Activer le profil
action-deactivate = Arrêter le profil
title-active = Actif
title-inactive = Inactif
status-active = Profil actif
status-stopped = Profil arrêté
activate-error = Impossible d’activer le profil : { $error }

# Editing
edit-add = Ajouter une action
edit-remove = Supprimer
edit-up = Monter
edit-down = Descendre
edit-not-editable = Cette action est conservée dans le profil, mais ne peut pas encore être modifiée.
edit-on-press = À l’appui
edit-on-release = Au relâchement
edit-on-both = À l’appui et au relâchement
edit-inverted = Inversé
edit-threshold = Seuil d’appui long
edit-short = Appui court
edit-long = Appui long
edit-deadzone = Zone morte
edit-invert = Inverser
edit-inherited = Hérité du mode « { $mode } »
edit-unbind = Supprimer toutes les actions de cette entrée
edit-override = Remplacer dans ce mode
input-axis-kind = Axe
input-button-kind = Bouton
input-hat-kind = Chapeau
kind-map-to-vjoy = Affecter à vJoy
kind-response-curve = Courbe de réponse
kind-change-mode = Changer de mode
kind-tempo = Appui court / long
kind-description = Description

# Files and modes
action-new = Nouveau profil
action-save-as = Enregistrer sous…
status-new = Nouveau profil
unsaved-title = Modifications non enregistrées
unsaved-text = Enregistrer les modifications de { $file } ?
unsaved-save = Enregistrer
unsaved-discard = Ne pas enregistrer
cancel = Annuler
modes-edit = Modifier les modes
modes-title = Modes
modes-name = Nom
modes-parent = Hérite de
modes-remove = Supprimer le mode et ses affectations
modes-new = Nouveau mode
modes-add = Ajouter un mode

# Undo
action-undo = Annuler (Ctrl+Z)
action-redo = Rétablir (Ctrl+Y)
status-undone = Annulé
status-redone = Rétabli
edit-curve-help = Faites glisser un point ou une poignée pour le déplacer · Clic dans le vide pour inverser · Double-clic pour ajouter un point · Clic droit sur un point pour le supprimer

# vJoy mapping
vjoy-device-n = vJoy #{ $n }
vjoy-axis-1 = Axe X
vjoy-axis-2 = Axe Y
vjoy-axis-3 = Axe Z
vjoy-axis-4 = Rotation X
vjoy-axis-5 = Rotation Y
vjoy-axis-6 = Rotation Z
vjoy-axis-7 = Curseur
vjoy-axis-8 = Molette
vjoy-button-n = Bouton { $n }
vjoy-hat-n = Chapeau { $n }
vjoy-absolute = Absolu
vjoy-relative = Relatif
vjoy-speed = Vitesse

# Mouse
kind-map-to-mouse = Affecter à la souris
mouse-mode-button = Bouton de souris
mouse-mode-motion = Mouvement du curseur
mouse-left = Bouton gauche
mouse-right = Bouton droit
mouse-middle = Bouton du milieu
mouse-forward = Bouton Suivant
mouse-back = Bouton Précédent
mouse-wheel-up = Molette vers le haut
mouse-wheel-down = Molette vers le bas
mouse-direction = Direction
mouse-min-speed = Vitesse minimale
mouse-max-speed = Vitesse maximale
mouse-time-to-max = Temps jusqu’à la vitesse maximale

# Toggle, split, pause, run
kind-smart-toggle = Bascule intelligente
kind-split-axis = Diviser l’axe
kind-pause-resume = Pause / reprise
kind-run-command = Exécuter une commande
edit-delay = Durée de maintien pour le mode momentané
edit-split = Diviser à
edit-lower = Partie basse
edit-upper = Partie haute
pause-op-pause = Pause
pause-op-resume = Reprendre
pause-op-toggle = Basculer
run-executable = Programme
run-arguments = Arguments
run-browse = Choisir un programme
status-paused = Profil en pause

# Double tap, chain
kind-double-tap = Double appui
kind-chain = Chaîne
edit-tap-threshold = Délai avant le second appui
edit-tap-exclusive = Exclusif : un appui simple attend pour écarter un double appui
edit-tap-combined = Combiné : un double appui exécute aussi les actions de l’appui simple
edit-single = Appui simple
edit-double = Double appui
edit-chain-timeout = Reprendre à la première étape après ce délai d’inactivité (0 = jamais)
edit-chain-step = Étape { $n }

# Hat as buttons
kind-hat-buttons = Chapeau comme boutons
edit-hat-four = 4 directions
edit-hat-eight = 8 directions (avec diagonales)
hat-north = Nord
hat-north-east = Nord-est
hat-east = Est
hat-south-east = Sud-est
hat-south = Sud
hat-south-west = Sud-ouest
hat-west = Ouest
hat-north-west = Nord-ouest
hat-center = Centre

# Merge axis
kind-merge-axis = Fusionner des axes
edit-reuse = Ajouter aussi ici cette action existante (partagée, comme Gremlin la lie aux deux axes)
edit-merge-name = Nom
edit-merge-no-device = (aucun périphérique)
edit-merged = Axe fusionné
merge-average = Moyenne
merge-minimum = Minimum
merge-maximum = Maximum
merge-sum = Somme
merge-bidirectional = Bidirectionnel (axe 2 moins axe 1)
merge-maximum-deflection = Déviation maximale
merge-closest-to-center = Le plus proche du centre

# Axis delta
kind-axis-delta = Variation d’axe
edit-delta-threshold = Variation de l’axe qui déclenche une impulsion
edit-delta-positive = L’axe a augmenté
edit-delta-negative = L’axe a diminué

# Dual axis deadzone
kind-dual-axis-deadzone = Zone morte à deux axes
edit-dual-inner = Zone morte intérieure (cercle autour du centre)
edit-dual-outer = Limite extérieure (déviation complète à partir d’ici)
edit-dual-close = Les zones mortes intérieure et extérieure sont trop proches.
edit-dual-first = Sortie du premier axe
edit-dual-second = Sortie du second axe

# Existing actions (Gremlin's reference action)
edit-link-existing = Lier une action existante
edit-copy-existing = Copier une action existante

# Condition
kind-condition = Condition
edit-cond-all = Toutes les conditions doivent être remplies
edit-cond-any = Une seule condition suffit
edit-cond-add = Condition
edit-cond-true = Si vrai
edit-cond-false = Sinon
cond-current = Cette entrée
cond-joystick = Entrée du joystick
cond-vjoy = Sortie vJoy
cond-keyboard = Touches maintenues
cond-range = Axe entre
cond-pressed = Appuyé
cond-released = Relâché

# Sound, speech, load profile
kind-play-sound = Jouer un son
kind-text-to-speech = Synthèse vocale
kind-load-profile = Charger un profil
file-filter-sound = Fichier son
edit-volume = Volume
edit-file-missing = Fichier introuvable
edit-tts-hint = Texte à prononcer ; $current_mode est remplacé par le mode actif
edit-tts-rate = Vitesse
edit-tts-pitch = Hauteur
edit-tts-back = Mettre en file après tout le reste
edit-tts-front = Mettre ensuite (voix du système : comme après tout le reste)
edit-tts-interrupt = Interrompre ce qui est en cours
edit-preview = Aperçu

kind-light-led = Éclairage LED
edit-led-device = Joystick avec LED
edit-led-no-device = Ce joystick n'est pas connecté ou ses LED ne peuvent pas être pilotées
edit-led-id = LED
led-vkb-grip = RVB en haut de la poignée
led-vkb-hat = Petite rouge près du chapeau
edit-led-color = Couleur

edit-led-color2 = Seconde couleur
edit-led-blend = Couleurs
led-blend-color1 = Une couleur
led-blend-color2 = Seconde couleur seule
led-blend-alternate = Alterner les couleurs
led-blend-alternate-reverse = Alterner, la seconde d'abord
led-blend-mix = Mélanger les couleurs
led-vkb-base-blue = Base, bleue (droite)
led-vkb-base-red = Base, rouge (gauche)
edit-led-mode = Mode d'éclairage
led-mode-on = Allumée
led-mode-slow-blink = Clignotement lent
led-mode-fast-blink = Clignotement rapide
led-mode-fastest-blink = Clignotement très rapide
led-mode-off = Éteinte
edit-led-test = Allumer maintenant
edit-led-on-activate = À l'activation du profil ou du mode
edit-tts-voice = Voix
edit-tts-default-voice = Voix par défaut
edit-tts-voice-missing = non installée
kind-map-to-keyboard = Affecter au clavier

# Key recording
keys-record = Enregistrer des touches : appuyez dessus ensemble, l’enregistrement s’arrête quand toutes sont relâchées
keys-press = Appuyez sur les touches…
keys-none = Aucune touche
keys-cancel = Annuler l’enregistrement
edit-chain-add = Ajouter une étape
edit-chain-remove = Supprimer cette étape et ses actions
kind-macro = Macro

# Macro editor
macro-repeat = Répéter
macro-repeat-single = Une fois
macro-repeat-count = Nombre de fois
macro-repeat-hold = Tant que maintenu
macro-repeat-toggle = Jusqu’au prochain appui
macro-repeat-count-tip = Combien de fois
macro-repeat-delay = Pause entre les répétitions
macro-add-step = Étape
macro-step-key = Touche
macro-step-pause = Pause
macro-step-mouse-button = Bouton de souris
macro-step-mouse-motion = Mouvement de souris
macro-step-vjoy-button = Bouton vJoy
macro-step-vjoy-axis = Axe vJoy
macro-step-vjoy-hat = Chapeau vJoy
macro-press = Appuyer
macro-release = Relâcher

# Logical device
logical-device = Périphérique logique
logical-device-tip = Entrées virtuelles de ce profil : affectez-y des entrées, puis liez-les comme un joystick
logical-add-axis = Ajouter un axe
logical-add-button = Ajouter un bouton
logical-add-hat = Ajouter un chapeau
logical-remove = Supprimer cette entrée et ses affectations
logical-empty = Aucune entrée logique pour l’instant : ajoutez-en une avec les boutons ci-dessus.
kind-map-to-logical-device = Affecter au périphérique logique
cond-logical = Entrée du périphérique logique

# Tray and startup
settings-tray = Afficher l’icône dans la zone de notification
settings-close-to-tray = Fermer dans la zone de notification
settings-minimize-to-tray = Réduire dans la zone de notification
settings-tray-double-click = Un double-clic active le profil
settings-tray-title = Icône de notification
settings-autostart = Démarrer à l’ouverture de session (réduit)
settings-activate-on-start = Activer le profil au démarrage
tray-show = Afficher Joystick Gobloon
tray-hiding = Masquage des périphériques
tray-activate = Activer « { $name } »
tray-activate-none = Activer le profil
tray-quit = Quitter
autostart-error = Impossible de modifier le démarrage à l’ouverture de session : { $error }

# About
about-alpha = version alpha
about-created = Créé par { $name }
about-support = Soutenir le développement
about-support-tip = Contribuer via PayPal
about-source = Code source
about-issues = Signaler un problème
about-releases = Versions
about-hiding = Masquage des périphériques : { $backend }
about-virtual = Joysticks virtuels : { $backend }
about-not-installed = non installé
about-gremlin = Les profils sont compatibles avec Joystick Gremlin.
about-license = Logiciel libre et open source sous licence MIT.

# Updates
settings-check-updates = Rechercher une nouvelle version au démarrage et l’installer
update-check = Rechercher des mises à jour
update-checking = Recherche d’une nouvelle version…
update-latest = Vous avez la dernière version
update-available = La version { $version } est disponible
update-install = Mettre à jour
update-downloading = Téléchargement de la version { $version }…
update-error = Impossible de rechercher des mises à jour
update-debug = Les versions de débogage ne se mettent pas à jour
update-install-error = Impossible d’installer la mise à jour : { $error }

# Input viewer
viewer-history = Axes dans le temps
viewer-current = Axes maintenant
viewer-buttons = Boutons et chapeaux
viewer-axis = Axe { $n }

# vJoy configuration
vjoy-add = Ajouter un périphérique vJoy
vjoy-add-max = vJoy prend en charge 16 périphériques au maximum.
vjoy-new-device = Nouveau vJoy #{ $n } : pas encore créé
vjoy-enable = Activer le pilote vJoy
vjoy-disable = Désactiver le pilote vJoy
vjoy-hat-continuous = Continu
vjoy-hat-discrete = 4 directions
vjoy-ffb = Retour de force
vjoy-ffb-enable = Activé
vjoy-ffb-const = Constante
vjoy-ffb-ramp = Rampe
vjoy-ffb-sq = Carré
vjoy-ffb-sine = Sinus
vjoy-ffb-tr = Triangle
vjoy-ffb-stup = Dent de scie montante
vjoy-ffb-stdn = Dent de scie descendante
vjoy-ffb-spr = Ressort
vjoy-ffb-dm = Amortisseur
vjoy-ffb-inr = Inertie
vjoy-ffb-fric = Friction
vjoy-config-create = Créer
vjoy-config-apply = Appliquer
vjoy-config-revert = Rétablir
vjoy-config-remove = Supprimer le périphérique
vjoy-config-admin = Windows demande les droits d’administrateur pour modifier vJoy
vjoy-config-locked = Arrêtez le profil pour modifier les périphériques vJoy.
vjoy-config-reading = Lecture de la configuration vJoy...
vjoy-config-label = configurer vJoy #{ $n }
vjoy-config-done = Configuration vJoy modifiée.
vjoy-config-error = Impossible de modifier la configuration vJoy : { $error }
vjoy-config-reboot = Configuration vJoy modifiée. Redémarrez Windows pour qu’elle prenne effet.
vjoy-remove-title = Supprimer vJoy #{ $n } ?
vjoy-remove-text = Les jeux et profils qui utilisent ce périphérique le perdent jusqu’à ce qu’il soit recréé.

# Response curve
curve-type = Type de courbe
curve-linear = Linéaire par morceaux
curve-cubic = Spline cubique
curve-bezier = Spline de Bézier cubique
curve-symmetric = Symétrique
curve-symmetric-tip = Refléter la courbe par rapport au centre pendant l’édition
curve-point = Point
curve-handle = Poignée

# Keyboard device
keyboard-device = Clavier
keyboard-tip = Touches du clavier comme entrées : liez des actions aux touches comme aux boutons d’un joystick
keyboard-add = Lier une touche
keyboard-empty = Aucune touche liée pour l’instant : enregistrez-en une avec le bouton ci-dessus.
edit-binding-description = Description de cette entrée
edit-unbound = Aucune action pour l’instant : ajoutez-en une.
viewer-detach = Ouvrir dans une fenêtre séparée
viewer-attach = Revenir dans la fenêtre principale
viewer-in-window = Le visualiseur d’entrées est ouvert dans sa propre fenêtre.

# Calibration
calibration-device = Périphérique à calibrer
calibration-raw = Brut
calibration-with-center = Avec centre
calibration-reset = Réinitialiser
calibration-save = Enregistrer ce calibrage
calibration-center = Calibrer le centre
calibration-center-tip = Laissez le manche au repos, bougez-le légèrement autour du centre, puis cliquez à nouveau
calibration-extrema = Calibrer les extrémités
calibration-extrema-tip = Amenez l’axe aux deux extrémités, puis cliquez à nouveau
calibration-in-window = Le calibrage est ouvert dans sa propre fenêtre.

# Device info
device-info = Informations sur le périphérique

device-leds = LED pilotables (action Éclairage LED)
device-info-name = Nom du périphérique
device-info-axes = Axes
device-info-buttons = Boutons
device-info-hats = Chapeaux
device-info-vid = VID
device-info-pid = PID
device-info-joystick-id = ID du joystick
device-info-guid = GUID du périphérique
device-info-copy = Copier dans le presse-papiers
