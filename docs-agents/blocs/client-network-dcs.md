# Bloc : client — connexion serveur et intégration DCS

## Connexion TCP (`Common/Network/Client/TCPClientHandler.cs`)
- Créé par `MainWindowViewModel.ConnectAsync()` (parsing `ip[:port]` inline, défaut 5002, DNS IPv4).
- `TryConnect(IPEndPoint)` → thread → `Connect()` (10 s, keepalive, idle check 30 s /
  `IdleTimeOut`) → `ClientSyncLoopAsync()`.
- Sortant via le bus : `UnitUpdateMessage{FullUpdate}` → `RADIO_UPDATE` (si `Equals` change ou
  180 s) / `UPDATE` ; `EAMConnectRequestMessage{Name, Password}` →
  `EXTERNAL_AWACS_MODE_PASSWORD` ; `DisconnectRequestMessage` ; `GatewayUnit*`.
- Entrant : `SYNC` (version, clients, settings), `UPDATE`/`RADIO_UPDATE` (→
  `SRClientUpdateMessage`), `SERVER_SETTINGS` (EAM désactivé → `EAMDisconnectMessage`),
  `CLIENT_DISCONNECT`, `VERSION_MISMATCH`, `EXTERNAL_AWACS_MODE_PASSWORD` (coalition 0 →
  `EAMDisconnectMessage`, sinon `EAMConnectedMessage`).
- Pas de reconnexion auto. Déconnexion → `TCPClientStatusMessage(false)` →
  `MainWindowViewModel.Stop()` (audio + `DCSRadioSyncManager`).
- Connexion OK → `Sounds.BeepConnected`, `StartAudio()`, `new DCSRadioSyncManager(guid).Start()`.

## EAM (External AWACS Mode)
`EAMConnectCommand` → requête ; `DCSRadioSyncHandler.HandleAsync(EAMConnectedMessage)` lance une
boucle 200 ms qui fabrique un `DCSPlayerRadioInfo` synthétique (`unit = "EAM"`,
`unitId = 100000000 + IntercomOffset`, radios depuis le preset serveur si
`AllowServerEAMRadioPreset`, sinon `awacs-radios-custom.json`, sinon `awacs-radios.json`). Les
paquets export DCS sont ignorés pendant EAM.

## Listeners UDP DCS (`DCS-SR-Client/Network/DCS/`, démarrés par `DCSRadioSyncManager`)
Bind `IPAddress.Any`, ports de `Network Settings`, retry 500 ms (un conflit de port ne fait que
logger un warning).

| Port | Classe | Rôle |
|------|--------|------|
| 9084 | `DCSRadioSyncHandler` | JSON `DCSPlayerRadioInfo` depuis l'export Lua. `UpdateRadioAsync()` fusionne dans `ClientStateSingleton.DcsPlayerRadioInfo` selon `FreqMode`/`VolumeMode`/`EncryptionMode`/`RetransmitMode`, `RADIO_EXPANSION`, `ALLOW_RADIO_ENCRYPTION`, `AlwaysAllowHotasControls`, `AllowDCSPTT`, `AlwaysAllowTransponderOverlay`. Détecte nouvel avion/siège → recharge `FixedChannels`, `AutoSelectPresetChannel`, `NewUnitEnteredMessage`. IDENT expire après 40 s. Si changé, `LastSent == 0` ou 60 s → `UnitUpdateMessage{FullUpdate=true}`. Puis `SendRadioUpdateToDCSAsync()` → `CombinedRadioState` vers 7080 (overlay in-game) et 7082 (panneaux externes) |
| 5068 | `DCSGameGuiHandler` | `{name, side, seat}` depuis GameGUI → coalition/siège/nom, `UnitUpdateMessage{FullUpdate=false}` |
| 5069 | `DCSAutoConnectHandler` | texte `host[:port]` → `AutoConnectMessage` → connexion ou prompt `AutoConnectMismatchPrompt` si déjà connecté ailleurs. (`AutoConnect`/`AutoConnectPrompt` ne sont pas lus ici : à vérifier) |
| 9040 | `Network/UDPCommandHandler` | `UDPInterfaceCommand{Command, RadioId, Frequency, Volume, Enabled, Code}` → `RadioHelper`/`TransponderHelper`. Interface pour GameGUI et outils externes |
| 9086 → / 9085 ← | `DCSLineOfSightHandler` | envoie `[{id,lat,lng,alt}]` par 10 toutes les 250 ms si `LOS_ENABLED`, reçoit `[{id,los}]` → `SRClientBase.LineOfSightLoss` |
| 10710 / 10711 | `Network/LotATC/LotATCSyncHandler` | position contrôleur (`ShouldUseLotATCPosition`, `LotATCHeightOffset`), LOS LotATC |
| 33501 | `Network/VAICOM/VAICOMSyncHandler` | inhibition TX |

`DCSRadioSyncManager` : timer 1 s qui remet radios/position à zéro quand les données sont périmées
(> 10 s, `IsCurrent()`).

## Multi-équipage
`seat` vient de l'export et du GameGUI ; `unitId` identique pour tout l'équipage → intercom
(`CanHearTransmission` branche INTERCOM). Profil auto : suffixe `_<seat+1>`.

## Messages du bus utilisés côté client
`TCPClientStatusMessage`, `VOIPStatusMessage`, `SRClientUpdateMessage`, `UnitUpdateMessage`,
`ServerSettingsUpdatedMessage`, `EAMConnectRequestMessage` / `EAMConnectedMessage` /
`EAMDisconnectMessage`, `DisconnectRequestMessage`, `InvalidServerVersionMessage`,
`ProfileChangedMessage`, `AutoConnectMessage` (`UI/ClientWindow/`), `NewUnitEnteredMessage`
(fichier `JoinedNewAircraftMessage.cs`), `InstructorModeMessage`, `CloseRadioOverlayMessage`,
`ToggleSingleStackRadioOverlayMessage`, `ToogleAwacsRadioOverlayMessage`,
`ServerSettingsPresetsSettingChangedMessage`.
