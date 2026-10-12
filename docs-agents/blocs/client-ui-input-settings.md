# Bloc : client WPF — démarrage, UI, entrées, settings

Projet `DCS-SR-Client` (`SR-ClientRadio.exe`, `net10.0-windows`, WPF + WinForms pour `NotifyIcon` /
`TaskDialog` / `FolderBrowserDialog`). Namespace `Ciribob.DCS.SimpleRadio.Standalone.Client`.
Packages : MahApps.Metro 2.4 (ControlzEx, thème), Extended.Wpf.Toolkit, PropertyChanged.Fody,
Sentry, SharpDX (DirectInput, XInput), NAudio.Wasapi, WebRtcVadSharp.

## Démarrage (`App.xaml.cs`)
1. `-lang:<culture>` → `CurrentUICulture` (resx `Resources.resx`, `Resources.zh.resx`).
2. Sentry + `UnhandledException` → NLog.
3. Vérifie `opus.dll` et `speexdsp.dll` à côté de l'exe, sinon exit.
4. NLog : `NLog.config` copié en sortie, sinon config programmatique équivalente (`clientlog.txt`,
   100 Mo). **Garder les deux en phase.**
5. Hors DEBUG : instance unique via mutex `DCS-SimpleRadioStandalone` sauf `-allowMultiple` ou
   `AllowMultipleInstances`.
6. `RequireAdmin` → relance en `runas` avec les mêmes args + `-allowMultiple`.
7. Icône de notification WinForms. `OnStartup` : `ThemeHelper.ApplyTheme(DarkMode)`
   (`Dark.Blue` / `Light.Blue`). `OnExit` : `ClientStateSingleton.Close()`.

Args : `-lang:xx`, `-cfg=<dir>` (dossier des .cfg), `-host=<ip[:port]>` (auto-connexion après 2 s),
`-allowMultiple`. `StartupUri` = `UI/ClientWindow/MainWindow.xaml`.

`MainWindow` ctor : GC `SustainedLowLatency`, `Images.Init()`, `Sounds.Init()`, position restaurée,
`DataContext = new MainWindowViewModel { FavouriteServersViewModel = ... }`, **vérification de mise à
jour GitHub synchrone** (bloque l'UI au démarrage), `CheckWindowVisibility()`.

## MVVM « maison »
- `Common/Helpers/PropertyChangedBaseClass` + **PropertyChanged.Fody** : toute auto-propriété publique
  d'une classe `INotifyPropertyChanged` notifie automatiquement. `NotifyPropertyChanged(nameof(X))`
  pour les propriétés calculées.
- Commandes : `Utils/DelegateCommand`. Messages : `EventBus.Instance` (Caliburn `EventAggregator`),
  handlers `IHandle<T>.HandleAsync`, abonnement dans le ctor, `Unsubscribe(this)` pour les objets
  transitoires.
- Pas de bootstrapper Caliburn, pas de conventions de nommage vue/VM, pas d'IoC. Beaucoup de logique
  en code-behind (overlays, transpondeur, liste clients, fenêtre settings serveur).
- Rafraîchissement par `DispatcherTimer` : 100 ms (VU, labels), 80 ms (overlays), 1 s (états),
  3 s (liste clients).

## Fenêtres et vues (`DCS-SR-Client/UI/ClientWindow/`)

| Vue | VM / code-behind | Notes |
|-----|------------------|-------|
| `MainWindow.xaml` (`MetroWindow`) | `MainWindowViewModel.cs` | Onglets General, Controls (`InputSettingsControl/InputSettings`), Favourites (`Favourites/FavouriteServersView`), Settings (`ClientSettingsControl/ClientSettings` + `ClientSettingsViewModel`), Help. Expose les singletons en propriétés pour le binding. `ConnectAsync`, `Stop`, toggles d'overlays, EAM |
| `RadioOverlayWindow/RadioOverlay.xaml` (`RadioOverlayWindow`) | code-behind | 5 `RadioControlGroup` + `IntercomControlGroup` + `TransponderPanel`, ratio verrouillé (`ScaleValue`), opacité, `RefocusDCS`, position persistée `RadioX/Y/Width/Height/Opacity` |
| `AwacsRadioOverlayWindow/AwacsOverlay.xaml` (`AwaRadioOverlayWindow`) | code-behind | 10 radios + intercom, bouton émission simultanée, `AwacsX/Y` |
| `RadioControlGroup`, `AwacsRadioControlGroup` | `DataContext = this` | Boutons fréquence → `Utils/RadioHelper` ; `ChannelViewModel = ClientStateSingleton.FixedChannels[RadioId-1]` |
| `PresetChannels/PresetChannelsView` | `PresetChannelsViewModel` | ComboBox de presets → `RadioHelper.SelectRadioChannel` |
| `TransponderPanel` | → `Utils/TransponderHelper` | IFF |
| `RadioCapabilities` | code-behind | Fenêtre « ? » des capacités de l'avion |
| `AwacsRadioOverlayWindow/InstructorMode/` | `InstructorModeViewModel` | Choisir l'`unitId` d'un autre client → `InstructorModeMessage` |
| `ClientList/ClientListWindow` | code-behind, 3 s | Liste + mute local |
| `ServerSettingsWindow/` | code-behind, 1 s | Affiche `SyncedServerSettings` |
| `InputProfileWindow/` | code-behind | Créer / renommer un profil |
| `InputSettingsControl/InputBindingControl` | DPs `ControlInputBinding`, `InputName` | Une ligne par binding + modificateur |
| `ClientSettingsControl/SettingsToggleControl` | DP `ToggleValue` | Wrapper `ToggleSwitch` pour chaque bool |

Un seul overlay ouvert à la fois (`ToggleSingleRadioStack` / `ToggleMultiRadioOverlay`). Les
raccourcis arrivent via `ToggleSingleStackRadioOverlayMessage` / `ToogleAwacsRadioOverlayMessage`
(typo volontairement conservée), débouncés 500 ms sur le Dispatcher.

Fichiers morts/à ignorer : `UI/ClientWindow/InputBindingControl.xaml`, `InputBindingListControl.xaml`
(doublons anciens), `InputSettingsViewModel.cs` (vide), `Utils/AxisTuningHelper`,
`Audio/Managers/AudioGraphTest.cs` (commenté), `Audio/BufferedSampleProvider.cs` (vide).

## Singletons (`DCS-SR-Client/Singletons/` + `Common/Network/Singletons/`)
- `ClientStateSingleton` : état du joueur. `DcsPlayerRadioInfo` (muté par `DCSRadioSyncHandler`,
  `RadioHelper`, `TransponderHelper`, overlays), `PlayerCoaltionLocationMetadata` (nom/side/siège/
  position), `RadioSendingState`, `RadioReceivingState[11]`, `FixedChannels` (11
  `PresetChannelsViewModel`), `ShortGUID`, `LastSent` (**mettre à 0 force une resync vers le
  serveur**), `IsConnected`, `IsVoipConnected`, `IsGameGuiConnected` / `IsGameExportConnected`
  (< 10 s), `IsLotATCConnected` (< 5 s), `ExternalAWACSModelSelected`, `InhibitTX`, `IntercomOffset`.
  Construit `DCSAutoConnectHandler` (toujours à l'écoute).
- `ConnectedClientsSingleton` : `ConcurrentDictionary<string, SRClientBase>`, `NotifyAll()` 100 ms.
- `SyncedServerSettings` : options serveur reçues.
- `AudioInputSingleton` / `AudioOutputSingleton` : périphériques WASAPI, `WindowsN` détecté.
- `InputDeviceManager` : nécessite `Application.Current.MainWindow` (ne pas toucher `Instance` avant).
- `GlobalSettingsStore` / `ProfileSettingsStore` : voir `architecture/modeles-donnees.md`.

## Entrées (`Singletons/InputDeviceManager.cs`)
- SharpDX DirectInput : clavier, souris, joysticks, périphériques **whitelistés** (GUID intégrés +
  `whitelist.txt`), tout le reste si `ExpandControls` ; `blacklist.txt` exclut. XInput optionnel
  (`AllowXInputController`, GUID fixe). Acquisition `Background | NonExclusive`. Bouton Rescan.
- Modèle : `InputDevice {InputBind, DeviceName, Button, InstanceGuid, ButtonValue}` (Button ≥ 128 =
  POV), `InputBindState` (principal + modificateur + `IsActive`). Enum `InputBinding` dans
  `Common/Settings/GlobalSettingsStore.cs`.
- `AssignButton(callback)` : snapshot puis poll 100 ms jusqu'à changement.
- `StartPTTListening(callback)` : thread dédié, **40 ms**. `GenerateBindStateList()` lit le profil
  courant ; résolution des conflits (bind avec modificateur gagne) ; callback →
  `UDPClientAudioProcessor.PTTHandler` (PTT, switch radio, `RadioSwitchIsPTT`) ; les autres actions
  sont traitées sur place avec anti-répétition (`_lastActiveBinding`) → `RadioHelper` /
  `TransponderHelper` / messages overlay.
- **Bug connu** : la boucle s'arrête à `AwacsOverlayToggle` (137), `TransponderPowerToggle` (138)
  n'est jamais sondé. Commentaire dans le code : « REMEMBER TO UPDATE THIS WHEN NEW BINDINGS ARE ADDED ».
- VAICOM : `Network/VAICOM/VAICOMSyncHandler` (UDP 33501) → `ClientStateSingleton.InhibitTX`.

## Settings côté UI
- `ClientSettingsViewModel` : propriétés `get => store.GetX(key); set { store.SetX(key, value);
  NotifyPropertyChanged(); }`, toutes relevées dans `ReloadSettings()` qui publie
  `ProfileChangedMessage` (écouté par `MainWindowViewModel`, chaque `InputBindingControl`, chaque
  `PresetChannelsViewModel`).
- Profils : `SelectedProfile`, `AvailableProfiles`, commandes Create/Copy/Rename/Delete.
  **Auto-sélection** : `DCSRadioSyncHandler` publie `NewUnitEnteredMessage{Unit, Seat}` →
  `ClientSettingsViewModel.HandleAsync` compare le préfixe alphanumérique du nom d'unité, préfère
  `<nom>_<seat+1>`, sinon `default`.
- Favoris : `Settings/Favourites/CsvFavouriteServerStore` → `FavouriteServers.csv` dans
  `Environment.CurrentDirectory` (ignore `-cfg`).
- Presets : `LastPresetsFolder` ; `Settings/RadioChannels/FilePresetChannelsStore` cherche
  `<nom radio normalisé>.txt` ; `awacs-radios(.custom).json` lus par `DCSRadioSyncHandler`.
- Chaque `Set` écrit le fichier synchrone : jamais dans une boucle chaude.

## Utils
`Utils/RadioHelper` (toutes les mutations radio : fréquence avec `RotaryStyleIncrement` et MIDS,
sélection, guard, clé 1..252, canal, volume, retransmit, simul, intercom ; met `LastSent = 0`),
`Utils/TransponderHelper` (si `iff.control == OVERLAY`), `RadioOverlayWindow/WindowHelper` (Win32
focus), `Utils/ThemeHelper`, `Utils/JsonDCSPropertiesResolver` (+ attribut
`JsonDCSIgnoreSerialization` pour l'envoi vers DCS), `UI/Images.cs`, `UI/Sounds.cs`, convertisseurs
dans `Utils/ValueConverters/`.
