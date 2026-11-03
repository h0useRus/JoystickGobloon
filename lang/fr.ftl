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
status-restored = Modifications non enregistrées de { $file } restaurées
status-switched = { $program } au premier plan : { $file } actif
status-switch-failed = { $program } au premier plan : impossible de charger { $file }
switch-unsaved-title = Changement de profil selon le programme
switch-unsaved-text = { $file } contient des modifications non enregistrées. Passer aux profils de vos programmes le fermerait : le changement reste désactivé jusqu’à l’enregistrement.
switch-save = Enregistrer l’actuel
switch-without = Continuer sans changement
switch-held-title = Le changement de profil selon le programme est en pause
switch-held-text = { $program } est au premier plan, mais { $file } contient des modifications non enregistrées. Son profil sera lancé une fois celles-ci enregistrées.
switch-save-continue = Enregistrer et changer
switch-stop = Arrêter le changement
switch-stopped-title = Profil arrêté
switch-stopped-text = Les périphériques de { $file } ont changé pendant son exécution :
switch-device-disconnected = { $device } a été déconnecté
switch-device-connected = { $device } a été connecté
switch-device-replaced = { $device } a été reconnecté avec un nouvel identifiant
switch-virtual-changed = Les joysticks ou manettes virtuels ont changé et ne suffisent plus au profil
switch-activate = Activer
status-switch-paused = Changement de profil selon le programme en pause : modifications non enregistrées
status-switch-stopped = Profil arrêté : ses périphériques ont changé
status-switch-off = Le changement de profil selon le programme est désactivé jusqu’à la prochaine activation
status-saved = { $file } enregistré
status-last-missing = Dernier profil introuvable : { $file }
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
virtual-banner = Ce profil utilise des périphériques virtuels absents de ce système ou configurés plus petits : { $devices }
virtual-setup = Configurer
virtual-setup-tip = Créer les périphériques virtuels manquants et ajouter aux existants ce qui leur manque. Rien n’est supprimé.
virtual-setup-admin = Créer les périphériques virtuels manquants et ajouter aux existants ce qui leur manque. Rien n’est supprimé. Chaque périphérique demande les droits d’administrateur.
virtual-setup-active = Arrêtez le profil pour modifier les périphériques virtuels.
virtual-setup-done = Les périphériques virtuels sont configurés pour le profil.
access-banner = Joystick Gobloon a besoin de l’autorisation de lire les manettes et de créer des périphériques virtuels.
access-grant = Accorder l’accès
access-granted = Accès aux périphériques accordé.
access-error = Impossible d’accorder l’accès aux périphériques : { $error }
keyboard-access-banner = Les affectations clavier ont besoin de l’autorisation de lire les claviers. Tout programme que vous lancez peut alors lire ce que vous tapez.
keyboard-access-allow = Autoriser la saisie clavier
keyboard-access-remove = Retirer l’accès au clavier
keyboard-access-allowed = Saisie clavier autorisée.
keyboard-access-removed = Accès au clavier retiré. Il prend fin quand le clavier est reconnecté ou que l’ordinateur redémarre.
keyboard-access-error = Impossible de modifier l’accès au clavier : { $error }
all-buttons-banner = { $device } a { $total } boutons, mais Linux ne transmet que les { $shown } premiers. Les programmes qui lisent directement le périphérique, comme Steam, pourront alors lire ce stick aussi, même masqué.
all-buttons-allow = Lire tous les boutons
all-buttons-allowed = Tous les boutons du stick sont lus désormais.
all-buttons-error = Impossible d’autoriser la lecture de tous les boutons : { $error }
subst-keep = Laisser tel quel
subst-apply = Appliquer
status-substituted = Commandes associées. Enregistrez le profil pour conserver la modification.

# vJoy
vjoy-missing = vJoy n’est pas installé ou n’a pas pu être chargé.
vjoy-no-access = Pas l’autorisation de créer des périphériques vJoy.
vjoy-enabled = Pilote activé
vjoy-disabled = Pilote désactivé
vjoy-no-devices = Aucun stick ni manette virtuels pour l’instant : ajoutez-en un avec les boutons ci-dessus.
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
hidhide-confirm-on-title = Activer le masquage des périphériques ?
hidhide-confirm-on-text = Seuls les programmes de la liste blanche verront les périphériques masqués.
hidhide-confirm-off-title = Désactiver le masquage des périphériques ?
hidhide-confirm-off-text = Tous les programmes voient de nouveau tous les périphériques, masqués compris.
hidhide-confirm-hint = Double-cliquez sur l’œil pour basculer sans cette question.
action-turn-on = Activer
action-turn-off = Désactiver
hidhide-hint = Cliquez sur l’œil pour masquer un périphérique aux autres programmes (œil fermé). Reconnectez-le ensuite pour que la modification prenne effet.
hidhide-exposed = Toujours visible pour les programmes qui lisent le périphérique brut (hidraw), p. ex. Steam ou Wine/Proton : le fermer demande le mot de passe administrateur, qui n'a pas été saisi. Affichez puis masquez de nouveau le périphérique pour qu'il soit redemandé.
hidhide-no-devices = Aucun périphérique à afficher.
action-refresh = Actualiser
settings-see-hidden = Voir les périphériques masqués
settings-see-hidden-tip = Au démarrage, le programme s’ajoute aux programmes qui voient encore les périphériques masqués, pour que les profils continuent de fonctionner.
settings-group-startup = Démarrage
settings-group-devices = Périphériques
settings-group-editor = Éditeur
settings-focus-input = Suivre l’entrée utilisée
settings-focus-input-tip = Quand un axe, un bouton ou un chapeau est utilisé, la page « Périphériques et affectations » sélectionne l’onglet de son périphérique et fait défiler la liste jusqu’à l’entrée.
settings-focus-skip-modifiers = Ne pas sauter aux modificateurs
settings-focus-skip-modifiers-tip = Appuyer sur un bouton ou une touche modificateur ne fait pas défiler la liste jusqu’à lui : l’entrée que vous configurez reste visible.
settings-input-events = Lire les manettes lors d’un changement
settings-input-events-tip = Moins de latence et de charge CPU : les manettes sont lues quand elles signalent un changement, et les appuis courts ne sont jamais perdus. Désactivé : chaque manette est lue à l’intervalle d’interrogation.
settings-poll-interval = Intervalle d’interrogation
settings-poll-interval-tip = Fréquence d’interrogation d’une manette : toutes les manettes si « Lire les manettes lors d’un changement » est désactivé, sinon seulement celles qui ne peuvent pas signaler les changements. Plus court : moins de latence et un peu plus de CPU.
settings-poll-interval-unused = Inutilisé pour l’instant : chaque manette connectée signale ses changements, aucune n’est donc interrogée. S’applique si « Lire les manettes lors d’un changement » est désactivé ou si une manette ne peut pas signaler ses changements.
settings-monotonic-curves = Les courbes ne font que monter ou descendre
settings-monotonic-curves-tip = Éditeur de courbe de réponse : un point ou une poignée s’arrête là où la courbe repartirait en arrière, elle ne croise donc zéro qu’une fois. Les courbes qui repartent déjà en arrière s’éditent librement.
settings-curve-right-click-invert = Le clic droit inverse les courbes
settings-curve-right-click-invert-tip = Éditeur de courbe de réponse : un clic droit dans le vide inverse la courbe. Le bouton Inverser fonctionne toujours.
settings-warn-gremlin = Avertir si l’enregistrement change le format du profil
settings-warn-gremlin-tip = Les profils avec des modificateurs ou des couches sont enregistrés dans un format que Joystick Gremlin ne peut pas ouvrir.
gremlin-format-title = Joystick Gremlin ne l’ouvrira pas
gremlin-format-text = { $file } utilise des fonctions propres à Joystick Gobloon (modificateurs et couches). Une fois enregistré, Joystick Gremlin ne pourra plus l’ouvrir.
gremlin-format-dont-ask = Ne plus demander
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
apps-add-running = Ajouter un programme en cours
apps-add-running-none = Aucun autre programme avec une fenêtre n’est en cours
apps-profile-none = Aucun profil
apps-profile-choose = Choisir un fichier…
apps-profile-tip = Profil actif tant que ce programme est au premier plan (lorsqu’un profil est activé)
apps-mode-start = Mode de départ
apps-mode-tip = Mode dans lequel le profil démarre pour ce programme
apps-own-no-profile = Joystick Gobloon ne choisit jamais de profil pour lui-même
apps-running-tip = Son profil est en cours

# Engine
action-activate = Activer le profil
action-deactivate = Arrêter le profil
title-active = Actif
title-inactive = Inactif
title-not-saved = non enregistré
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
send-mode-tip = Envoi : comment la sortie suit l'entrée
send-hold = Maintenir
send-hold-tip = Enfoncée tant que l'entrée est enfoncée
send-press = Appuyer seulement
send-press-tip = L'entrée l'enfonce, une autre entrée la relâche
send-release = Relâcher seulement
send-release-tip = L'entrée la relâche
send-pulse = Impulsion
send-pulse-tip = Un appui de durée fixe, quelle que soit la durée de l'entrée
send-repeat = Répéter
send-repeat-tip = Appuie encore et encore tant que l'entrée est enfoncée
send-toggle = Bascule
send-toggle-tip = Le premier appui la maintient, le suivant la relâche
send-pulse-ms = Durée de l'appui
send-delay-ms = Première répétition après
send-period-ms = Répéter toutes les
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
kind-map-to-vjoy = Affecter au stick virtuel
kind-response-curve = Courbe de réponse
kind-change-mode = Changer de mode
kind-tempo = Appui court / long
kind-description = Description
edit-rename = Cliquez pour renommer
edit-description-hint = Cliquez pour écrire une note
edit-click-to-edit = Cliquez pour modifier

# Files and modes
action-new = Nouveau profil
wizard-open = Nouveau profil avec l’assistant : chaque manette vers un joystick virtuel
wizard-title = Assistant de nouveau profil
wizard-name = Nom du profil
wizard-name-hint = p. ex. Mon simulateur de vol
wizard-create = Créer
wizard-devices = Choisissez le joystick virtuel de chaque manette. Chaque axe, bouton et chapeau reçoit un « Affecter au stick virtuel ».
wizard-none = Non associé
wizard-taken = Une autre manette utilise déjà ce joystick virtuel
wizard-no-vjoy = Pas encore de joysticks virtuels : ajoutez-les d’abord sur la page Joysticks virtuels
wizard-fewer = { $vjoy } a moins d’entrées que { $device } : { $missing } restent non associés
wizard-axes = { $n ->
    [one] 1 axe
   *[other] { $n } axes
}
wizard-buttons = { $n ->
    [one] 1 bouton
   *[other] { $n } boutons
}
wizard-hats = { $n ->
    [one] 1 chapeau
   *[other] { $n } chapeaux
}
status-wizard = Profil créé : { $name }
action-save-as = Enregistrer sous…
status-new = Nouveau profil
unsaved-title = Modifications non enregistrées
unsaved-text = Enregistrer les modifications de { $file } ?
unsaved-save = Enregistrer
unsaved-discard = Ne pas enregistrer
cancel = Annuler
modes-edit = Modifier les modes
modifiers-edit = Modificateurs
modifiers-none = Aucun modificateur pour l’instant. Un modificateur est un bouton qui, tant qu’il est maintenu, fait passer les autres entrées sur une autre couche d’actions.
modifiers-add = Ajouter un modificateur
modifiers-press = Appuyez sur un bouton d’une manette…
modifiers-assign = Cliquez, puis appuyez sur le bouton que ce modificateur doit utiliser
modifiers-remove = Supprimer le modificateur et les actions de ses couches
modifier-input = Modificateur { $name } : tant qu’il est maintenu, les autres entrées utilisent sa couche ; lui-même n’exécute aucune action
layer-base = Base
layer-set = Cette couche a des actions
layer-empty = Aucune action sur cette couche
layer-additive = Ajouter à la base
layer-additive-tip = Activé : ces actions s’exécutent avec la couche de base. Désactivé : elles la remplacent tant que les modificateurs sont maintenus.
layer-keep-value = Garder la dernière valeur
layer-keep-value-tip = Activé : quand cette couche s’arrête, ses axes gardent leur dernière valeur. Désactivé : ils reviennent au repos.

# Axis or hat as a button
vb-as-button = Traiter comme un bouton
vb-as-button-tip = Activé : ces actions fonctionnent comme un bouton, appuyé tant que l'axe est dans la plage (ou que le chapeau pointe dans une des directions choisies) et relâché quand il en sort.
vb-lower = Limite basse de la plage
vb-upper = Limite haute de la plage
vb-anywhere = Appuyer quel que soit le côté par lequel l'axe entre dans la plage
vb-from-below = Appuyer seulement quand l'axe monte dans la plage
vb-from-above = Appuyer seulement quand l'axe descend dans la plage
vb-add = Ajouter des actions qui fonctionnent comme un bouton (plage d'axe ou directions du chapeau)
vb-remove = Supprimer ces actions
layers-tip = Couches : { $layers }
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
edit-curve-help = Faites glisser un point ou une poignée pour le déplacer · Double-clic pour ajouter un point · Clic droit sur un point pour le supprimer
edit-curve-help-invert = Clic droit dans le vide pour inverser

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
vjoy-quickmap = Choisir une sortie
output-profile = Profil : { $name }
output-free = { $output } : libre
output-used = { $output } : { $n ->
    [one] utilisée par 1 entrée
   *[other] utilisée par { $n } entrées
}
output-conflict = Des entrées actives en même temps l’écrivent
output-more = …et { $n } de plus
output-legend-used = Utilisée
output-legend-conflict = Conflit
output-legend-tip = Afficher ces contrôles (le profil ne change pas)
output-same-input = Une autre action de cette entrée l’écrit déjà
output-same-input-clash = Une autre action de cette entrée écrit la même sortie : choisissez-en une autre
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
status-latency = Latence entre la lecture d’une manette et la sortie virtuelle, dernière seconde : moyenne { $average } ms, max { $max } ms. Le délai propre de la manette n’est pas inclus.

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
edit-merge-full = Elle fusionne déjà deux autres axes ; changez l’un d’eux dans son éditeur.
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
keys-other = Autres touches : F13–F24 et multimédia
keys-clear = Effacer les touches
key-play-pause = Lecture/pause
key-media-stop = Arrêt
key-previous-track = Piste précédente
key-next-track = Piste suivante
key-mute = Muet
key-volume-down = Baisser le volume
key-volume-up = Monter le volume
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
macro-step-vjoy-button = Bouton du stick virtuel
macro-step-vjoy-axis = Axe du stick virtuel
macro-step-vjoy-hat = Chapeau du stick virtuel
macro-step-pad-button = Bouton de manette
macro-step-pad-axis = Stick ou gâchette de manette
macro-step-pad-dpad = Croix directionnelle de manette
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
about-gremlin = Importe les profils Joystick Gremlin à partir de R14.
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
vjoy-add = Ajouter un stick virtuel
vjoy-add-max = vJoy prend en charge 16 périphériques au maximum.
vjoy-new-device = Nouveau vJoy #{ $n } : pas encore créé
vjoy-enable = Activer le pilote vJoy
vjoy-disable = Désactiver le pilote vJoy
vjoy-hat-continuous = Continu
vjoy-hat-discrete = 4 directions
vjoy-ffb = Retour de force
vjoy-ffb-enable = Activé
vjoy-name = Nom
vjoy-name-suggest = Proposer un nom : d'après la manette que le profil affecte à ce périphérique, ou d'après sa disposition
vjoy-name-invalid = Jusqu'à 79 caractères, sans espace au début ni à la fin, différent des noms des autres périphériques.
vjoy-name-renamed = Les jeux voient un périphérique renommé comme un nouveau : il faudra peut-être refaire leurs affectations.
vjoy-product = ID de produit
vjoy-product-tip = Les jeux distinguent les périphériques par cet ID. Vide = choisi d'après le numéro du périphérique ; saisissez l'ID d'une ancienne installation pour garder les affectations d'un jeu.
vjoy-product-invalid = Un nombre hexadécimal de BE01 à BEFF (sauf BEAD), non utilisé par un autre périphérique.
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
vjoy-config-done = Périphériques virtuels modifiés.
vjoy-config-error = Impossible de modifier la configuration vJoy : { $error }
vjoy-config-reboot = Configuration vJoy modifiée. Redémarrez Windows pour qu’elle prenne effet.
vjoy-remove-title = Supprimer vJoy #{ $n } ?
vjoy-remove-text = Les jeux et profils qui utilisent ce périphérique le perdent jusqu’à ce qu’il soit recréé.

# Response curve
curve-type = Type de courbe
curve-linear = Linéaire par morceaux
curve-cubic = Spline cubique
curve-monotone = Lisse, sans dépassement
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
voice-device = Voix
voice-tip = Commandes vocales : dites une phrase en maintenant une entrée « appuyer pour parler » ; chaque commande agit comme un bouton
voice-command-n = Commande { $n }
voice-no-button = Les commandes vocales ne sont entendues que lorsqu'une entrée avec l'action Entrée vocale est maintenue. Choisissez un bouton de joystick, une touche du clavier ou un bouton de manette, ajoutez-lui l'action Entrée vocale, puis ajoutez ici les commandes et leurs phrases.
voice-button = Écoute tant que maintenu : { $device } · { $input }
voice-button-go = Aller à cette entrée
voice-add = Commande
voice-empty = Aucune commande vocale pour l'instant : ajoutez-en une avec le bouton ci-dessus.
voice-remove = Supprimer cette commande et ses affectations
voice-phrases = Dites l'une de ces phrases :
voice-phrase-remove = Supprimer cette phrase
voice-phrase-add = Ajouter une phrase
voice-no-phrases = Aucune phrase pour l'instant : cette commande ne peut pas être entendue.
voice-hold = Durée pendant laquelle la commande maintient son bouton après avoir été entendue
kind-voice-input = Entrée vocale
voice-mic = Microphone
voice-mic-default = Microphone par défaut
voice-mic-missing = Ce microphone n'est pas branché : le microphone par défaut écoute en attendant.
voice-input-hint = Écoute les commandes vocales tant que cette entrée est maintenue. Les commandes se trouvent dans l'onglet Voix de la page Périphériques.
voice-no-model = Pas de modèle vocal pour les commandes vocales : téléchargez-le dans Paramètres, Voix.
voice-failed = Les commandes vocales ne fonctionnent pas : { $error }
voice-listening = Écoute…
voice-loading = Chargement du modèle vocal…
voice-heard = Entendu « { $text } » ({ $confidence } %) : { $command }
voice-heard-unknown = Entendu « { $text } » ({ $confidence } %) : aucune commande
voice-heard-nothing = Rien entendu
status-voice = Commandes vocales : maintenez l'entrée « appuyer pour parler » pour parler
settings-voice = Voix
settings-voice-model = Modèle vocal
settings-voice-model-hint = Les commandes vocales sont reconnues sur cet ordinateur : rien de ce que vous dites n'en sort. Chaque langue a besoin de son modèle vocal, téléchargé une fois.
settings-voice-language = Langue des commandes vocales
settings-voice-language-program = Comme le programme
settings-voice-no-model = Il n'y a pas encore de modèle vocal pour { $language } : choisissez une autre langue pour les commandes vocales.
settings-voice-download = Télécharger le modèle vocal ({ $size } Mo)
settings-voice-downloading = { $done } sur { $total } Mo
settings-voice-cancel = Annuler le téléchargement
settings-voice-installed = Modèle vocal installé ({ $size } Mo)
settings-voice-remove = Supprimer le modèle vocal
settings-voice-download-failed = Le modèle vocal n'a pas pu être téléchargé : { $error }
settings-voice-no-library = La bibliothèque de reconnaissance vocale est absente : réinstallez Joystick Gobloon.
settings-voice-microphone = Microphone
settings-voice-gain = Volume du microphone
settings-voice-gain-tip = Augmentez-le pour un microphone faible, baissez-le quand le niveau atteint le bout de la barre
settings-voice-test = Tester le microphone
settings-voice-test-hint = Parlez aussi fort que pour donner des commandes : la barre devrait atteindre à peu près le milieu, jamais le bout.
settings-voice-clipping = Trop fort
settings-voice-keep-open = Garder le micro ouvert
settings-voice-keep-open-tip = Activé : les micros du profil restent ouverts tant qu'il est actif, pour que les premiers mots après l'appui sur le bouton soient entendus (certains casques restent muets une ou deux secondes après l'ouverture). Rien n'est reconnu tant que le bouton n'est pas maintenu.
settings-voice-confidence = Confiance minimale
settings-voice-confidence-tip = Une commande entendue avec moins d'assurance ne fait rien : augmentez-la si d'autres paroles déclenchent des commandes, baissez-la si les commandes sont souvent mal reconnues
settings-voice-next-start = Les changements s'appliquent à la prochaine activation d'un profil.
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
calibration-jitter = Filtre de tremblement
calibration-jitter-tip = Ignore les changements plus petits que cette part de la course, pour les axes usés qui tremblent. Sans délai ; les extrémités et le centre sont toujours atteints. Le trait sur la barre montre ce qui est envoyé. Enregistrez pour appliquer
calibration-debounce = Anti-rebond
calibration-debounce-tip = Boutons et chapeaux de ce périphérique : après un appui ou un relâchement, les changements pendant ce délai sont ignorés, pour les contacts qui rebondissent. Le premier changement passe aussitôt
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

# Virtual gamepads
kind-map-to-gamepad = Vers une manette
gamepad-n = Manette { $n }
gamepad-tip = Manette virtuelle de style Xbox : les jeux, Steam et Proton voient une manette Xbox 360.
gamepad-add = Ajouter une manette virtuelle
gamepad-add-max = { $n } manettes au plus.
gamepad-add-n = Ajouter la manette { $n }
gamepad-delete-n = Supprimer la manette { $n }
gamepad-missing = La manette { $n } n’existe pas encore : ajoutez-la sur la page Joysticks virtuels.
pad-a = A
pad-b = B
pad-x = X
pad-y = Y
pad-lb = LB
pad-rb = RB
pad-back = Back
pad-start = Start
pad-guide = Guide
pad-ls = Clic du stick gauche
pad-rs = Clic du stick droit
pad-dpad-up = Croix haut
pad-dpad-down = Croix bas
pad-dpad-left = Croix gauche
pad-dpad-right = Croix droite
pad-left-x = Stick gauche X
pad-left-y = Stick gauche Y
pad-right-x = Stick droit X
pad-right-y = Stick droit Y
pad-lt = LT (gâchette gauche)
pad-rt = RT (gâchette droite)
pad-ls-short = LS
pad-rs-short = RS
pad-dpad = Croix directionnelle

system-banner-install = Installez System Gobloon : les sticks masqués le sont pour tous les programmes, et les périphériques virtuels restent présents pendant les parties, comme avec HidHide et vJoy sous Windows.
system-banner-stopped = System Gobloon est installé mais ne fonctionne pas.
system-banner-update = System Gobloon { $version } est installé ; ce programme apporte une autre version.
system-install = Installer
system-update = Mettre à jour
system-start = Démarrer
system-remove = Supprimer
system-not-now = Pas maintenant
system-admin = Demande le mot de passe administrateur
system-about = Masque les sticks pour tous les programmes et garde les joysticks, manettes, clavier et souris virtuels pendant les parties. Aucun mot de passe n'est nécessaire après l'installation.
system-status-missing = Non installé.
system-status-stopped = Installé, arrêté.
system-status-outdated = La version { $version } fonctionne : mettez-la à jour vers la version de ce programme.
system-status-current = La version { $version } fonctionne.
system-remove-title = Supprimer System Gobloon ?
system-remove-text = Les sticks masqués ne le seront plus que pendant que Joystick Gobloon fonctionne, et les périphériques virtuels n'existeront que pendant qu'un profil est actif. Les périphériques masqués et les dispositions des périphériques virtuels sont conservés.
system-installed = System Gobloon est installé.
system-removed = System Gobloon est supprimé.
system-error = System Gobloon : { $error }
vjoy-not-persistent = Ces périphériques n'existent que pendant qu'un profil est actif ou qu'un test les tient. System Gobloon (paramètres, Général) les garde en permanence.
hidhide-while-running = Les périphériques ne sont masqués que pendant que Joystick Gobloon fonctionne. System Gobloon (paramètres, Général) les masque en permanence pour tous les programmes.
kind-button-axis = Axe par boutons
bax-to-vjoy = Un axe d’un joystick virtuel
bax-to-gamepad = Un stick ou une gâchette d’une manette virtuelle
bax-to-logical = Un axe du périphérique logique
bax-decrease = Déplace l’axe vers le bas (l’entrée liée le déplace vers le haut)
bax-decrease-press = Appuyez sur un bouton ou une touche…
bax-decrease-none = Aucune entrée ne le déplace vers le bas
bax-decrease-listen = Écouter : appuyez sur le bouton ou la touche qui déplace l’axe vers le bas
bax-decrease-clear = Retirer l’entrée qui le déplace vers le bas
bax-motion-tip = Comment les boutons déplacent l’axe
bax-set = Valeur fixe
bax-set-tip = Un appui met l’axe à une valeur, l’autre entrée à l’opposé
bax-velocity = Vitesse
bax-velocity-tip = L’axe se déplace tant qu’un bouton est maintenu
bax-stepped = Crans
bax-stepped-tip = Un appui passe à la position suivante
bax-encoder = Encodeur
bax-encoder-tip = Un appui avance d’un pas, d’autant plus grand que les appuis sont rapides
bax-smooth = Doux
bax-smooth-tip = Un appui déplace la cible d’un pas, l’axe la suit en douceur
bax-value = Valeur
bax-return = Retour au relâchement
bax-speed = Vitesse
bax-acceleration = Accélération
bax-max-speed = Vitesse maximale
bax-step = Pas
bax-max-step = Plus grand pas
bax-fast-rate = Plus grand pas à
bax-spring = Retour au repos après
bax-response = Temps de réponse
bax-positions-tip = Clic ajoute une position, glisser la déplace, clic droit la retire
bax-even-count = Nombre de positions
bax-even = Répartir ce nombre de positions régulièrement
bax-start = Valeur de départ
bax-start-tip = Mettre l’axe à cette valeur au démarrage du profil ; désactivé, il reste où il est
