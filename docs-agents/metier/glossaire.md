# Glossaire SRS

Vocabulaire du domaine. Les identifiants de code sont en `monospace`.

| Terme | Définition |
|-------|-----------|
| **SRS** | DCS-SimpleRadioStandalone. Radio vocale autonome pour le simulateur **DCS World**. Remplace un plugin TeamSpeak historique. |
| **DCS** | Digital Combat Simulator (Eagle Dynamics). Le jeu. Expose une API Lua (Export, GameGUI, Hooks). |
| **Client** | `SR-ClientRadio.exe` (projet `DCS-SR-Client`). Appli WPF du pilote : capture micro, encode, envoie au serveur, reçoit, mixe, joue. |
| **Serveur** | `SR-Server.exe` (GUI WPF, projet `Server`) ou `SRS-Server-Commandline` (headless Windows/Linux, projet `ServerCommandLine`). Route la voix entre clients selon les règles radio. |
| **Export.lua** | Script Lua exécuté par DCS à chaque frame d'export. SRS y injecte une ligne qui charge `DCS-SimpleRadioStandalone.lua`, lequel lit les radios du cockpit et les envoie au client en UDP. |
| **GameGUI / Hook** | Scripts Lua exécutés dans l'environnement « GameGUI » de DCS (menu, overlay in-game, auto-connect). |
| **Module (avion)** | Fichier Lua `Scripts/DCS-SRS/Scripts/DCS-SRS-Modules/<Avion>.lua` décrivant comment lire les radios d'un cockpit donné. |
| **FC3 / low-fidelity** | Avions Flaming Cliffs (F-15C, Su-27…) sans cockpit cliquable. Les radios sont contrôlées depuis l'overlay SRS, pas depuis le cockpit. |
| **Full fidelity / clickable** | Avions à cockpit cliquable (A-10C, F/A-18C…). SRS lit la fréquence, le volume, le chiffrement directement depuis le cockpit. |
| **Overlay** | Fenêtre semi-transparente du client affichant les radios (fréquence, volume, actif, RX/TX). Variante AWACS plus large. Une version in-game (Lua) existe aussi pour la VR. |
| **PTT** | Push-To-Talk. Touche/bouton maintenu pour émettre. Peut venir d'un raccourci SRS, ou du cockpit DCS (in-cockpit PTT). |
| **Radio active / selected** | La radio sur laquelle le PTT commun émet. Indice `selected` dans les données exportées. |
| **Modulation** | Type de signal : AM, FM, INTERCOM, DISABLED, HAVEQUICK, SATCOM, MIDS, SINCGARS (enum `Modulation`). |
| **Guard** | Fréquence secondaire de détresse (121.5 AM, 243.0 AM) écoutée en plus de la fréquence principale (`secFreq`). |
| **Encryption / Enc / KY-58** | Chiffrement simulé : un simple entier `encKey` (1-252). Deux radios se comprennent si même clé. Le KY-58 est l'appareil réel modélisé dans A-10C, F-16, F/A-18. |
| **HAVE QUICK (HQ)** | Mode anti-brouillage UHF à saut de fréquence. En SRS : modulation spéciale, seuls les avions HQ s'entendent. |
| **MIDS** | Liaison de données Link-16 avec canaux voix (126 canaux). En SRS : modulation 6, fréquences virtuelles. |
| **Intercom** | Canal interne à un appareil multi-équipage (UH-1H, F-14, AH-64…). Modulation INTERCOM. |
| **EAM** | External AWACS Mode. Permet d'être connecté sans être dans un avion DCS (GCI, ATC via LotATC, autre jeu). Protégé par mot de passe de coalition. |
| **Coalition** | Rouge (red), Bleu (blue), Spectateur. Avec « Secure Coalition Radios », seuls les membres d'une même coalition s'entendent. |
| **LOS** | Line Of Sight. Option serveur : pas de réception si le terrain bloque la ligne de vue. Calculé via export DCS. |
| **Distance Limit** | Option serveur : portée max dépendant de la puissance de la radio. Atténuation progressive. |
| **Expansion radios** | Option serveur ajoutant des radios « virtuelles » aux avions qui en ont peu, pilotées depuis l'overlay. |
| **Retransmit (rtMode)** | Relais : une radio en mode retransmit réémet ce qu'elle reçoit sur ses autres radios (nœuds/hops limités côté serveur). |
| **Presets / channelisation** | Fichiers `.txt` listant des fréquences nommées par radio, ou `awacs-radios.json` définissant les radios EAM. Peuvent être poussés par le serveur. |
| **Profil** | Jeu de réglages client (touches, effets, audio) sauvegardé sous un nom. Sélection auto par type d'avion possible. |
| **LotATC** | Logiciel tiers de contrôle aérien pour DCS. SRS s'y intègre (export transpondeur, statut connexion). |
| **VAICOM** | Plugin tiers de commande vocale. SRS peut inhiber son TX. |
| **ExternalAudio** | Outil CLI (`DCS-SR-ExternalAudio`) pour injecter du TTS ou un MP3 sur une fréquence depuis un script de mission. |
| **AutoUpdater** | `SRS-AutoUpdater.exe` : télécharge la dernière release GitHub et lance l'installeur. |
| **Opus** | Codec audio utilisé pour la voix (lib Concentus en C#). |
| **IRL Tx/Rx behaviour** | Options serveur : comportement réaliste (ne pas s'entendre soi-même en émettant, etc.). |
