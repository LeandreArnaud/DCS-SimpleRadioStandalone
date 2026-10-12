# Paramètres serveur (vue métier)

Source : wiki `SRS-Server-(WIP).md` + GUI serveur. Les clés techniques (`ServerSettingsKeys`) et
leurs défauts sont dans `docs-agents/architecture/modeles-donnees.md`. Stockés dans `server.cfg`
(chemin alternatif via `-cfg=`). Un seul port (défaut **5002**) en **TCP et UDP**.

| # | Option GUI | Effet |
|---|-----------|-------|
| 1 | Start/Stop | Démarre/arrête l'écoute. |
| 2 | Clients | Nombre de clients connectés. |
| 3 | Port | Port d'écoute (TCP+UDP). |
| 4 | Client Admin | Liste des clients, kick, ban (fichier `banned.txt`, redémarrage pour débannir). |
| 5 | Secure Coalition Radios | Chaque coalition n'entend que la sienne. |
| 6 | Spectator Audio | Les spectateurs peuvent parler/écouter comme en FC3. |
| 7 | Auto Export List | Écrit `clients-list.json` toutes les 30 s (pour pages web). |
| 8 | Line of Sight | Pas de réception sans ligne de vue terrain. |
| 9 | Distance Limit | Portée limitée selon puissance radio. |
| 10 | IRL Radio Tx Behaviour | Comportement réaliste en émission. |
| 11 | IRL Radio Rx Behaviour | Comportement réaliste en réception. |
| 12 | Radio Expansion | Ajoute des radios virtuelles aux avions pauvres en radios. |
| 13 | Allow Radio Encryption | Active le chiffrement simulé. |
| 14 | Strict Radio Encryption | Radio chiffrée → les transmissions en clair deviennent du bruit. |
| 15 | Check for beta updates | Inclut les pré-releases dans la vérification de mise à jour. |
| 16 | External AWACS Mode | Autorise les connexions EAM. |
| 17/18 | EAM blue/red password | Mots de passe de coalition EAM. |
| 19 | Test Frequencies AM (MHz) | Fréquences d'écho pour test micro. |
| 20 | Show Tuned/Client Count | Affiche le nombre de clients sur une fréquence. |
| 21 | Global Lobby Freq. AM (MHz) | Fréquences audibles par tous. |
| 22 | Ignore Radio Effect on Global Freq | Pas d'effets radio sur les fréquences globales. |
| 23 | LotATC Transponder Export | Diffuse les bips transpondeur via SRS. |
| 24 | Show Transmitter Name | Nom de l'émetteur visible côté client. |
| 25 | Log Received Transmissions | CSV quotidien `YYYY-MM-DD-transmissionlog.csv`. |
| 26 | Transmission Archive Time | Jours de rétention des logs. |
| 27 | Retransmit Hop/Node Count | Nombre max de sauts de relais. |
| — | Server Channel Presets | Pousse aux clients les fichiers du dossier `Presets/` à côté du `.cfg`. |

## Auto-connect (serveur DCS)

Fichier `Scripts/DCS-SRS-AutoConnectGameGUI.lua` à placer dans `Saved Games/DCS/Scripts/Hooks/` du
**serveur DCS**, en éditant `SRSAuto.SERVER_SRS_HOST = "ip:port"`. Les clients connectés au serveur
DCS reçoivent l'adresse et sont invités (ou connectés automatiquement selon leurs options).

## Démarrage headless

- Windows : `SR-Server.exe` (GUI) ou `SRS-Server-Commandline.exe`.
- Linux : binaire `SRS-Server-Commandline` self-contained publié par la release.
- Multi-instances : `-cfg=server02.cfg` et ports distincts.
