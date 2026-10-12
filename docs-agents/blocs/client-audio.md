# Bloc : pipeline audio du client

Le moteur est dans `Common/Audio/**` ; `DCS-SR-Client/Audio/` ne contient que l'orchestration WASAPI
et le traitement des paquets. `SharedAudio/` ne porte que les DLL natives.

## Fichiers clés

| Fichier | Rôle |
|---------|------|
| `DCS-SR-Client/Audio/Managers/AudioManager.cs` | Orchestrateur : capture WASAPI, resample 16 kHz, VOX, Speex, Opus, construction du graphe RX, passthrough micro, registre des `ClientAudioProvider` |
| `DCS-SR-Client/Audio/Managers/UDPClientAudioProcessor.cs` | Machine d'état PTT, choix des radios qui émettent, construction des paquets, thread de décodage RX, décision « puis-je entendre », relais (retransmit), état de réception |
| `DCS-SR-Client/Audio/Managers/AudioPreview.cs` | Test micro (capture → Opus → décodage → filtre ARC-210 → sortie), sans réseau |
| `DCS-SR-Client/Audio/Managers/EventDrivenResampler.cs` | Resampler push (DMO, ou WDL sur Windows N / Wine) |
| `DCS-SR-Client/Audio/Utility/SpeexProcessor.cs` | Denoise micro (AGC désactivé), settings relus toutes les ~5 s |
| `DCS-SR-Client/Audio/Utility/NAudio/SRSWasapiOut.cs` | Fork de `WasapiOut` (event-sync, fallback de format) |
| `Common/Audio/Providers/ClientAudioProvider.cs` | Un par **client distant** : décodage Opus 48 kHz, 11 jitter buffers, AGC/denoise entrant, atténuation LOS/distance, ambiance cockpit, bruit si non déchiffrable, chaîne TX |
| `Common/Audio/Providers/ClientTransmissionPipelineProvider.cs` | Effets « côté émetteur » : clipping, bruit de fond, tonalités NATO/HQ, modèle radio TX, CVSD si chiffré, wet/dry, volume |
| `Common/Audio/Providers/RadioMixingProvider.cs` | Une par **radio locale** : mixe tous les clients, tonalités début/fin, filtres RX, capture FM, balance G/D → stéréo |
| `Common/Audio/Providers/ClientEffectsPipeline.cs` | Somme des segments, capture FM, modèle RX, clipping |
| `Common/Audio/Providers/SRSMixingSampleProvider.cs`, `VolumeSampleProviderWithPeak.cs` | Mixeur final + gain « speaker boost » + VU |
| `Common/Audio/Providers/CachedAudioEffectProvider.cs` | Charge `AudioEffects/*.wav` (+ `Ambient/` à la demande) |
| `Common/Audio/Models/RadioModel.cs` | Schéma JSON des modèles radio, factory, défauts ARC-210 / intercom |
| `Common/Audio/Utility/Speex/JitterBuffer.cs`, `Providers/JitterBufferProviderInterface.cs` | Port de `jitter.c` de speexdsp, 200 slots, `JITTER_MS=400` |
| `Common/Audio/Recording/*` | Enregistrement MP3/Opus par radio ou mixdown |

## Chaîne TX (émission)

1. `AudioInputSingleton` énumère les périphériques WASAPI (NAudio `MMDeviceEnumerator`).
2. `AudioManager.InitMicInput` : `WasapiCapture` mode partagé, format natif.
3. `EventDrivenResampler` → 16 kHz / 16 bit / mono (créé **sur le thread de capture**, affinité COM).
4. File `_micInputQueue` ; trames de **640 échantillons = 40 ms**.
5. VOX optionnel : `WebRtcVadSharp` sur 2 × 20 ms + RMS > `VOXMinimumDB`.
6. `SpeexProcessor` : denoise (`Denoise`, `DenoiseAttenuation`). Pas d'AGC micro, pas de boost appliqué.
7. `OpusEncoder.Create(16000, 1, Voip)`, FEC off, **bitrate par défaut** (jamais fixé).
8. `UDPClientAudioProcessor.Send` : exige `_udpClient.Ready`, données DCS fraîches, micro dispo,
   `PTTPressed()` ≥ 1 radio. `PTTPressed` gère VAICOM inhibit, intercom (hot mic / `IntercomPTT`),
   PTT SRS ou PTT cockpit (`DcsPlayerRadioInfo.ptt`), radio sélectionnée (non DISABLED, freq > 100
   ou INTERCOM, non rxOnly), émission simultanée (`simul`). `PTTHandler` (appelé par
   `InputDeviceManager`, 40 ms) applique `PTTStartDelay`, `PTTReleaseDelay`, `RadioSwitchIsPTT`.
9. Paquet : fréquences dédupliquées (freq, modulation, clé) ; `Encryption = enc ? encKey : 0` ;
   `UnitId` remplacé par `IntercomUnitId` en mode instructeur ; `PacketNumber++`.
10. `UDPVoiceHandler.Send` (file + sémaphore, thread UDP dédié). Tonalité TX locale jouée au
    basculement de `RadioSendingState`. L'audio propre part aussi vers `_passThroughAudioProvider`
    (sortie micro optionnelle, enregistrement).

Threads TX : capture NAudio (resample/VAD/Speex/Opus/paquet), thread UDP (socket), thread
`InputDeviceManager` (PTT, `volatile bool _ptt`).

## Chaîne RX (réception)

1. Thread `UdpAudioDecode` : `EncodedAudio.Take()` → `UDPVoicePacket.DecodeVoicePacket`.
2. Pour chaque fréquence du paquet : fréquences globales → clé forcée 0 ;
   `DCSPlayerRadioInfo.CanHearTransmission` (voir `protocole-reseau.md`) ; puis, sauf
   INTERCOM/MIDS/global : `HasLineOfSight` (`LOS_ENABLED`, AM ≤ 30 MHz exempté,
   `LineOfSightLoss` du client émetteur) et `InRange` (`DISTANCE_ENABLED`, haversine + Friis,
   `ReceivingPowerLossPercent`) et radio non bloquée (`IRL_RADIO_TX`).
3. Tri des candidats (déchiffrable +16, sélectionnée +8, volume +4). Première seulement, sauf
   `IRL_RADIO_RX_INTERFERENCE`. `RadioReceivingState[radio]` mis à jour (`IsReceiving` < 350 ms,
   `SentBy` si `SHOW_TRANSMITTER_NAME`). Puis `RetransmitAudio()`.
4. `AudioManager.AddClientAudio` : `ClientAudioProvider` par `OriginalClientGuid`, enregistré comme
   source de **chaque** `RadioMixingProvider`. Décodage Opus 48 kHz (reset si trou > 400 ms) → jitter
   buffer de la radio reçue.
5. Thread de rendu WASAPI (40 ms) : `SRSWasapiOut` ← `VolumeSampleProviderWithPeak` ←
   `SRSMixingSampleProvider` ← N `RadioMixingProvider`.
6. `RadioMixingProvider.Read` : vide les tonalités en attente, lit chaque `ClientAudioProvider.Read`
   (jitter → Speex entrant → si déchiffrable : atténuation LOS/distance + ambiance cockpit, sinon
   bruit → `ClientTransmissionPipelineProvider.Process`) → segments → enregistrement →
   tonalités début/fin → `ClientEffectsPipeline.ProcessSegments` (somme, capture FM, modèle RX,
   wet/dry, clipping) → balance G/D (`Radio{N}Channel`, `IntercomChannel`).

Plusieurs clients sur la même fréquence : un jitter buffer chacun, sommés puis écrêtés (sauf
capture FM qui garde le plus fort).

## Effets : ordre dans `ClientTransmissionPipelineProvider.BuildRadioEffectsChain`
1. `ClippingProvider(±4000/32768)` si `RadioEffectsClipping`.
2. Bruit de fond si `RadioBackgroundNoiseEffect` : gain = −10·ln(f/1e6)/2 + `NoiseGainDB`
   (ou `HFNoiseGainDB` ≤ 25 MHz) + `noiseGain` du modèle ; source bruit blanc gaussien (> 25 MHz) ou
   rose (HF) + passe-bas 800 Hz. Tonalité `NATO_TONE` (FM/SINCGARS) ou `HAVEQUICK_TONE` (HQ) en boucle.
3. Modèle radio TX (`txEffect`) ou chaîne `encryptionEffect` (CVSD) si `RadioEncryptionEffects` et
   (MIDS ou clé > 0).
4. Clipping ±1, wet/dry (`RadioEffectsRatio`), volume de la radio.
Ignoré entièrement si `NoAudioEffects` (`RADIO_EFFECT_OVERRIDE` sur fréquence globale).

Tonalités début/fin (`RadioMixingProvider`) : intercom → `INTERCOM_TRANS_*` ; chiffré → `KY_58_TX`
(propre TX) / `KY_58_RX` ; MIDS → `MIDS_TX` / `MIDS_TX_END` ; sinon fichier choisi par
`RadioTransmissionStart/EndSelection` (tout WAV commençant par `RADIO_TRANS_START/END`). Gatés par
`RadioRx/TxEffects_Start/End`.

## Modèles de radio (`DCS-SR-Client/RadioModels/`)
Presets DSP **par type de radio** (pas par avion). JSON `{version, noiseGain, txEffect, rxEffect,
encryptionEffect?}`, effets polymorphes `$type` : `chain`, `filters` (`lowpass|highpass|peak`,
biquad si `q`), `saturation`, `compressor`, `sidechainCompressor`, `gain`, `cvsd`. Choix du modèle :
`RadioBase.Model` de la radio **de l'émetteur** (vient du champ `model` exporté par le Lua) si
`PerRadioModelEffects`, sinon ARC-210 intégré. `RadioModelFactory` lit `*.json` non récursivement
dans `<cwd>/RadioModels` et `<cwd>/RadioModelsCustom`. État actuel : seul `arc210.json` est copié
en sortie ; les autres sont dans `WIP/` et ne sont pas chargés ; le modèle RX appliqué est toujours
l'intercom par défaut (`modelName` passé à null, TODO dans le code). Doc : `RadioModels/HOWTO.md`.

## Constantes et pièges
- 16 kHz mono en capture, 48 kHz mono float partout en RX ; `DeferredSourceProvider` lève si la tête
  de chaîne n'est pas 48 kHz mono float.
- Les settings sont **sondés** (3 s) : un changement met quelques secondes à s'appliquer.
- Chaînes d'effets **stateful** et cachées par modèle **par provider** (un par client distant).
- Verrous : `JitterBufferProviderInterface._lock`, `lock(sources)` dans les mixeurs. Buffers poolés
  (`PooledArray`, `ArrayPool`) : rendre `DeJitteredTransmission.PCMMonoAudio` via `Dispose(ref)`.
- `AddClientAudio` indexe par `OriginalClientGuid`, `RemoveClientBuffer` par `ClientGuid`.
- Effets chargés depuis `AppDomain.BaseDirectory`, modèles depuis `Directory.GetCurrentDirectory()`.
- Ordre de disposal dans `AudioManager.StopEncoding` : processor → capture → VAD → resampler →
  waveOut → micWaveOut → mixeurs → encodeur → UDP → Speex → recording → unsubscribe.
- Fichiers `*_NOISE.wav` livrés mais non utilisés (bruit synthétisé).
