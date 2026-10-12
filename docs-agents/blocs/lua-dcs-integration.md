# Bloc : intégration DCS (Lua + lua-srs.dll)

Côté jeu. Deux environnements Lua distincts de DCS sont utilisés :

- **Export** (`Export.lua`) : accès au cockpit (`GetDevice`, `get_argument_value`, `LoGetSelfData`…).
  Script : `Scripts/DCS-SRS/Scripts/DCS-SimpleRadioStandalone.lua`.
- **GameGUI** (hooks `Scripts/Hooks/*.lua`) : accès réseau DCS (`net.*`, `DCS.*`), chat, dxgui, options.
  Scripts : `DCS-SRSGameGUI.lua`, `DCS-SRS-OverlayGameGUI.lua`.

Tout passe par **UDP localhost** vers/depuis le client C#. Logs dans `dcs.log`, tags `SRS-export`,
`SRS-GameGUI`, `SRS-OverlayGameGUI`, `SRS-AutoConnectGameGUI`.

## 1. Fichiers et déploiement

| Repo | Installé dans `Saved Games/DCS/` | Rôle |
|------|----------------------------------|------|
| `Scripts/Export.lua` | ligne **injectée** à la fin de `Scripts/Export.lua` | `pcall(dofile(writedir..Mods/Services/DCS-SRS/Scripts/DCS-SimpleRadioStandalone.lua))` |
| `Scripts/Hooks/DCS-SRS-hook.lua` | `Scripts/Hooks/` | `dofile` des deux scripts GameGUI en `pcall` |
| `Scripts/DCS-SRS-AutoConnectGameGUI.lua` | `Scripts/Hooks/` **du serveur DCS**, manuel | Annonce l'adresse SRS dans le chat aux joueurs |
| `Scripts/DCS-SRS/entry.lua` | `Mods/Services/DCS-SRS/` | `declare_plugin("DCS-SRS")` : binaire `lua-srs.dll`, page Options, skins, `version = "VersionPrefix"` |
| `Scripts/DCS-SRS/Scripts/DCS-SimpleRadioStandalone.lua` | `Mods/Services/DCS-SRS/Scripts/` | Cœur export (~920 lignes) : table `SR`, helpers, boucle, UDP, LOS |
| `Scripts/DCS-SRS/Scripts/DCS-SRS-Modules/*.lua` | idem | Un fichier par famille d'avion |
| `Scripts/DCS-SRS/Scripts/DCS-SRSGameGUI.lua` | idem | Nom/side/seat/slot, auto-connect, commandes chat, lancement client |
| `Scripts/DCS-SRS/Scripts/DCS-SRS-OverlayGameGUI.lua` | idem | Overlay in-game (VR), écoute UDP 7080 |
| `Scripts/DCS-SRS/UI/DCS-SRS-Overlay.dlg` | `Mods/Services/DCS-SRS/UI/` | Dialog dxgui de l'overlay |
| `Scripts/DCS-SRS/Options/*` | `Mods/Services/DCS-SRS/Options/` | Page Options → Special → DCS-SRS |
| `DCS-SR-Lua/` (C#) | `Mods/Services/DCS-SRS/bin/lua-srs.dll` | Module Lua natif (NativeAOT) |
| `Scripts/DCS-SRS/Scripts/DCS-Plugin-Example/` | doc | Modèle de plugin tiers |

Installeur : `Installer/MainWindow.xaml.cs` → `InstallScripts()` (remplace toute ligne contenant
`SimpleRadioStandalone.lua` dans Export.lua ou l'ajoute, copie le hook, copie l'arbre `DCS-SRS`).
`publish.ps1` copie `Scripts/` et remplace le littéral `VersionPrefix` dans tous les `.lua`.

## 2. Ports UDP (localhost)

| Port | Sens | Contenu |
|------|------|---------|
| 9084 | Export → Client | JSON état radios (toutes les 0,2 s) |
| 9086 | Client → Export | Requêtes LOS `[{id,lat,lng,alt}]` (par 10, toutes les 250 ms, si LOS serveur) |
| 9085 | Export → Client | Réponses LOS `[{id,los}]` |
| 9087 | GameGUI → Export | `{name, side, seat, slot}` |
| 5068 | GameGUI → Client | même payload seat/slot (`DCSGameGuiHandler`) |
| 5069 | GameGUI → Client | Auto-connect : texte `host:port` (`DCSAutoConnectHandler`) |
| 9040 | GameGUI/outils → Client | Commandes radio/transpondeur JSON (`UDPCommandHandler`) |
| 7080 | Client → Overlay GameGUI (+ panneaux externes) | `CombinedRadioState` JSON |

Côté client, `DCSLineOfSightHandler.cs`, `DCSGameGuiHandler`, `DCSAutoConnectHandler`,
`UDPCommandHandler.cs`, `DCSRadioSyncHandler` dans `DCS-SR-Client/Network/DCS/`.

## 3. Script export : fonctionnement

- Hook : remplace `LuaExportActivityNextEvent` (appelle `SR.exporter` toutes les 0,2 s puis le hook
  précédent, retourne le **min** des deux tNext) et `LuaExportBeforeNextFrame` (lit sockets LOS et
  seat, non bloquant). Tout en `pcall`. Compatible TacView/Helios/DCS-BIOS.
- Ajoute les chemins pour `require('terrain')` (LOS) et charge `Scripts/JSON.lua` de DCS.
- `SR.LoadModsPlugins()` : charge chaque module de `DCS-SRS-Modules/` puis
  `Mods/Aircraft|Tech/<mod>/SRS/autoload.lua` via `SR.LoadModule(path)` = `loadfile(path)().register(SR)`.
- Clé d'exporteur : `LoGetSelfData().Name` (type d'unité). Pas d'exporteur → FC3 (radios synthétiques
  pilotées par overlay, `freqMode/volMode=1`) ou slot spécial (Spectator, Game-Master,
  Tactical-Commander, JTAC-Operator ; `unitId = 100000001`, 11 radios fixes).
- Mort/éjection : `GetDevice(0)` nil ou nombre → payload spectateur.

### Payload envoyé (port 9084)

```
name (pilote), unit (type), unitId, selected (0-based, -1 aucun), ptt, control (0 HOTAS/overlay, 1 cockpit),
simultaneousTransmissionControl, seat, latLng{lat,lng,alt}, ambient{vol, abType},
intercomHotMic, capabilities{dcsPtt, dcsIFF, dcsRadioSwitch, intercomHotMic, desc},
iff{status, mode1, mode2, mode3, mode4, control, expansion, mic},
radios[1..n] : name, freq (Hz, 1 = éteinte), modulation, volume 0..1, secFreq, freqMin, freqMax,
  encKey, enc, encMode, freqMode, guardFreqMode, volMode, expansion, rtMode, model, channel, rxOnly, retransmit, simul
```

`radios[1]` est **toujours l'intercom** (modulation 2, model `intercom`). Modes : `freqMode/volMode/
guardFreqMode` 0 = cockpit, 1 = overlay ; `encMode` 0 aucun, 1 overlay (toggle + clé), 2 cockpit,
3 cockpit toggle + clé overlay ; `rtMode` 0 cockpit, 1 overlay, 2 désactivé. Enums C# miroir :
`DCS-SR-Client/Network/DCS/Models/DCSState/DCSRadio.cs`, `Common/Models/Player/Modulation.cs`.
MIDS : `freq = SR.MIDS_FREQ (1030 MHz) + canal × 100 kHz`, modulation 6.

### Helpers `SR` pour les modules

`getRadioFrequency(deviceId, roundTo, ignoreIsOn)`, `getRadioModulation(deviceId)`,
`getRadioVolume(deviceId, arg, {min,max}, invert)`, `getKnobPosition`, `getSelectorPosition(arg, step)`,
`getButtonPosition(arg)`, `getNonStandardSpinner`, `getListIndicatorValue(id)` (parse `list_indication`,
style DCS-BIOS), `getAmbientVolumeEngine()`, `getSpecialOption('Plugin.Key')` (mémoïsé), `rerange`,
`round`, `advRound`, `nearlyEqual`, `lastKnownSeat`, `lastKnownSlot`, `LastKnownUnitId`, `RadioModels`
(noms de presets → `DCS-SR-Client/RadioModels/<model>.json`, cf. `RadioModels/HOWTO.md`), `MIDS_FREQ*`.

### LOS

`SR.checkLOS` : `terrain.isVisible(x, y+offset, z, x2, alt+20, z2)` avec offset 20→200 m par pas de
20 m. `los` = 0 (dégagé), `offset/200` (partiel), 1 (bloqué). Le client met `LineOfSightLoss` sur le
`SRClientBase`. L'atténuation distance est calculée côté client ; le Lua ne fait que le LOS.

## 4. GameGUI

### `DCS-SRSGameGUI.lua`
- `require("srs")` (lua-srs.dll) en `pcall` ; si absent, seul l'auto-lancement du client est perdu.
- Callbacks `DCS.setUserCallbacks` : `onPlayerChangeSlot`, `onSimulationFrame` (toutes les 5 s →
  envoi seat/slot sur 5068 et 9087), `onChatMessage`.
- **Auto-connect** : le hook serveur envoie dans le chat `"SRS Running on <port>"` (hôte =
  `net.get_server_host()`) ou `"SRS Running @ host:port"`. Le client Lua n'accepte que `from == 1`
  (hôte), éventuellement lance `srs.start_srs(host)`, puis envoie `host:port` sur 5069.
- **Commandes chat** `SRSRADIO …` / `SRSTRANS …` (de soi-même) → JSON `{Command, RadioId, Frequency,
  Volume, Enabled, Code}` sur 9040. Types dans `UDPInterfaceCommand.UDPCommandType` (0 FREQUENCY_DELTA,
  1 ACTIVE_RADIO, 2 TOGGLE_GUARD, 3/4 CHANNEL_UP/DOWN, 5 SET_VOLUME, 6-10 transpondeur, 11 GUARD,
  12 FREQUENCY_SET, 13 M2, 14-19 retransmit/simul, 20 INTERCOM_CHANNEL). Interface générique aussi
  utilisable par des outils externes.
- **PTT** : aucun chemin Lua pour le PTT clavier. Le PTT vient des bindings du client C# ou du switch
  cockpit lu par le module export (`_data.ptt`, `capabilities.dcsPtt`).

### `DCS-SRS-OverlayGameGUI.lua`
- `module("srs_overlay")`, config `Config/SRSConfig.lua` (mode, hotkey `Ctrl+Shift+escape`, position).
- Écoute 7080, décode `CombinedRadioState` (`RadioInfo`, `RadioSendingState`, `RadioReceivingState`,
  `ClientCountConnected`, `TunedClients`). Modes : full → minimum → minimum_vol → txrx_only → hidden.
- `module_specific[unit]` : retours cockpit (M-2000C, UH-60L, FA-18C/F-16C voyants VoIP HOTAS).
- Options lues via `OptionsData.getPlugin("DCS-SRS", key)`.

### Options (`Options/optionsDb.lua`)
`srsAutoLaunchEnabled` (true), `srsOverlayEnabled` (true), `srsOverlayCompactModeEnabled` (false),
`srsOverlayHelpTextEnabled` (true). Lues en jeu, **pas** transmises au client C#.

## 5. `DCS-SR-Lua` → `lua-srs.dll`

- `net10.0-windows`, `PublishAot=true`, `AssemblyName=lua-srs`, référence `Common`.
- `[UnmanagedCallersOnly(EntryPoint="luaopen_srs")]` → `require("srs")` via cpath `bin/lua-?.dll`.
- `Native.cs` : `[LibraryImport("lua")]` sur l'API Lua 5.1 (lua.dll de DCS). `lua51/*.h` = en-têtes
  de référence, non compilés. `State.cs` : wrapper. `SRS.cs` : table `srs` avec `start_srs(host)`
  (registre `HKCU\SOFTWARE\DCS-SR-Standalone\SRPathStandalone`, refuse si `sr-clientradio` tourne,
  lance `Client\SR-ClientRadio -host=<host>`), `get_srs_path()`, `srs.VERSION`.
- Raison d'être : le Lua GameGUI ne peut ni lire le registre ni lancer un processus.

## 6. Plugins tiers

`Mods/Aircraft/<Mod>/SRS/autoload.lua` retournant `{ register = function(SR) SR.exporters["<unit>"] =
fn end }`. Même contrat qu'un module interne. Exemple : `DCS-Plugin-Example/SRS/autoload.lua`.

## 7. Conventions Lua

- StyLua (`.stylua.toml`) : Lua51, 120 colonnes, 4 espaces, **CRLF**, guillemets doubles, parenthèses
  toujours, pas de collapse. `Export.lua` ignoré. Certains fichiers GameGUI ne sont pas encore formatés.
- Fonction exporteur `exportRadio<Unit>(_data, SR)` retournant `_data`. Fichier = nom court du type.
  État privé dans une table locale (`_f16`, `_fa18`). Locaux préfixés `_`.
- Jamais de version en dur : littéral `VersionPrefix`.
- Logs : `log.write(tag, log.INFO|ERROR|DEBUG|TRACE, str)` ; hook : `net.log`.

## 8. Pièges

- Ids d'arguments/devices cockpit cassent aux mises à jour DCS ; comparer les floats avec
  `SR.nearlyEqual` / `SR.round`.
- `terrain` doit être requérable sinon le LOS échoue silencieusement (« Terrain Export is not working »).
- Données export > 10 s = périmées côté client (`IsCurrent`) ; overlay effacé après 5 s.
- Requêtes LOS par paquets de 10 (buffer LuaSocket 8192 octets).
- Ports occupés par un autre outil / une 2e instance DCS = lien rompu.
- `os.execute` indisponible dans GameGUI, d'où le module .NET.
