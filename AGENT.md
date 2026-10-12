# AGENT.md — guide de l'agent pour DCS-SimpleRadioStandalone (SRS)

SRS = radio vocale autonome pour le simulateur **DCS World**. Un **client WPF** (pilote) capture le
micro, lit les radios du cockpit via des scripts **Lua** exportés par DCS, encode en **Opus** et
envoie au **serveur** (WPF ou console, Windows/Linux) qui route la voix entre clients selon les règles
radio (fréquence, modulation, coalition, chiffrement, intercom). Solution .NET 10, C#, WPF, Lua 5.1.
Upstream : https://github.com/ciribob/DCS-SimpleRadioStandalone (wiki cloné dans `docs-agents/wiki-upstream/`).

## Règles non négociables
1. **Jamais de `git add` / `commit` / `stash` / `push`** ni de commande modifiant l'index ou
   l'historique sans demande explicite. Laisser les changements non stagés (plusieurs sessions
   agent tournent en parallèle sur ce repo). Lecture seule (`status`, `diff`, `log`, `show`) libre.
2. **La logique est dans `Common/`** (protocole, modèles, settings, moteur audio, serveur entier).
   Client et serveurs sont des hôtes. Chercher d'abord là.
3. **Protocole figé** : ne jamais réordonner `MessageType` ni changer la disposition de
   `UDPVoicePacket` sans bump de `UpdaterChecker.MINIMUM_PROTOCOL_VERSION`. N'ajouter les membres
   d'enums de settings qu'à la fin, ne jamais les renommer (ce sont les clés INI).
4. **Fréquences en Hz** (double) partout en interne ; MHz dans les fichiers et l'UI. Égalité radio =
   ±500 Hz. 11 radios max, index 0 = intercom.
5. **CRLF + BOM UTF-8** pour `.cs`, `.xaml`, `.lua` ; suivre le style du fichier voisin ; pas de
   reformatage massif. Lua formaté StyLua (`.stylua.toml`).
6. Un nouveau champ à propager réseau doit être ajouté dans `Equals`/`MetaDataEquals`/`DeepClone` et
   les copies manuelles de `ServerSync` et `TCPClientHandler`, sinon il n'est jamais envoyé.
7. WAV d'effets : 48 kHz / 16 bit / mono. Graphe audio RX : 48 kHz mono float.
8. Ne pas toucher aux fichiers parasites listés dans `docs-agents/guidelines/pieges-et-dette.md`
   ni au projet `DCS-SRS-Mobile` (abandonné) sans demande.
9. Build complet et tests = **Windows + .NET 10 SDK** seulement. Sur macOS/Linux : `Common`,
   `SharedAudio`, `ServerCommandLine`. Après un changement de package : `dotnet restore --force-evaluate`
   (lock files). Si on ne peut pas builder, le dire clairement.
10. Mettre à jour la doc `docs-agents/` concernée dans le même changement.

## Carte du dépôt
| Dossier | Rôle |
|---------|------|
| `Common/` | Cœur partagé net10.0 (Win+Linux) |
| `DCS-SR-Client/` | Client WPF `SR-ClientRadio.exe` |
| `Server/`, `ServerCommandLine/` | Hôtes serveur GUI / console |
| `Scripts/` | Lua DCS (export cockpit, GameGUI, overlay in-game, 67 modules avions) |
| `DCS-SR-Lua/` | `lua-srs.dll` NativeAOT (lance le client depuis DCS) |
| `DCS-SR-ExternalAudio/` | CLI TTS/MP3 vers une fréquence |
| `Installer/`, `AutoUpdater/` | Installation, mise à jour GitHub |
| `SharedAudio/` | DLL natives opus/lame (pas de code) |
| `DCS-SR-CommonTests/` | MSTest (7 tests) |
| `publish.ps1`, `.github/workflows/` | Build, signature, release |
| `docs-agents/` | Cette documentation |

Ports : serveur **5002 TCP+UDP** ; DCS↔client localhost UDP 9084 (radios), 9085/9086 (LOS),
5068 (siège), 5069 (auto-connect), 9040 (commandes), 7080 (overlay in-game), 10710/10711 (LotATC),
33501 (VAICOM).

## Sommaire de la documentation (`docs-agents/`)
Lire `docs-agents/README.md` pour le détail. Par besoin :

| Besoin | Lire |
|--------|------|
| Comprendre l'ensemble, trouver où toucher | `architecture/vue-d-ensemble.md` |
| Protocole TCP/UDP, règle « qui entend qui » | `architecture/protocole-reseau.md` |
| Modèles, enums, **toutes les clés de settings**, fichiers de données | `architecture/modeles-donnees.md` |
| `Common/` (carte, invariants) | `blocs/common.md` |
| Audio client (TX/RX, effets, modèles radio) | `blocs/client-audio.md` |
| UI WPF, entrées joystick, settings/profils | `blocs/client-ui-input-settings.md` |
| Connexion serveur, EAM, listeners DCS/LotATC | `blocs/client-network-dcs.md` |
| Serveur (TCP, routage, GUI, CLI, Linux) | `blocs/serveur.md` |
| Lua / DCS (export, GameGUI, modules avions, `lua-srs.dll`) | `blocs/lua-dcs-integration.md` |
| ExternalAudio, Installer, AutoUpdater, Mobile | `blocs/external-audio.md`, `blocs/installer-autoupdater.md` |
| Vocabulaire et règles métier | `metier/glossaire.md`, `metier/fonctionnalites-radio.md` |
| Options utilisateur (serveur, client), presets, avions | `metier/parametres-*.md`, `metier/presets-et-profils.md`, `metier/aeronefs-supportes.md` |
| Style de code | `guidelines/conventions-code.md` |
| Build, tests, exécution, versions | `guidelines/build-test-run.md` |
| Git, branches, CI, release | `guidelines/git-et-release.md` |
| **Recettes pas à pas** (option, binding, message, effet, avion…) | `guidelines/recettes.md` |
| Bugs connus, dette, fichiers morts | `guidelines/pieges-et-dette.md` |
| Wiki utilisateur brut | `wiki-upstream/` |

## Méthode pour une nouvelle feature
1. Lire `vue-d-ensemble.md` puis le bloc concerné et la recette la plus proche.
2. Identifier si la logique va dans `Common` (probable), le client, le serveur ou le Lua.
3. Si une option est impliquée : enum + défaut + UI + CLI + sync client (recette 1 ou 2).
4. Écrire le code dans le style du fichier, CRLF, strings en resx.
5. Tests dans `DCS-SR-CommonTests` pour tout ce qui est pur (protocole, helpers).
6. Mettre à jour `docs-agents/` ; laisser les changements non stagés ; résumer ce qui n'a pas pu
   être vérifié (build Windows, test en vol).
