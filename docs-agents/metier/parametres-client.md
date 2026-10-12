# Paramètres client (vue métier)

Source : wiki `Settings-(WIP).md` et `General.md`. Clés techniques (`GlobalSettingsKeys`,
`ProfileSettingsKeys`) dans `docs-agents/architecture/modeles-donnees.md`.

Deux niveaux :

- **Global** : un seul jeu, indépendant du profil.
- **Profil** : un jeu par profil (touches, effets, canaux audio). Sélection auto par avion possible.

## Écran General

Micro (choix + aperçu), sortie haut-parleurs + sortie micro optionnelle, boost, adresse serveur
`ip:port` + favoris, Connect, « show server settings », toggles overlay radio / AWACS / liste
clients, dossier Presets, bloc EAM (mot de passe + nom), pied de page (profil courant, unité DCS,
nb clients, position, statut Serveur / VOIP / Jeu / LotATC).

## Global settings

| Option | Effet |
|--------|-------|
| Auto Connect | Se connecter au serveur annoncé par le serveur DCS. |
| Auto Connect Prompt | Demander avant de se connecter. |
| Auto Connect Mismatch Prompt | Demander si déjà connecté à un autre serveur. |
| Reset Radio Overlay | Remet position/opacité par défaut. |
| Hide Overlay Taskbar Item | L'overlay n'a pas d'entrée dans la barre des tâches. |
| Auto Refocus DCS | Redonne le focus à DCS après manipulation de l'overlay. |
| Allow More Input Devices | Accepte presque tout périphérique DirectInput (hors souris). |
| Microphone AGC / Noise Suppression | Gain automatique / réduction de bruit. |
| Minimize to tray / Start minimized | Comportement fenêtre. |
| Auto Select Profile For Aircraft | Charge le profil portant le nom de l'avion. |
| Check for beta updates | Inclut les bêtas. |
| Play connection sounds | Sons de connexion. |
| Set SRS Path for DCS | Chemin d'installation DCS. |
| Require Admin | Lance en admin (capture des touches partagées avec DCS). |
| Allow VAICOM TX Inhibit | Laisse VAICOM bloquer l'émission. |
| Show Transmitter Name | Affiche le nom de l'émetteur (si serveur OK). |
| Allow Anonymous Usage Stats | Statistiques anonymes. |

## Profile settings

**Controls / Cockpit integration** : Radio Switch works as PTT, Auto Select First Channel Preset,
ALWAYS allow SRS Hotkeys, ALWAYS allow Transponder Overlay Controls, Allow INCOCKPIT DCS Controlled
PTT, Rotary Style Frequency Increment (123.900 → 123.000 au lieu de 124.000), PTT Release Delay (ms),
PTT Start Delay (ms).

**Radio Effects** : Rx Effects (start/end), Tx Effects (start/end), son de début/fin, Encryption
Effects, MIDS Tx & Rx effects, Radio Voice Effect (filtre), Clipping, FM Radio Tone (+ volume),
HAVEQUICK Effect, Background Radio Effects UHF/VHF/HF/FM (+ volumes), bruit ambiant.

**Audio** : par radio et intercom, canal gauche/droite/les deux et volume par canal.

## Controls (bindings)

Par radio N : « Radio N » + modificateur. Commun : PTT + modificateur, Overlay toggle + modificateur,
AWACS overlay, fréquence +/- (grand et petit pas), canal +/-, volume, intercom PTT, transpondeur…
Les touches peuvent être clavier ou boutons joystick/HID.

## Favoris

Liste de serveurs (nom, adresse, mot de passe EAM) stockée dans un fichier à côté des réglages.
