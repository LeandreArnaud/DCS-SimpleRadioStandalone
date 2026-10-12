# Modèles de données, enums et clés de settings

## 1. Modèles réseau (`Common/Models/Player/`)

### `SRClientBase` (un client connecté)
`ClientGuid` (ShortGuid 22 car.), `Name` (vide → `"---"`), `Coalition` (0 spectateur, 1 rouge,
2 bleu), `AllowRecord`, `Seat`, `RadioInfo` (`PlayerRadioInfoBase`), `LatLngPosition`, `Gateway`,
`GatewayClient`, `DISEntityId` (-1). Non sérialisés (`[JsonIgnore]`) : `Muted`, `VoipPort`,
`LineOfSightLoss` (0 = rien, ≥1 = bloqué), `TransmittingFrequency`, `LastTransmissionReceived`,
`ClientSession`. `MetaDataEquals(other, usePosition)` ignore `RadioInfo`. `DeepClone()`.

### `PlayerRadioInfoBase`
Champs publics : `ambient` (`Ambient`), `iff` (`Transponder`), `radios` (`RadioBase[11]`, index 0 =
intercom), `unit` (type d'avion), `unitId` (uint). `Equals` compare tout. `LastUpdate` ignoré.
Le client a une version enrichie `DCSPlayerRadioInfo` (`DCS-SR-Client/Network/DCS/Models/DCSState/`)
avec `DCSRadio` (modes, bornes, canal, `selected`, `ptt`, `control`, `capabilities`...).

### `RadioBase`
`freq` (double **Hz**, défaut 1 = éteinte), `secFreq` (guard Hz, 1/0 = inactif), `modulation`
(défaut `DISABLED`), `enc`, `encKey` (byte), `retransmit`, `IntercomUnitId`, `Model`, `Name`.
Setters `Model`/`Name` normalisent : minuscules, `[^a-zA-Z0-9]` supprimé, 32 caractères max.
`Equals` : `FreqCloseEnough` (500 Hz) + compare `Model` mais pas `Name`. `DeepClone()` ne copie
**pas** `retransmit` ni `Name`.

### Enums

| Enum | Valeurs | Où |
|------|---------|----|
| `Modulation` | `AM=0, FM=1, INTERCOM=2, DISABLED=3, HAVEQUICK=4, SATCOM=5, MIDS=6, SINCGARS=7` | `Common/Models/Player/Modulation.cs` |
| `IFFControlMode` | `COCKPIT=0, OVERLAY=1, DISABLED=2` | `Transponder.cs` |
| `IFFStatus` | `OFF=0, NORMAL=1, IDENT=2` | `Transponder.cs` |
| `FreqMode` / `VolumeMode` | `COCKPIT=0, OVERLAY=1` | `DCS-SR-Client/.../DCSRadio.cs` (client seulement, int dans Common) |
| `RetransmitMode` | `COCKPIT=0, OVERLAY=1, DISABLED=2` | idem |
| `EncryptionMode` | `NO_ENCRYPTION=0, ENCRYPTION_JUST_OVERLAY=1, ENCRYPTION_FULL=2, ENCRYPTION_COCKPIT_TOGGLE_OVERLAY_CODE=3` | idem |
| `MessageType` | voir `protocole-reseau.md` | `NetworkMessage.cs` |
| `CachedAudioEffect.AudioEffectTypes` | `RADIO_TRANS_START=0, RADIO_TRANS_END=1, KY_58_TX=2, KY_58_RX=3, NATO_TONE=4, MIDS_TX=5, MIDS_TX_END=6, HAVEQUICK_TONE=7, INTERCOM_TRANS_START=12, INTERCOM_TRANS_END=13, AMBIENT_COCKPIT=15` | `Common/Audio/Models/` |
| `UDPCommandType` | 0 FREQUENCY_DELTA, 1 ACTIVE_RADIO, 2 TOGGLE_GUARD, 3/4 CHANNEL_UP/DOWN, 5 SET_VOLUME, 6-10 transpondeur, 11 GUARD, 12 FREQUENCY_SET, 13 M2, 14-19 retransmit/simul, 20 INTERCOM_CHANNEL | `DCS-SR-Client/Network/UDPCommandHandler.cs` |
| `ServerPresetConfiguration` | `USE_SERVER_ONLY_IF_SET, USE_CLIENT_ONLY, USE_CLIENT_AND_SERVER_IF_SET` | `ProfileSettingsStore.cs` |

Autres : `Transponder` (`control`, `status`, `mode1/2/3` int -1 = off, `mode4` bool, `mic`,
`expansion`), `Ambient` (`abType` string → `AudioEffects/Ambient/<abType>.wav`, `vol`),
`LatLngPosition` (5 décimales, `IsValid()` = lat et lng ≠ 0), `DCSRadioCustom` (radios EAM, miroir
manuel de `DCSRadio`, 11 entrées obligatoires), `ServerPresetChannel` (`Name`, `Frequency` en
**MHz**, ou numéro de canal 1..125 pour les fichiers contenant `mids`), `ClientListExport`.

## 2. Constantes (`Common/Constants.cs`)

`MIC_SAMPLE_RATE=16000`, `MIC_INPUT_AUDIO_LENGTH_MS=40`, `MIC_SEGMENT_FRAMES=640`,
`OUTPUT_SAMPLE_RATE=48000`, `OUTPUT_SEGMENT_FRAMES=1920`, `MAX_RADIOS=11`,
`CLIENT_UPDATE_INTERVAL_LIMIT=180` s. Ailleurs : `FreqCloseEnough` 500 Hz, ping UDP 15 s, timeout
UDP 42 s, `IsReceiving` 350 ms, `JITTER_MS=400`, données DCS périmées après 10 s,
`MINIMUM_PROTOCOL_VERSION="1.9.0.0"`.

## 3. Settings (SharpConfig, fichiers `.cfg` INI)

Tous les stores : singleton, création des défauts si fichier absent, `.bak` + reset si corrompu,
**sauvegarde synchrone à chaque `Set`** (éviter dans les boucles chaudes), clé manquante ajoutée
avec son défaut au premier `Get`. Les noms d'enum sont les clés INI : **renommer un membre casse
les configs existantes**.

### `ServerSettingsStore` → `server.cfg` (`-cfg=` pour changer)
Sections : `General Settings` (**synchronisé aux clients**), `Server Settings` (local),
`External AWACS Mode Settings` (mots de passe). Clés et défauts (`DefaultServerSettings.Defaults`) :

| Clé | Section | Défaut | Appliqué par |
|-----|---------|--------|--------------|
| `SERVER_PORT` | S | 5002 | serveur (TCP+UDP) |
| `SERVER_IP` | S | 0.0.0.0 | serveur (bind UDP seulement) |
| `COALITION_AUDIO_SECURITY` | G | false | serveur + client |
| `SPECTATORS_AUDIO_DISABLED` | G | false | serveur |
| `CLIENT_EXPORT_ENABLED` / `CLIENT_EXPORT_FILE_PATH` | G / S | false / clients-list.json | serveur |
| `LOS_ENABLED`, `DISTANCE_ENABLED`, `IRL_RADIO_TX`, `IRL_RADIO_RX_INTERFERENCE`, `RADIO_EXPANSION`, `RADIO_EFFECT_OVERRIDE` | G | false | **client seulement** |
| `IRL_RADIO_STATIC` | G | false | inutilisé |
| `EXTERNAL_AWACS_MODE` | G | false | serveur + client |
| `EXTERNAL_AWACS_MODE_BLUE_PASSWORD` / `_RED_PASSWORD` | E | "" | serveur, jamais diffusé |
| `CHECK_FOR_BETA_UPDATES` | S | false | hôtes |
| `ALLOW_RADIO_ENCRYPTION` | G | true | client |
| `STRICT_RADIO_ENCRYPTION` | G | false | serveur + client |
| `TEST_FREQUENCIES` | G | "247.2,120.3" (MHz) | serveur (écho) |
| `GLOBAL_LOBBY_FREQUENCIES` | G | "248.22" (MHz) | serveur + client |
| `SHOW_TUNED_COUNT` / `SHOW_TRANSMITTER_NAME` | G | true / false | client |
| `LOTATC_EXPORT_ENABLED` / `_PORT` / `_IP` | G | false / 10712 / 127.0.0.1 | serveur |
| `UPNP_ENABLED` | S | true | serveur, pas de GUI |
| `RETRANSMISSION_NODE_LIMIT` | G | 0 | serveur + client |
| `TRANSMISSION_LOG_ENABLED` / `_RETENTION` | G | false / 2 | serveur |
| `SERVER_PRESETS_ENABLED` / `SERVER_PRESETS` | G | false / généré | serveur → client |
| `SERVER_EAM_RADIO_PRESET_ENABLED` / `SERVER_EAM_RADIO_PRESET` | G | false / généré | serveur → client |
| `ALLOW_INSTRUCTOR_MODE` | G | false | client |
| `HTTP_SERVER_ENABLED` / `_PORT` / `_ADDRESS` / `_API_KEY` | S | false / 8080 / localhost / ShortGuid aléatoire | serveur |

Client : `SyncedServerSettings.Instance.GetSetting(key)` / `GetSettingAsBool(key)` (caché),
`GlobalFrequencies` (Hz), `RetransmitNodeLimit`, presets.

### `GlobalSettingsStore` → `global.cfg`
Sections `Position Settings` (fenêtres), `Client Settings`, `Network Settings` (ports). Enum
`GlobalSettingsKeys`, défauts dans `defaultGlobalSettings`. Principales clés :
`AutoConnect=true`, `AutoConnectPrompt=false`, `AutoConnectMismatchPrompt=true`,
`RadioOverlayTaskbarHide`, `RefocusDCS`, `ExpandControls` (Allow More Inputs), `MinimiseToTray`,
`StartMinimised`, `AudioInputDeviceId/AudioOutputDeviceId/MicAudioOutputDeviceId`, `LastServer`,
`MicBoost=0.514`, `SpeakerBoost=0.514`, `RadioX/Y/Size/Opacity/Width/Height`, `ClientX/Y`,
`AwacsX/Y`, `ClientIdLong`, `Denoise=true`, `DenoiseAttenuation=-30`, `IncomingAudioAGC*`,
`IncomingAudioDenoise*`, `LastSeenName`, `CheckForBetaUpdates`, `AllowMultipleInstances`,
`PlayConnectionSounds=true`, `RequireAdmin`, `AutoSelectSettingsProfile`, `LotATCHeightOffset=50`,
`VAICOMTXInhibitEnabled`, `ShowTransmitterName=true`, `IdleTimeOut=600`, `AllowRecording`,
`RecordAudio`, `SingleFileMixdown`, `RecordingQuality=V3`, `RecordingFormat=mp3`,
`DisallowedAudioTone`, `VOX`, `VOXMode=3`, `VOXMinimumTime=300`, `VOXMinimumDB=-59`,
`AllowXInputController`, `LastPresetsFolder`, `DISEntityID=-1`, `DarkMode=false`,
`SettingsProfiles=["default.cfg"]`.
Ports (`Network Settings`) : `DCSIncomingUDP=9084`, `DCSLOSIncomingUDP=9085`,
`DCSLOSOutgoingUDP=9086`, `CommandListenerUDP=9040`, `OutgoingDCSUDPInfo=7080`,
`OutgoingDCSUDPOther=7082`, `DCSIncomingGameGUIUDP=5068`, `DCSAutoConnectUDP=5069`,
`LotATCIncomingUDP=10710`, `LotATCOutgoingUDP=10711`, `VAICOMIncomingUDP=33501`.

### `ProfileSettingsStore` → `<profil>.cfg`
Section `Client Settings` (enum `ProfileSettingsKeys`, défauts `DefaultSettingsProfileSettings`)
+ une section par `InputBinding` (`name`, `button`, `value`, `guid`). Clés principales :
`RadioEffectsRatio=1.0`, `RadioEffectsClipping`, `RadioEncryptionEffects=true`, `NATOTone=true`,
`HAVEQUICKTone=true`, `RadioRxEffects_Start/End=true`, `RadioTxEffects_Start/End=true`,
`RadioTransmissionStartSelection=RADIO_TRANS_START.wav`, `RadioTransmissionEndSelection`,
`IntercomTransmissionStart/EndSelection`, `MIDSRadioEffect=true`, `AutoSelectPresetChannel=true`,
`AlwaysAllowHotasControls`, `AllowDCSPTT=true`, `RadioSwitchIsPTT`, `RadioSwitchIsPTTOnlyWhenValid`,
`AlwaysAllowTransponderOverlay`, `PTTReleaseDelay=0`, `PTTStartDelay=0`,
`RadioBackgroundNoiseEffect=true`, `NATOToneVolume=1.2`, `HQToneVolume=0.3`, `NoiseGainDB`,
`HFNoiseGainDB`, `PerRadioModelEffects=true`, `AMCollisionVolume=1.0`, `RotaryStyleIncrement`,
`AmbientCockpitNoiseEffect=true`, `AmbientCockpitNoiseEffectVolume=1.0`,
`AmbientCockpitIntercomNoiseEffect`, `DisableExpansionRadios`,
`ServerPresetSelection=USE_CLIENT_AND_SERVER_IF_SET`, `AllowServerEAMRadioPreset=true`,
`InstructorMode`, `Radio1Channel..Radio10Channel=0`, `IntercomChannel=0` (balance -1..1).

### `InputBinding` (dans `GlobalSettingsStore.cs`)
Principaux 100..138 : `Intercom=100`, `Switch1..10=101..110`, `Ptt=111`, `OverlayToggle=112`,
`Up100..Up0001=113..118`, `Down100..Down0001=119..124`, `NextRadio=125`, `PreviousRadio=126`,
`ToggleGuard=127`, `ToggleEncryption=128`, `EncryptionKeyIncrease/Decrease=129/130`,
`RadioChannelUp/Down=131/132`, `TransponderIDENT=133`, `RadioVolumeUp/Down=134/135`,
`IntercomPTT=136`, `AwacsOverlayToggle=137`, `TransponderPowerToggle=138`. Modificateur = +100.

## 4. Fichiers de données à côté des exécutables

| Fichier | Qui | Rôle |
|---------|-----|------|
| `global.cfg`, `<profil>.cfg` | client | settings (dossier exe ou `-cfg=`) |
| `FavouriteServers.csv` | client | favoris `Name,Address,IsDefault,EAMPassword` (CWD) |
| `whitelist.txt`, `blacklist.txt` | client | GUID de périphériques DirectInput (CWD) |
| `awacs-radios.json`, `awacs-radios-custom.json` | client | radios EAM (dossier presets) |
| `<radio>.txt` | client / serveur `Presets/` | channelisation |
| `RadioModels/*.json`, `RadioModelsCustom/*.json` | client | modèles DSP de radios |
| `AudioEffects/*.wav`, `AudioEffects/Ambient/*.wav` | client | sons (48 kHz 16 bit mono obligatoire) |
| `Recordings/` | client | enregistrements MP3/Opus |
| `server.cfg`, `banned.txt`, `clients-list.json`, `*-transmissionlog.csv` | serveur | config, bans, exports |
| `clientlog.txt`, `serverlog.txt`, `installer-log.txt` | tous | logs NLog |
