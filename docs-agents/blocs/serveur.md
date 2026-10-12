# Bloc : serveur (`Server/`, `ServerCommandLine/`, cœur dans `Common/Network/Server/`)

**Le serveur est dans `Common`.** `Server/` (WPF) et `ServerCommandLine/` (console) sont des hôtes
minces autour de `ServerState`.

## Fichiers

| Fichier | Rôle |
|---------|------|
| `Common/Network/Server/ServerState.cs` | Orchestrateur : `ConcurrentDictionary<string, SRClientBase> _connectedClients`, `_bannedIps`, crée `UDPVoiceRouter`, `ServerSync`, `HttpServer` ; exports JSON (5 s) et LotATC (2 s) ; `banned.txt` ; handlers `StartServerMessage`, `StopServerMessage`, `KickClientMessage`, `BanClientMessage` |
| `Common/Network/Server/ServerSync.cs` | Serveur TCP (`: TcpServer`) : handshake, version, SYNC/UPDATE/RADIO_UPDATE/EAM/gateway, diffusion des settings |
| `Common/Network/Server/SRSClientSession.cs` | Session par connexion : check ban, framing `\n`, kill client lent (> 50 Mo) |
| `Common/Network/Server/UDPVoiceRouter.cs` | Boucle UDP + routage (classe `internal`) |
| `Common/Network/Server/HttpServer.cs` | API admin `HttpListener`, `X-API-KEY` |
| `Common/Network/Server/NatHandler.cs` | UPnP / NAT-PMP |
| `Common/Network/Server/TransmissionLogging/` | Log CSV via target NLog dédiée |
| `Server/Bootstrapper.cs` | Caliburn `BootstrapperBase` : `-cfg=`, Sentry, NLog, `SimpleContainer` (`IWindowManager`, `IEventAggregator`, `ServerState`, `MainViewModel`, `ClientAdminViewModel`), affiche `MainViewModel`, check update bloquant |
| `Server/App.xaml` | Instancie le `Bootstrapper` en ressource (pas d'`App.xaml.cs`) |
| `Server/NLog.config` | `serverlog.txt` ; si présent, la config programmatique est ignorée (garder en phase) |
| `Server/UI/MainWindow/MainView.xaml` + `MainViewModel.cs` | Start/Stop, compteur, port, un bouton toggle par option, textboxes EAM / test / global (debounce 500 ms), sliders rétention / hops |
| `Server/UI/ClientAdmin/` | Liste des clients : nom, dernière fréquence, coalition, Kick, Ban, Mute (en mémoire) |
| `ServerCommandLine/Program.cs` | `Parser.Default.ParseArguments<Options>` ; applique les options sur `ServerSettingsStore` ; `ServerState` sur un thread ; attend Ctrl-C / SIGTERM ; `IHandle<SRSClientStatus>` affiche connexions |
| `Server/Server_s3vspq0t_wpftmp.csproj` | Fichier temporaire WPF commité par erreur, hors .sln. **Ignorer.** |

## Démarrage
- GUI : résoudre `ServerState` **démarre** le serveur (ctor → `StartServer()`). `OnExit` → `StopServer()`.
- CLI : `-c/--cfg` (le setter retire un `fg=` parasite car `-cfg=x` est lu comme `-c fg=x`),
  options → `Set*Setting` (chaque Set **écrit** `server.cfg`), `new ServerState(eventAggregator)`.
  Flags : `--port`, `--serverBindIP`, `--coalitionSecurity`, `--spectatorAudioDisabled`,
  `--clientExportEnabled/Path`, `--realRadioTx`, `--realRadioRx`, `--radioExpansion`, `--enableEAM`,
  `--eamBluePassword`, `--eamRedPassword`, `--betaUpdates`, `--allowRadioEncryption`,
  `--strictRadioEncryption`, `--testFrequencies`, `--globalLobbyFrequencies` (**jamais appliqué**,
  bug), `--showTunedCount`, `--showTransmitterName`, `--lotATCExport*`, `--retransmitNodeLimit`,
  `--transmissionLogEnabled`, `--radioEffectOverride`, `--serverPresetChannelsEnabled`,
  `--serverEAMRadioPresetEnabled`, `--allowInstructorMode`, `--httpServer*`, `--consoleLogs`.
- Ports : `SERVER_PORT` pour TCP (bind `Any`, codé en dur) et UDP (bind `SERVER_IP`). HTTP admin
  optionnel. `NatHandler` si `UPNP_ENABLED` (découverte 10 s, fermeture best-effort).
- Échec de bind TCP → log + **`Environment.Exit(0)`**. Échec de bind UDP → exception non observée
  dans `Task.Run`, le TCP continue sans voix (à confirmer).

## TCP (`ServerSync.HandleMessage`)
- `HandleConnectedClient` pour tout sauf `GATEWAY_*` : client inconnu → vérif `Version` (null ou
  < 1.9.0.0 → `VERSION_MISMATCH` + déconnexion), sinon enregistre, `ClientSession = session.Id`,
  publie `ServerStateMessage`.
- `UPDATE` : `MetaDataEquals(usePosition: true)` ; copie champ par champ et `Multicast` si changé
  ou > 175 s. `RADIO_UPDATE` : idem + `PlayerRadioInfoBase.Equals`. `SYNC` : réponse à la session
  seule puis diffusion. `SERVER_SETTINGS` : rediffusion (aussi déclenchée par
  `ServerSettingsChangedMessage` depuis la GUI). `EXTERNAL_AWACS_MODE_PASSWORD` : bleu → 2, rouge → 1,
  sinon 0 ; réponse + `UPDATE` à tous. `EXTERNAL_AWACS_MODE_DISCONNECT` : coalition 0, radios vides.
  `GATEWAY_*` : clients virtuels d'une session passerelle, retirés avec elle.
- `HandleDisconnect` : retire, `MulticastAllExeceptOne(CLIENT_DISCONNECT)`, `ServerStateMessage`.
- Kick : `FindSession(client.ClientSession)?.Disconnect()`. Ban : IP ajoutée à `_bannedIps` et
  `banned.txt` (`AppContext.BaseDirectory`), relu à chaque `StartServer`.
- Presets : `ServerChannelPresetHelper(<dossier du cfg>/Presets/*.txt)` chargé **une fois** ;
  `Presets/awacs-radios-custom.json` (11 radios). Redémarrer pour recharger.
- Log transmissions : `TransmissionLoggingQueue.LogTransmission` pour `RetransmissionCount == 0`,
  thread 500 ms, ligne `TRANSMISSION, guid, name, coalition, freq, start, end, voipEndpoint` après
  0,4 s de silence ; fichiers `yyyy-MM-dd-transmissionlog.csv`, archive `TransmissionLogArchive/`.

## UDP (`UDPVoiceRouter`)
Voir `docs-agents/architecture/protocole-reseau.md` §2. Détails : `UdpClient` `ExclusiveAddressUse`,
`DontFragment`, `AllowNatTraversal` (Windows) ; `ReceiveAsync` en boucle ; 22 octets = ping (met à
jour `VoipPort`, écho) ; sinon `Task.Run(ProcessPendingPacketAsync)` ; GUID = 22 derniers octets ;
exceptions avalées. `_testFrequencies` / `_globalFrequencies` sont des listes remplacées par
référence sur `ServerFrequenciesChanged`. Settings relus via SharpConfig à chaque paquet.

**Pas fait par le serveur** : LOS, distance, IRL, expansion, effets, modification du paquet.

## GUI (Caliburn conventions)
`x:Name="FooToggle"` sur un `Button` → méthode `FooToggle()` ; `Content="{Binding FooText}"`. Chaque
toggle : `SetGeneralSetting(key, !current)`, `NotifyOfPropertyChange`, publie
`ServerSettingsChangedMessage`. Les textboxes de fréquences publient `ServerFrequenciesChanged`
(pas `ServerSettingsChangedMessage` : les clients voient la nouvelle `GLOBAL_LOBBY_FREQUENCIES` à
la prochaine diffusion seulement). Pas de GUI pour `UPNP_ENABLED`, `SERVER_IP`, `HTTP_*`,
`LOTATC_EXPORT_IP/PORT`, `CLIENT_EXPORT_FILE_PATH` (éditer `server.cfg`). Strings dans
`Server/Properties/Resources(.zh).resx`.

## Linux (`ServerCommandLine`)
Aucune dépendance WPF dans la chaîne (`Common` a `UseWPF=false`). Arrêt propre via
`PosixSignalRegistration(SIGTERM)` + `CancelKeyPress`. Publié self-contained single-file
(`ServerCommandLine-Linux/SRS-Server-Commandline`). `server.cfg` relatif au CWD, `banned.txt` et
`clients-list.json` dans `AppContext.BaseDirectory`, `Presets/` à côté du cfg.
`ServerState.NormalizePath` utilise `new Uri(path)` : un `CLIENT_EXPORT_FILE_PATH` relatif lève
`UriFormatException`.

## Pièges
- Deux bus : `IEventAggregator` injecté (`ServerState`, `ServerSync`, `UDPVoiceRouter`, VMs) vs
  `EventBus.Instance` (`SRSClientSession` publie `SRSClientStatus`).
- `ServerSettingsStore.CFG_FILE_NAME` est statique et doit être fixé **avant** le premier `Instance`.
- `MessageType` sérialisé en int : n'ajouter qu'à la fin.
- Rediffusion heartbeat 175 s (`CLIENT_UPDATE_INTERVAL_LIMIT - 5`).
- `Options.ToString()` du CLI mélange deux libellés (EnableEAM / EAMRedPassword).
