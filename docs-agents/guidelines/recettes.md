# Recettes : comment faire les modifications courantes

Chaque recette liste les fichiers à toucher, dans l'ordre. Vérifier les invariants de
`docs-agents/blocs/common.md` avant de commencer.

## 1. Ajouter une option serveur (bout en bout)
1. `Common/Settings/Setting/ServerSettingsKeys.cs` : ajouter le membre **à la fin** de
   `ServerSettingsKeys` + défaut (string) dans `DefaultServerSettings.Defaults`.
2. Section : `GetGeneralSetting`/`SetGeneralSetting` si les clients doivent la voir (incluse
   automatiquement dans `ToDictionary()`), `GetServerSetting`/`SetServerSetting` si locale.
3. Comportement serveur : lire `ServerSettingsStore.Instance.GetGeneralSetting(key).BoolValue`
   dans `UDPVoiceRouter` / `ServerSync` / `ServerState`.
4. GUI : `Server/UI/MainWindow/MainView.xaml` (copier une ligne `Button x:Name="XToggle"
   Content="{Binding XText}"`), `MainViewModel.cs` (`XText` + `XToggle()` qui set, notifie, publie
   `ServerSettingsChangedMessage`), strings dans `Server/Properties/Resources.resx` (+ zh).
5. CLI : `ServerCommandLine/Program.cs` → `[Option]` dans `Options`, application dans `StartServer`,
   ligne dans `ToString()`.
6. Client : `SyncedServerSettings.Instance.GetSettingAsBool(key)` ; parsing spécial dans `Decode`
   seulement si non booléen ; affichage dans `ServerSettingsWindow.xaml(.cs)` si visible.
7. Documenter dans `docs-agents/metier/parametres-serveur.md` et `architecture/modeles-donnees.md`.

## 2. Ajouter une option client (globale ou de profil)
1. Globale : `GlobalSettingsKeys` + `defaultGlobalSettings` dans `Common/Settings/GlobalSettingsStore.cs`.
   Profil : `ProfileSettingsKeys` + `DefaultSettingsProfileSettings` dans `ProfileSettingsStore.cs`.
2. `DCS-SR-Client/UI/ClientWindow/ClientSettingsControl/ClientSettingsViewModel.cs` : propriété
   `get/set` sur le store + ajout dans `ReloadSettings()`.
3. `Properties/Resources.resx` (+ `.zh.resx`) : libellé et tooltip.
4. `ClientSettings.xaml` : `<Label Content="{x:Static p:Resources.X}"/>` +
   `<clientSettingsControl:SettingsToggleControl ToggleValue="{Binding X, Mode=TwoWay}"/>` (ou
   slider/combobox).
5. Consommer : les providers audio relisent les settings toutes les 3 s (`ReLoadSettings` /
   `RefreshSettings`) ; ailleurs lire via le store. Si réaction immédiate nécessaire, publier un
   message (modèle : `ServerSettingsPresetsSettingChangedMessage`).

## 3. Ajouter un raccourci (InputBinding)
1. `enum InputBinding` (`Common/Settings/GlobalSettingsStore.cs`) : `Foo = 139` et `ModifierFoo = 239`.
2. `DCS-SR-Client/Singletons/InputDeviceManager.cs` : étendre la borne haute dans
   `GenerateBindStateList()` et le test de plage dans `StartPTTListening` ; ajouter `case
   InputBinding.Foo:` → `RadioHelper` / `TransponderHelper` / message. Action « maintenue » type
   PTT → `UDPClientAudioProcessor.PTTHandler` à la place.
3. `Properties/Resources.resx` : `InputFoo`.
4. `UI/ClientWindow/InputSettingsControl/InputSettings.xaml` :
   `<inputSettingsControl:InputBindingControl ControlInputBinding="Foo" InputName="{x:Static p:Resources.InputFoo}"/>`.
   Le profil persiste automatiquement (section nommée d'après l'enum).

## 4. Ajouter un message TCP
1. `Common/Models/NetworkMessage.cs` : membre **à la fin** de `MessageType`, propriété de payload
   éventuelle (null omis sur le fil).
2. Serveur : `case` dans `ServerSync.HandleMessage` (réponse à un : `session.Send(msg.Encode())`,
   à tous : `Multicast`, tous sauf un : `MulticastAllExeceptOne`). État par connexion → propriété
   sur `SRSClientSession`. Publier `ServerStateMessage` si la liste change.
3. Client : `case` dans `TCPClientHandler.ClientSyncLoopAsync` ; réaction UI via POCO dans
   `Common/Models/EventMessages/` + `EventBus.Instance.PublishOnUIThreadAsync` + `IHandle<T>`.
   Client → serveur : `IHandle<MaRequete>` dans `TCPClientHandler` qui appelle `SendToServerAsync`.
4. Rupture de compatibilité → bump `UpdaterChecker.MINIMUM_PROTOCOL_VERSION`.

## 5. Propager un nouveau champ de `SRClientBase` / `RadioBase`
Ajouter le champ, puis : `Equals` / `MetaDataEquals` / `DeepClone` (`SRClientBase`, `RadioBase`,
`PlayerRadioInfoBase`), copies manuelles dans `ServerSync.HandleClientMetaDataUpdate`,
`HandleGatewayClientUpdate`, `SendFullRadioUpdate`, et `TCPClientHandler.HandlePartialUpdate`.
Mettre à jour `DCSRadio` (client) et `DCSRadioCustom` (EAM) si c'est un champ de radio, et le Lua
`DCS-SimpleRadioStandalone.lua` si DCS doit le fournir.

## 6. Modifier une règle de routage voix
- Rejets au niveau émetteur (spectateur, mute) : `UDPVoiceRouter.ProcessPendingPacketAsync`.
- Décision par destinataire (hops, global, gateway, coalition, test) : `UDPVoiceRouter.GenerateOutgoingPacket`.
- Niveau radio (fréquence/modulation/chiffrement/guard/intercom) :
  `PlayerRadioInfoBase.CanHearTransmission` — **partagé avec le client**.
- Côté client seulement (LOS, distance, IRL, priorités) : `UDPClientAudioProcessor.UdpAudioDecode`.
- Gater derrière une option (recette 1). Ajouter un test dans `DCS-SR-CommonTests`.

## 7. Ajouter un effet audio
- Bloc DSP réutilisable : `ISampleProvider` dans `Common/Audio/Providers/` (modèle
  `FiltersProvider`, `SaturationProvider`), puis exposition JSON : classe `XxxEffect : IEffect` +
  `[JsonDerivedType(typeof(XxxEffect), "xxx")]` dans `Common/Audio/Models/RadioModel.cs`, doc dans
  `DCS-SR-Client/RadioModels/HOWTO.md`.
- Câblé en dur : `ClientTransmissionPipelineProvider.BuildRadioEffectsChain` (par transmission
  distante) ou `ClientEffectsPipeline.ProcessSegments` (par radio locale). 48 kHz mono float, chaîne
  réutilisée entre transmissions (reset si stateful).
- Son échantillonné : WAV 48 kHz/16 bit/mono dans `DCS-SR-Client/AudioEffects/`, `<Content>`
  `CopyToOutputDirectory` dans `DCS-SR-Client.csproj`, membre dans
  `CachedAudioEffect.AudioEffectTypes`, chargement dans `CachedAudioEffectProvider.LoadEffects`,
  lecture via `effectsBuffer.Write` (`RadioMixingProvider`, one-shot) ou `VolumeCachedEffectProvider`
  (`ClientTransmissionPipelineProvider.LoadTones`, boucle). Toggle = `ProfileSettingsKeys` (recette 2).

## 8. Ajouter un modèle de radio (DSP)
Copier `DCS-SR-Client/RadioModels/arc210.json` en `<model>.json` (ou `RadioModelsCustom/` chez
l'utilisateur), régler `txEffect/rxEffect/noiseGain`, ajouter un `<None Update>` avec copie en
sortie dans `DCS-SR-Client.csproj`, faire exporter `model = "<model>"` par le module Lua de l'avion
(`SR.RadioModels`). Nom normalisé (alphanumérique minuscule).

## 9. Ajouter un avion (module Lua)
Voir `docs-agents/blocs/lua-dcs-integration.md` §6. Résumé :
1. Nom DCS = `LoGetSelfData().Name`.
2. Créer `Scripts/DCS-SRS/Scripts/DCS-SRS-Modules/<Short>.lua` sur le modèle de `KA50.lua` (simple)
   ou `UH1H.lua` (sièges, IFF, hot mic) : `exportRadioXYZ(_data, SR)` remplit `_data.radios[n]`
   (`name`, `freq` via `SR.getRadioFrequency(deviceId)`, `modulation`, `volume`, `model`, `encMode`,
   `channel`, `secFreq`…), `_data.control`, `_data.capabilities`, `_data.iff`, `_data.ambient`,
   retourne `_data` ; `register(SR)` met `SR.exporters["<Nom>"]`.
3. Ids de devices/arguments : `Cockpit/Scripts/devices.lua`, `clickabledata.lua`,
   `mainpanel_init.lua` du module DCS ; `list_indication(n)` pour les textes UFC.
4. Côté C# rien d'obligatoire. Optionnel : WAV `AudioEffects/Ambient/<abType>.wav`, modèle radio,
   ligne dans `docs-agents/metier/aeronefs-supportes.md`.
5. `stylua`, test en vol, surveiller `dcs.log` (`SRS-export`).

## 10. Ajouter une commande UDP externe (port 9040)
`DCS-SR-Client/Network/UDPCommandHandler.cs` : membre dans `UDPCommandType` (à la fin) + `case` →
`RadioHelper`/`TransponderHelper`. Côté Lua, `DCS-SRSGameGUI.lua` (`handleRadio` /
`handleTransponder`) si une commande chat doit la déclencher.

## 11. Ajouter une fenêtre / un onglet WPF client
Créer `UI/ClientWindow/<Nom>/<Nom>.xaml` (+ VM héritant de `PropertyChangedBaseClass`), ouvrir
depuis `MainWindowViewModel` (modèle `ToggleClientList`), strings en resx, respecter le thème
MahApps (ressources de `Themes/Styles.xaml`), timers `DispatcherTimer` pour le rafraîchissement.

## 12. Changer les paramètres de capture audio
`Common/Constants.cs` (taux/trame ; Opus accepte 8/12/16/24/48 kHz, 2,5-120 ms ; adapter
`AudioManager.DoesFrameContainSpeech` et `InitVox` qui supposent 2 × 20 ms),
`AudioManager.InitEncodersSpeex` + `AudioPreview.StartPreview` (`OpusEncoder.Create`, `Bitrate`,
`ForwardErrorCorrection` ; décodeur `AudioProvider` doit suivre pour le FEC),
`SpeexProcessor.RefreshSettings` (denoise micro), `ClientAudioProvider.ReLoadSettings` (AGC entrant),
`AudioManager.InitMicInput` / `InitWaveOut` (périphériques, latence 40 ms).
