# Vue d'ensemble de l'architecture

## Les composants

```
                 ┌──────────────────────────── Machine du pilote ────────────────────────────┐
                 │                                                                            │
                 │  DCS World                              SR-ClientRadio.exe (WPF)           │
                 │  ┌─────────────────────┐   UDP 9084     ┌──────────────────────────┐       │
                 │  │ Export.lua ─► SRS   │ ─────────────► │ DCSRadioSyncHandler      │       │
                 │  │  (cockpit, radios)  │ ◄───────────── │ DCSLineOfSightHandler    │       │
                 │  │                     │  UDP 9086/9085 │ UDPCommandHandler (9040) │       │
                 │  │ GameGUI hooks       │ ─► 5068/5069   │ AudioManager / UDPClient │       │
                 │  │  (seat, autoconnect)│ ◄─ 7080 overlay│ InputDeviceManager       │       │
                 │  └─────────────────────┘                └─────────┬────────────────┘       │
                 │                                       TCP 5002 (JSON) │ UDP 5002 (voix Opus) │
                 └────────────────────────────────────────────────────┼─────────────────────────┘
                                                                      ▼
                                       SRS-Server.exe (WPF) / SRS-Server-Commandline (Win/Linux)
                                       ┌────────────────────────────────────────────┐
                                       │ ServerState ─ ServerSync (TCP) ─ UDPVoiceRouter │
                                       │ HttpServer (admin), exports (JSON, LotATC)  │
                                       └────────────────────────────────────────────┘
                                                                      ▲
                 DCS-SR-ExternalAudio (CLI TTS/MP3) ──────────────────┘   DCS-SRS-Mobile (Android, prototype)
```

| Projet | Exe / sortie | Rôle | Plateforme |
|--------|--------------|------|------------|
| `Common` | `SRS-Common.dll` | **Cœur partagé** : protocole TCP/UDP, modèles, settings, moteur audio, **serveur complet** (`ServerState`, `ServerSync`, `UDPVoiceRouter`) | net10.0, Win+Linux |
| `DCS-SR-Client` | `SR-ClientRadio.exe` | Client WPF : UI, overlays, entrées, intégration DCS, capture/lecture audio | Windows |
| `Server` | `SRS-Server.exe` | Hôte WPF du serveur (Caliburn.Micro) | Windows |
| `ServerCommandLine` | `SRS-Server-Commandline` | Hôte console du serveur (CommandLineParser) | Win + Linux |
| `DCS-SR-ExternalAudio` | `DCS-SR-ExternalAudio.exe` | Injecte TTS/MP3 sur une fréquence (faux client) | Windows |
| `DCS-SR-Lua` | `lua-srs.dll` | Module Lua natif (NativeAOT) pour lancer le client depuis DCS | Windows |
| `Scripts/` | Lua | Export cockpit, GameGUI, overlay in-game, modules avions | DCS |
| `Installer` | `Installer.exe` | Installe programme + scripts dans Saved Games, registre | Windows |
| `AutoUpdater` | `SRS-AutoUpdater.exe` | Télécharge la release GitHub, lance l'installeur | Windows |
| `SharedAudio` | natifs | Porte `opus.dll/.so`, `libmp3lame.dll/.so` vers la sortie (aucun .cs) | — |
| `DCS-SR-CommonTests` | MSTest | 7 tests (packet UDP, conversions) | Windows (TFM) |
| `DCS-SRS-Mobile` | MAUI Android | Prototype abandonné, hors .sln | — |

Règle d'or : **la logique est dans `Common`**. Le client et les deux serveurs sont des hôtes.
Graphe de dépendances : `Common` ← Client, Server, ServerCommandLine, ExternalAudio, AutoUpdater,
DCS-SR-Lua, Tests. `Installer` ne dépend de rien.

## Flux principaux

### 1. État radio : DCS → client → serveur → autres clients
1. `DCS-SimpleRadioStandalone.lua` lit le cockpit toutes les 0,2 s, envoie un JSON sur UDP 9084.
2. `DCSRadioSyncHandler` fusionne dans `ClientStateSingleton.DcsPlayerRadioInfo` en respectant les
   modes (cockpit vs overlay) et les options serveur.
3. Si changement (ou 60 s), `UnitUpdateMessage` → `TCPClientHandler` envoie `RADIO_UPDATE` en TCP.
4. `ServerSync` met à jour `_connectedClients` et rediffuse à tous.
5. Chaque client garde la liste dans `ConnectedClientsSingleton` (sert au routage RX et à l'UI).
6. Le client renvoie son état combiné sur UDP 7080 pour l'overlay in-game.

### 2. Voix : micro → serveur → haut-parleurs
1. WASAPI capture → resample 16 kHz → VOX → Speex denoise → Opus 40 ms (`AudioManager`).
2. `UDPClientAudioProcessor.Send` : si PTT, construit `UDPVoicePacket` (fréquences des radios qui
   émettent, modulation, clé enc, unitId, GUID) → `UDPVoiceHandler` → UDP 5002.
3. `UDPVoiceRouter` (serveur) : pour chaque autre client, décide d'après coalition, fréquence
   (±500 Hz), modulation, chiffrement, intercom (même `unitId`), fréquences globales/test, hops.
   Le paquet est relayé **tel quel** (jamais décodé).
4. Client récepteur : `UdpAudioDecode` → `CanHearTransmission` (quelle radio, déchiffrable ?) →
   LOS/distance (si options serveur) → `ClientAudioProvider` (Opus 48 kHz, jitter buffer par radio,
   atténuations, ambiance cockpit, chaîne d'effets TX du modèle de radio) → `RadioMixingProvider`
   (une par radio locale : mixage, tonalités début/fin, filtres RX, balance G/D) →
   `SRSMixingSampleProvider` → `SRSWasapiOut`.

### 3. Connexion
`MainWindowViewModel.ConnectAsync` → `TCPClientHandler.TryConnect` → `SYNC` → réponse `SYNC`
(liste clients + settings serveur + version) → `TCPClientStatusMessage(true)` → `StartAudio()` +
`DCSRadioSyncManager.Start()` (ouvre tous les listeners UDP DCS). Ping UDP 22 octets toutes les
15 s ; première réponse = VoIP prête.

### 4. Auto-connect
Hook serveur DCS → message chat `SRS Running on <port>` → `DCS-SRSGameGUI.lua` → UDP 5069 →
`AutoConnectMessage` → connexion (ou prompt si déjà connecté ailleurs).

## Où vit quoi (carte rapide)

| Je veux toucher… | Aller voir |
|------------------|-----------|
| Format des paquets / messages | `Common/Models/UDPVoicePacket.cs`, `NetworkMessage.cs` |
| Règle « qui entend qui » | `Common/Models/Player/PlayerRadioInfoBase.CanHearTransmission` (partagé client/serveur), `Common/Network/Server/UDPVoiceRouter.cs`, `DCS-SR-Client/Audio/Managers/UDPClientAudioProcessor.cs` |
| Options serveur | `Common/Settings/Setting/ServerSettingsKeys.cs`, `ServerSettingsStore.cs`, GUI `Server/UI/MainWindow/MainViewModel.cs`, CLI `ServerCommandLine/Program.cs` |
| Options client | `Common/Settings/GlobalSettingsStore.cs`, `ProfileSettingsStore.cs`, UI `DCS-SR-Client/UI/ClientWindow/ClientSettingsControl/` |
| Touches / joystick | `DCS-SR-Client/Singletons/InputDeviceManager.cs`, enum `InputBinding` dans `GlobalSettingsStore.cs` |
| Effets audio | `Common/Audio/Providers/`, modèles JSON `DCS-SR-Client/RadioModels/`, WAV `DCS-SR-Client/AudioEffects/` |
| Lecture cockpit d'un avion | `Scripts/DCS-SRS/Scripts/DCS-SRS-Modules/<Avion>.lua` |
| Overlay WPF | `DCS-SR-Client/UI/ClientWindow/RadioOverlayWindow/`, `AwacsRadioOverlayWindow/` |
| Overlay in-game | `Scripts/DCS-SRS/Scripts/DCS-SRS-OverlayGameGUI.lua` |
| Installation / mise à jour | `Installer/MainWindow.xaml.cs`, `AutoUpdater/MainWindow.xaml.cs`, `Common/Helpers/UpdaterChecker.cs` |
| Build / release | `publish.ps1`, `.github/workflows/` |
