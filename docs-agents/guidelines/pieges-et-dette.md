# Pièges connus, bugs repérés, dette technique

Liste établie par lecture du code le 2026-10-12. **Ne pas corriger spontanément** : signaler, et
corriger seulement si la tâche le demande ou si c'est sur le chemin. Vérifier que l'item existe
encore avant de s'y référer.

## Bugs probables
| Où | Quoi |
|----|------|
| `DCS-SR-Client/Singletons/InputDeviceManager.cs` | Boucle de polling bornée à `AwacsOverlayToggle` (137) : `TransponderPowerToggle` (138) jamais sondé. |
| `ServerCommandLine/Program.cs` | `--globalLobbyFrequencies` déclaré mais jamais appliqué ; `Options.ToString()` mélange deux libellés. |
| `Common/Helpers/JsonNetworkPropertiesResolver.cs` | Teste l'attribut sur le **type** du membre, pas sur le membre : `[JsonNetworkIgnoreSerialization]` inopérant. Utiliser `[JsonIgnore]`. |
| `Common/Models/UDPVoicePacket.cs` | Modulation manquante remplie avec 4 (HAVEQUICK) au lieu de AM. |
| `Common/Models/Player/RadioBase.cs` | `Equals` compare `Model` pas `Name`, `GetHashCode` l'inverse ; `DeepClone` oublie `retransmit` et `Name`. |
| `Common/Network/Singletons/SyncedServerSettings.cs` | `double.TryParse` dépendant de la culture pour `GLOBAL_LOBBY_FREQUENCIES` (serveur en invariant). |
| `DCS-SR-ExternalAudio/Client/Program.cs` | Écart nombre fréquences/modulations affiche « QUITTING! » mais continue ; Azure TTS NRE sans `--voice`. |
| `.github/workflows/release.yml` | Glob de tag `[0-9].[0-9].[0-9].[0-9]` : `2.4.10.0` ne déclenche pas. |
| `.github/workflows/formatter.yml` | Condition `pull_request.merged == true` sur opened/synchronize : ne s'exécute jamais. |
| `Common/Network/Server/ServerState.cs` | `NormalizePath` via `new Uri()` : chemin relatif d'export → `UriFormatException`. |
| `Common/Network/Server/UDPVoiceRouter.cs` | Échec de bind UDP non observé (dans `Task.Run`) ; TCP continue sans voix. Échec TCP → `Environment.Exit(0)`. |
| `DCS-SR-Client/DCS-SR-Client.csproj` | Entrées `RadioModels\arc131.json` etc. pointent vers des fichiers déplacés dans `WIP/` : seul `arc210.json` est copié. `Resources.ru-RU.resx` référencé mais absent. |
| `Common/Audio/Providers/RadioMixingProvider.cs` | Modèle RX toujours l'intercom par défaut (`modelName` null, TODO). |
| `DCS-SR-Client/Network/DCS/DCSAutoConnectHandler` + `MainWindowViewModel` | `AutoConnect` / `AutoConnectPrompt` exposés en UI mais non lus par le handler C# (à vérifier côté Lua). |
| `Server/UI/MainWindow/MainViewModel.cs` | Changement des fréquences globales publie `ServerFrequenciesChanged` sans `ServerSettingsChangedMessage` : les clients l'apprennent à la prochaine diffusion. |

## Incohérences documentaires dans le code
- Tolérance de fréquence : **500 Hz** réels (`FreqCloseEnough`) ; commentaires disent 100 Hz / 1 kHz.
- `Constants.JITTER_BUFFER=50` apparemment inutilisé (le jitter buffer Speex utilise 400 ms).
- `IRL_RADIO_STATIC` inutilisé. Fichiers `*_NOISE.wav` livrés mais non référencés.
- Typos figées dans l'API : `ToogleAwacsRadioOverlayMessage`, `PlayerCoaltionLocationMetadata`,
  `AwaRadioOverlayWindow`, `MulticastAllExeceptOne`, `ModsPuginsRecursiveSearch`.
- `JoinedNewAircraftMessage.cs` contient la classe `NewUnitEnteredMessage`.
- Certains messages d'événements sont dans le namespace `...Common.Network.Client` alors qu'ils
  vivent dans `Models/EventMessages/`.

## Fichiers parasites / morts (ne pas toucher sans demande)
`Server/Server_s3vspq0t_wpftmp.csproj`, `DCS-SimpleRadioStandalone.VC.db` (33 Mo, base
IntelliSense C++), `DCS-SR-ExternalAudio/README.md` (README NAudio.Vorbis),
`DCS-SR-ExternalAudio/FodyWeavers.xml`, `DCS-SR-Client/Audio/Managers/AudioGraphTest.cs`,
`DCS-SR-Client/Audio/BufferedSampleProvider.cs`, `UI/ClientWindow/InputBindingControl.xaml`,
`InputBindingListControl.xaml`, `Utils/AxisTuningHelper.cs`, `InputSettingsViewModel.cs`,
`Common/Audio/Providers/CachedLoopingNatoToneAudioProvider.cs`, `ALawProvider.cs`,
`TransmissionAssembler` / `ClientTransmissionBuffer`, `Models/Player/TransponderBase.cs`,
projet `DCS-SRS-Mobile`.

## Zones fragiles
- Ids d'arguments cockpit DCS (changent avec les mises à jour DCS).
- Copies manuelles champ par champ dans `ServerSync` / `TCPClientHandler` (oubli = champ jamais propagé).
- `MAX_RADIOS=11` dupliqué en littéral.
- Settings écrits sur disque à chaque `Set`.
- Les deux bus d'événements côté serveur.
- Check de mise à jour GitHub **bloquant** au démarrage du client et du serveur GUI.
- Chemins : effets via `BaseDirectory`, modèles via `CurrentDirectory`, favoris/whitelist via
  `CurrentDirectory`, settings via `-cfg`.
