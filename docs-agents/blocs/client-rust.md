# Bloc : client Rust (`client-rust/`)

Réécriture du client (`DCS-SR-Client`) en Rust, **sans .NET**, binaire statique `SR-ClientRadio.exe`
remplaçable à l'identique dans le dossier `Client/` d'une installation existante. Le client C# reste la
référence de comportement ; le serveur, le Lua et l'installeur ne changent pas.

Plan d'ensemble et jalons : `/Users/leandre/.claude/plans/moonlit-swimming-russell.md` (copie à maintenir ici
quand le plan évolue). État d'avancement : voir « Jalons » en bas.

## Règles propres à ce bloc

- **Compatibilité au fil près** avec `Common/` : toute évolution du protocole C# (`MessageType`, disposition UDP,
  clés `ServerSettingsKeys`, champs JSON) doit être reportée dans `crates/srs-protocol` **et** testée par un
  golden test. Ne jamais « corriger » une bizarrerie C# dans le code Rust sans la documenter ici et dans
  `pieges-et-dette.md`.
- Fichiers `.cfg` : format SharpConfig 4.1 reproduit dans `crates/srs-settings/src/ini.rs`. `global.cfg` sans
  BOM, profils avec BOM, `True`/`False`, `{a,b}`, valeurs avec espace entre guillemets.
- Rien d'allouant ni de bloquant dans le callback audio (M2+).
- Les `.rs` suivent `rustfmt.toml` (LF, 110 colonnes). Les règles CRLF+BOM du dépôt ne s'appliquent qu'aux
  `.cs/.xaml/.lua`.
- Pas de `git add/commit` spontané (règle globale).

## Layout

```
client-rust/
  Cargo.toml                 workspace, profil release (lto fat, strip)
  .cargo/config.toml         +crt-static (msvc/gnu)
  crates/
    srs-protocol/            pur, sans I/O : version, short_guid, modulation, voice_packet, message,
                             hearing, radio_calc, server_settings (+ tests/golden_udp.rs)
    srs-settings/            ini (SharpConfig), keys (enums = clés disque), global, profile, profiles (créer/copier/
                             renommer/supprimer + sélection auto par avion), favourites (FavouriteServers.csv)
    srs-net/                 shared (clients + settings serveur), tcp (session), udp (voix : ping/Ready/42 s)
    srs-dcs/                 model (DCSPlayerRadioInfo/DCSRadio, enums, CanHearTransmission variante DCS), merge
                             (UpdateRadioAsync), radio_helper (RadioHelper), presets (FilePresetChannelsStore +
                             PresetChannelsViewModel.Reload), transponder (TransponderHelper)
    srs-audio/               opus, speex, vad, jitter (port de JitterBuffer.cs), jitter_iface, remote_client
                             (ClientAudioProvider + ambiance cockpit), radio_mixer (RadioMixingProvider + ClientEffectsPipeline :
                             tonalités début/fin, modèle RX, capture FM), master, engine (Arc<Mutex<AudioEngine>>), devices (cpal
                             sortie + capture), resample (micro → 16 kHz), mic_pipeline, volume,
                             effects/{dsp (biquad/1er ordre/compresseur/CVSD/bruit), model (JSON `$type` + défauts ARC-210 et
                             intercom), cache (WAV AudioEffects/ + Ambient/), tones, pipeline (ClientTransmissionPipelineProvider)}
    srs-client/              client (façade : runtime tokio, session, UDP, decode, sortie, capture, encodeur, boucle EAM,
                             listeners DCS, poller d'entrées), decode, encoder, tx (PTTPressed + paquet), eam, dcs_io
                             (sockets UDP locaux), dcs_sync (fusion export → état, overlay 7080, RADIO_UPDATE), combined_state,
                             ptt (PTTHandler + actions radio), radio_state, state
    srs-input/               bind_state (résolution main+modificateur), actions (anti-répétition), poller (thread 40 ms,
                             AssignButton), backend/{directinput,xinput,keys} (Windows), NullBackend ailleurs
    srs-opus-sys/            libopus 1.5.2 vendu (celt/silk/src/include, C portable, sans SIMD), compilé par cc
    speexdsp-preprocess-sys/ sources C speexdsp vendues (preprocess, filterbank, fft, mdf) compilées par cc
    srs-app/                 binaire SR-ClientRadio : args, logging (clientlog.txt), single_instance (mutex), ui/{mod
                             (fenêtre principale, profils, invites, sons, position), settings, controls, favourites,
                             clients, server_settings, radio_widgets, overlay (overlay radio), awacs (overlay AWACS +
                             mode instructeur)}, build.rs (version 4 parties)
```

Non porté (volontairement, voir plan) : enregistrement, LotATC, VAICOM, mise à jour auto, tray, RefocusDCS,
RequireAdmin, passthrough micro, aperçu audio, localisation.

## Décisions techniques

| Sujet | Choix | Note |
|---|---|---|
| UI | eframe/egui 0.36, backend glow | `App::ui(&mut Ui, ..)` (pas `update`) ; overlay = `show_viewport_immediate` + `with_always_on_top` + `with_transparent` |
| Opus | libopus 1.5.2 vendu, compilé par `cc` (`srs-opus-sys`) | cmake abandonné : sous cargo-xwin les fichiers SSE4.1 ne compilent pas avec clang-cl ; build C portable sans RTCD, largement suffisant pour 1 encodeur + N décodeurs |
| Réseau | tokio, une tâche par session, événements `NetEvent` vers l'UI via canal std + `request_repaint` | `TcpSession::spawn` ; tests contre un faux serveur tokio (`srs-net/tests`) |
| Moteur audio RX | `Arc<parking_lot::Mutex<AudioEngine>>` partagé entre le thread decode et le callback cpal | Écart assumé avec le plan « zéro verrou dans le callback » pour M2 ; sections critiques courtes, à remplacer par des files lock-free si des glitches apparaissent |
| Sortie audio | cpal, 48 kHz demandé, sinon interpolation linéaire vers le taux du périphérique | rubato prévu si la qualité ne suffit pas |
| Speex | crate maison, C vendu | `mdf.c` inclus seulement pour le symbole `speex_echo_get_residual` |
| VAD | `webrtc-vad` 0.4 | 16 kHz, 20 ms × 2, mode `VOXMode` |
| ShortGuid | base64 maison 16 octets → 22 chars | ordre d'octets `Guid.ToByteArray()` (mixed-endian) si on part d'un UUID |
| Version | `build.rs` → `SRS_CLIENT_VERSION` = `CARGO_PKG_VERSION` + `.0` (ou env `SRS_VERSION`) | jamais `1.0.0.0` (refus serveur) |

## Bizarreries C# reproduites volontairement

- `UDPVoicePacket` : modulation absente = `4` (HAVEQUICK), GUIDs lus depuis la fin du datagramme.
- `CanHearTransmission` : sur un match guard non déchiffrable, le flag `decryptable` garde la valeur du dernier
  match primaire.
- `SRClientBase.Name` vide → `"---"`. `LatLngPosition` arrondi à 5 décimales (banker's rounding).
- `SyncedServerSettings` : `RETRANSMISSION_NODE_LIMIT` illisible laisse l'ancienne valeur ; `SERVER_PRESETS`
  clés de dictionnaire sensibles à la casse, propriétés insensibles.
- `GetProfiles` : le défaut `{default.cfg}` est immédiatement réécrit `{default}` (test `Contains("default")`).
- `GLOBAL_LOBBY_FREQUENCIES` : le C# parse avec la culture courante (échoue en fr-FR) ; Rust parse toujours
  avec `.`. Divergence assumée (c'est un bug C#).
- `CombinedRadioState` (7080/7082) : le filtre `[JsonDCSIgnoreSerialization]` du C# ne fonctionne pas (il teste
  l'attribut sur le *type* de la propriété) ; tous les champs sont donc émis, comme en C#.
- `UpdateRadioAsync` : mélange de `update.encMode` et `client.encMode` dans la gestion du chiffrement, reproduit.
- Bindings clavier : `button` = index dans l'enum `SharpDX.DirectInput.Key` (table générée `backend/keys.rs`),
  pas un scancode. Joystick : index `rgbButtons`, POV = 128+n avec l'angle en `value`. XInput : GUID fixe
  `bb78c26f-9bfd-41c1-81f0-2f1258d25075`, `button` = bit `GamepadButtonFlags`.
- Effets audio : le compresseur (`SidechainCompressor`) applique `over·(ratio−1)` dB (convention Chunkware), donc
  le ratio 1.18 des modèles livrés *expanse* légèrement ; reproduit. Une transmission `NoAudioEffects` (fréquence
  globale + `RADIO_EFFECT_OVERRIDE`) saute aussi le volume radio. Le modèle RX est toujours l'intercom par défaut
  (HP 270 / LP 4500) car le C# passe `modelName = null`. `lastModulation` du mixeur n'est mis à jour que par les
  tonalités d'émission (les tonalités de réception MIDS dépendent donc de la dernière *émission*).
  Ambiance cockpit : volume = `DecibelsToLinear(GetTargetdB(RMS du WAV, vol))`, un fichier plus silencieux joue
  donc *plus bas*. Capture FM (`IRL_RADIO_RX_INTERFERENCE`) : garde le segment au `ReceivingPower` le plus
  *élevé* (donc la perte la plus forte), comme le C#.
- Presets `.txt` : le nom de fichier normalisé (alphanumérique minuscule) doit être un *préfixe* du nom de la
  radio normalisé (`AN_ARC-210.txt` pour « AN/ARC-210(V) »). Une transmission sur un `secFreq` (guard) n'est
  jamais relayée. Les commandes UDP transpondeur forcent un RADIO_UPDATE même sans transpondeur en overlay.
- UI (C# reproduit) : `AutoConnect` et `AutoConnectPrompt` ne sont lus nulle part (auto-connexion toujours active,
  seule l'invite « mismatch » existe) ; un échec de connexion joue aussi le bip de déconnexion ; les identifiants
  de périphériques stockés sont les ID WASAPI (`{0.0.0.00000000}.{guid}`, `"default"`), lus via `cpal::DeviceId` ;
  `RadioTxEffects_End` est bien lu (le C# lit par erreur `RadioRxEffects_End` dans sa vue, mais le pipeline lit la
  bonne clé) ; sélection auto de profil : préfixe sur le nom normalisé, `_<siège+1>`, un préfixe vide matche tout.
- Divergences mineures assumées (inaudibles) : les générateurs de bruit et le passe-bas 800 Hz gardent leur état
  entre deux trames (le C# les recrée à chaque `Read`) ; les WAV d'ambiance sont tous chargés au démarrage
  (le C# lit le disque depuis le thread audio) ; le nom du modèle radio de l'émetteur est résolu par le thread
  de décodage au lieu du thread audio.

## Livraison pour test Windows

`client-rust/tools/package.sh --build` produit `client-rust/dist/SRS-Client-Rust-<version>.zip` (exe +
`AudioEffects/` + `RadioModels/arc210.json` + `awacs-radios.json` + notice). Procédure de test utilisateur dans
`client-rust/README.md` (« Testing on the Windows PC »).

## Build

- Mac/Linux natif : `cargo test --workspace` (aucune dépendance système : opus/speex/vad compilés par cc).
- Serveur de référence : `docker compose -f client-rust/tools/docker/server/compose.yml up -d` (image construite
  depuis `ServerCommandLine`, émulation x64 ; HTTP API désactivée car `HttpListener` plante sous Linux), puis
  `SRS_TEST_SERVER=127.0.0.1:5002 cargo test -p srs-net --test real_server -- --ignored` (session TCP, EAM, ping)
  et `SRS_TEST_SERVER=127.0.0.1:5002 cargo test -p srs-client --test echo_real_server -- --ignored` (chaîne RX
  complète via les fréquences d'écho du serveur) et `tx_echo_real_server` (chaîne TX complète via l'encodeur). Note : juste après `up`, le proxy Docker accepte puis coupe les
  connexions tant que le serveur n'écoute pas (« connection reset ») ; attendre quelques secondes.
- Windows depuis le Mac : `cargo xwin build --release --target x86_64-pc-windows-msvc -p srs-app`
  (`brew install llvm lld`, `cargo install cargo-xwin`). Vérifié : l'exe n'importe que des DLL système.
- CI : `.github/workflows/client-rust.yml` (fmt, clippy `-D warnings`, tests ubuntu, build+tests windows,
  contrôle des DLL dépendantes, artefact `dist/Client`, pré-release sur tag `client-rust-v*`).

## Jalons

| Jalon | État | Notes |
|---|---|---|
| M0 socle | fait (sauf test sur le PC Windows) | protocole + settings + natifs + squelette UI : tests verts sur mac ; exe Windows cross-compilé, imports = DLL système uniquement |
| M1 session TCP | **fait** | `srs-net::tcp` + UI connexion/EAM/liste clients ; 7 tests faux serveur ; test `real_server` (ignored) vert contre le `ServerCommandLine` C# en Docker : SYNC, écho, EAM bleu → coalition 2, ping UDP → Ready |
| M2 UDP + réception | fait (sauf écoute humaine) | `srs-net::udp`, jitter (port fidèle), décodeur par client, mixeurs, sortie cpal, thread decode, boucle EAM, panneau radios. Test `echo_real_server` (ignored) vert : trames Opus envoyées sur 247.2 AM, renvoyées par le serveur C#, décodées et rendues (pic 0.17). Reste : écoute A/B avec un client C# sur le PC Windows |
| M2 UDP + réception | à faire | |
| M3 émission | fait (sauf micro réel sur Windows) | capture cpal → `Downsampler` (FIR + interpolation) → VAD → Speex → Opus → `tx::ptt_pressed` / `build_packet` → UDP ; PTT = bouton UI / Espace (DirectInput en M4). Test `tx_echo_real_server` (ignored) vert : micro synthétique, PTT, écho serveur, rendu audible |
| M4 DCS + entrées | code fait, à valider sur le PC Windows avec DCS | `srs-dcs::merge` (port ligne à ligne de `UpdateRadioAsync`, 7 tests), `radio_helper` (RadioHelper), `srs-client::dcs_io` (listeners 9084/5068/9085/5069/9040, sorties 7080/7082/9086), `dcs_sync` (ProcessRadioInfoAsync, GameGUI, LOS, commandes, péremption 10 s), `combined_state` (JSON overlay), `srs-input` (résolution des binds, actions, poller 40 ms, DirectInput8 + XInput sous Windows, table `SharpDX.Key`), `ptt` (PTTHandler : délais, switches radio). Test local `dcs_sync_local` (faux export/GameGUI/commande/auto-connect) vert |
| M5 LOS / distance / effets | code fait, à valider à l'oreille sur Windows | LOS/distance (M2), `CurrentlyBlockedRadios` (IRL_RADIO_TX), retransmission (`RetransmitAudio`), chaîne d'effets complète (`effects/`), ambiance cockpit, tonalités début/fin RX et TX (`play_*_transmit` depuis l'encodeur), modèle RX + capture FM dans le mixeur, loader `RadioModels[Custom]/*.json` (+ défauts), presets `.txt` + serveur, `TransponderHelper` + commandes UDP. 40 tests dans `srs-audio`, écho serveur réel toujours vert. Non vérifiable ici : rendu sonore vs C# (procédure README) |
| M6 UI complète + migration | code fait, à valider sur Windows | Fenêtre principale (périphériques par ID WASAPI, favoris CSV, dossier presets, EAM, boost, VU, PTT, pied de page), onglets Settings (toutes les clés global/profil), Controls, Favourites, Clients (tri, R, mute), Server, Log, Help ; profils (créer/copier/renommer/supprimer, sélection auto) ; overlay radio (5 radios dynamiques, intercom, transpondeur, presets, chiffrement, RT, position/taille/opacité persistées) ; overlay AWACS (10 radios, fréquence éditable, SIMUL, intercom 1-30, mode instructeur) ; invite mismatch auto-connect ; sons de connexion ; mutex `-allowMultiple` ; `clientlog.txt` ; position fenêtre, dark mode, start minimised. Non vérifiable ici : rendu Windows, DirectInput réel, transparence overlay au-dessus de DCS |
