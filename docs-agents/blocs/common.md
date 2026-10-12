# Bloc : `Common/` (SRS-Common)

Bibliothèque partagée **net10.0** (Win + Linux), sans WPF. C'est ici que vit l'essentiel : protocole,
modèles, settings, moteur audio et **tout le serveur**. Namespace racine
`Ciribob.DCS.SimpleRadio.Standalone.Common`. Nullable non activé.

Packages : Caliburn.Micro.Core (event aggregator uniquement), Concentus.Oggfile (Opus→Ogg),
NAudio.Core, NLog, Octokit, OggVorbisEncoder (référencé, non utilisé), sharpconfig, SharpOpenNat.
Natifs P/Invoke attendus : `opus`, `speexdsp`, `libmp3lame`.

## Carte des dossiers

| Dossier | Contenu |
|---------|---------|
| `Constants.cs` | Taux d'échantillonnage, tailles de trame, `MAX_RADIOS=11`, intervalle d'update |
| `Models/` | `NetworkMessage`, `UDPVoicePacket`, `ShortGuid`, `RadioReceivingState`, DTOs serveur, `ClientListExport`, `Player/` (modèles radio), `EventMessages/` (POCO du bus) |
| `Network/Client/` | `TCPClientHandler` (boucle TCP client), `UDPVoiceHandler` (socket voix, ping, file d'envoi) |
| `Network/Server/` | `ServerState` (orchestrateur), `ServerSync` (TCP), `SRSClientSession`, `UDPVoiceRouter`, `HttpServer`, `NatHandler`, `TransmissionLogging/` |
| `Network/Singletons/` | `EventBus`, `ConnectedClientsSingleton`, `SyncedServerSettings` |
| `Settings/` | `ServerSettingsStore`, `GlobalSettingsStore` (+ enums `GlobalSettingsKeys`, `InputBinding`), `ProfileSettingsStore` (+ `ProfileSettingsKeys`), `ServerChannelPresetHelper`, `Setting/ServerSettingsKeys.cs`, `Input/` |
| `Audio/Opus/` | P/Invoke libopus, `OpusEncoder`/`OpusDecoder` |
| `Audio/Providers/` | Graphe NAudio `ISampleProvider` : `ClientAudioProvider`, `ClientTransmissionPipelineProvider`, `ClientEffectsPipeline`, `RadioMixingProvider`, `SRSMixingSampleProvider`, `CachedAudioEffectProvider`, filtres, bruit, CVSD… |
| `Audio/Models/` | `RadioModel` (DTO JSON + factory), `CachedAudioEffect`, `ClientAudio`, `JitterBufferAudio`, `DeJitteredTransmission`, `TransmissionSegment` |
| `Audio/Dsp/` | BiQuad, filtres 1er ordre, enveloppe, compresseur sidechain |
| `Audio/Utility/` | `CircularFloatBuffer`, `ConversionHelpers`, `VolumeConversionHelper`, `Speex/` (Preprocessor, JitterBuffer), `WineDetector` |
| `Audio/Recording/` | `AudioRecordingManager`, writers MP3 (LAME) / Opus |
| `Audio/LameWrapper/`, `NAudioLame/` | Bindings LAME (code tiers, tabulations) |
| `Audio/Codecs/CVSD.cs` | Dégradation « chiffrement » |
| `Helpers/` | `UpdaterChecker` (`VERSION`, `MINIMUM_PROTOCOL_VERSION`), `GitHubUpdater`, `LoggingHelper`, `RadioCalculator` (Friis, haversine, Link16), `JsonNetworkPropertiesResolver`, `PropertyChangedBaseClass`, `PooledArray`, `ListHelper` |
| `NetCoreServer/` | Copie vendorisée de NetCoreServer (TCP) : `TcpServer`, `TcpSession`, `Buffer` ; `ServerSync : TcpServer`, `SRSClientSession : TcpSession` |
| `Properties/` | `Resources.resx` + `Resources.zh.resx` |

## Points d'architecture

- **Pas de conteneur DI.** Singletons statiques `X.Instance` (double-checked lock) ou construction
  explicite. Le serveur reçoit un `IEventAggregator` par constructeur.
- **Deux bus d'événements** : `EventBus.Instance` (statique, client et `SRSClientSession`) et
  l'`IEventAggregator` injecté (serveur). Publier sur le mauvais = silence.
- Messages : classes POCO dans `Models/EventMessages/` (`TCPClientStatusMessage`,
  `VOIPStatusMessage`, `SRClientUpdateMessage`, `UnitUpdateMessage`, `ServerSettingsUpdatedMessage`,
  `EAMConnect*`, `StartServerMessage`, `KickClientMessage`…). Handlers `IHandle<T>.HandleAsync`.
- Threads : `new Thread` pour les boucles longues (UDP, logs, enregistrement), `Task.Run` pour le
  fire-and-forget, `CancellationTokenSource` pour l'arrêt, `ConcurrentDictionary` pour l'état partagé.
  Beaucoup de `catch (Exception) {}` silencieux.
- Audio chaud : `ArrayPool<float>`, `Span<float>`, SIMD `Vector<float>`, settings rafraîchis toutes
  les 3 s plutôt que lus à chaque trame.
- C# moderne : `GeneratedRegex`, `required`, `init`, collection expressions, `Lock`, mot-clé `field`
  (C# 14) → SDK .NET 10 obligatoire.

## Normalisation des noms de radio
Deux regex identiques `[^a-zA-Z0-9]` : `RadioBase.NormaliseRadioRegex` (Model/Name, minuscules,
32 car.) et `ServerChannelPresetHelper.NormaliseRegex` (noms de fichiers presets). Le client
associe un preset si le nom normalisé de la radio `StartsWith` la clé du fichier
(`an-arc-210.txt` → `anarc210`).

## Invariants à respecter
1. Fréquences en **Hz** (double) en interne et sur le fil ; MHz dans les fichiers/UI.
2. 11 radios max, dupliqué en littéral dans `AudioRecordingManager` et `SyncedServerSettings`.
3. Ajouter un champ qui doit se propager : l'ajouter à `Equals`/`MetaDataEquals`/`DeepClone` de
   `SRClientBase`, aux copies manuelles dans `ServerSync.HandleClientMetaDataUpdate` /
   `SendFullRadioUpdate` et `TCPClientHandler.HandlePartialUpdate`. Sinon il n'est jamais envoyé.
4. Nouvelle clé setting = enum + défaut, sinon chaîne vide persistée et `GetXxxBool` renvoie false.
5. WAV d'effets : 48 kHz / 16 bit / mono, sinon `Loaded=false` silencieux.
6. `UpdaterChecker.VERSION` vient de l'assembly **d'entrée** (l'exe), pas de SRS-Common.
