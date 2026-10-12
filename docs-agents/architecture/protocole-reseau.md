# Protocole réseau

Un seul port serveur (défaut **5002**) utilisé en **TCP** (contrôle, JSON) et **UDP** (voix, binaire).
Côté client, une série de ports UDP localhost dialoguent avec DCS (voir
`docs-agents/blocs/lua-dcs-integration.md`).

## 1. Canal TCP de contrôle (`Common/Models/NetworkMessage.cs`)

- **Framing** : un objet JSON par ligne, terminé par `\n`. `Encode()` ajoute le `\n` ; le serveur
  (`SRSClientSession.OnReceived`) découpe sur `\n` ; le client lit avec `ReadLineAsync`.
- **Sérialisation** : `System.Text.Json`, `IncludeFields = true` (les modèles utilisent des champs
  publics minuscules `freq`, `radios`…), `WhenWritingNull`, enums en **int**. Aucun naming policy.
  Pour cacher un membre : `[JsonIgnore]` (l'attribut `[JsonNetworkIgnoreSerialization]` est quasi
  inopérant, bug dans le resolver).
- **Champs** : `Client` (`SRClientBase`), `MsgType`, `Clients` (liste, seulement dans la réponse
  SYNC), `ServerSettings` (dictionnaire string/string), `ExternalAWACSModePassword`, `Version`.

### `MessageType` (ordre numérique figé, n'ajouter qu'à la fin)

| # | Type | Sens | Rôle |
|---|------|------|------|
| 0 | `UPDATE` | C→S, S→tous | Métadonnées seules (nom, coalition, siège, position). `RadioInfo` null. |
| 1 | `PING` | les deux | Keepalive, contenu ignoré. |
| 2 | `SYNC` | C→S puis S→C | Premier message. Le serveur répond avec tous les clients + settings + version, puis diffuse un `RADIO_UPDATE` du nouveau. |
| 3 | `RADIO_UPDATE` | C→S, S→tous | Mise à jour complète avec `RadioInfo`. |
| 4 | `SERVER_SETTINGS` | S→tous | Les options serveur ont changé. |
| 5 | `CLIENT_DISCONNECT` | S→tous | Un client est parti. |
| 6 | `VERSION_MISMATCH` | S→C | Envoyé avant déconnexion d'un client trop vieux. |
| 7 | `EXTERNAL_AWACS_MODE_PASSWORD` | C→S puis S→C | Login EAM. Réponse `Client.Coalition` = 2 bleu, 1 rouge, 0 échec. |
| 8 | `EXTERNAL_AWACS_MODE_DISCONNECT` | C→S | Quitte EAM sans fermer la socket. |
| 9-11 | `GATEWAY_CLIENT_*` | gateway | Une session représente plusieurs clients virtuels (passerelle DIS). |

### Cycle de vie
1. Connexion TCP (timeout 10 s, NoDelay, keepalive). Le client envoie `SYNC`.
2. Serveur : refuse si `Version` null ou < `UpdaterChecker.MINIMUM_PROTOCOL_VERSION` (`1.9.0.0`).
   Enregistre le client (`ClientGuid` = ShortGuid 22 caractères).
3. Client : même vérification sur la version serveur ; remplit `ConnectedClientsSingleton` et
   `SyncedServerSettings.Decode`.
4. Ensuite le client n'envoie que sur **changement** (`SRClientBase.Equals` / `MetaDataEquals`) ou
   toutes les 180 s (`Constants.CLIENT_UPDATE_INTERVAL_LIMIT`). Le serveur rediffuse sur changement
   ou toutes les 175 s. La position est mise à zéro sauf si `DISTANCE_ENABLED` ou `LOS_ENABLED`.
5. Client : déconnexion si rien envoyé depuis `IdleTimeOut` (600 s) ; après 5 erreurs JSON
   consécutives (= version incompatible). Pas de reconnexion automatique.
6. Serveur : coupe une session dont le buffer d'envoi dépasse 50 Mo ; IP bannies (`banned.txt`)
   refusées à la connexion.

### Settings diffusés
`ServerSettingsStore.ToDictionary()` = section `General Settings` + `SERVER_PRESETS` (JSON) +
`SERVER_EAM_RADIO_PRESET` (JSON). Les sections `Server Settings` et mots de passe EAM ne partent
jamais.

## 2. Canal UDP voix (`Common/Models/UDPVoicePacket.cs`)

Little-endian. Un paquet = une trame Opus de 40 ms + métadonnées.

| Zone | Champ | Taille |
|------|-------|--------|
| En-tête | `PacketLength` (u16, total) | 2 |
| | `AudioPart1Length` (u16) | 2 |
| | longueur zone fréquences (u16 = N × 10) | 2 |
| Audio | octets Opus | `AudioPart1Length` |
| Fréquence × N | `Frequency` (double, **Hz**) | 8 |
| | `Modulation` (byte, enum `Modulation`) | 1 |
| | `Encryption` (byte, clé ; 0 = clair) | 1 |
| Fixe | `UnitId` (u32) | 4 |
| | `PacketNumber` (u64, croissant par émetteur) | 8 |
| | `RetransmissionCount` (byte, sauts) | 1 |
| | `OriginalClientGuid` (22 ASCII, émetteur d'origine) | 22 |
| | `Guid` (22 ASCII, émetteur courant) | 22 |

Constantes : `PacketHeaderLength=6`, `FrequencySegmentLength=10`, `FixedPacketLength=57`,
`GuidLength=22`. Plusieurs fréquences = émission simultanée. Test de référence : 79 octets pour
6 octets audio et 1 fréquence (`DCS-SR-CommonTests/Network/UDPVoicePacketTests.cs`).

**Pièges** : les GUID sont lus **depuis la fin** du buffer ; `EncodePacket` remplit une modulation
manquante avec `4` (HAVEQUICK), pas AM.

### Ping / keepalive
Datagramme de **22 octets exactement** = ping (le ShortGuid ASCII). Le client l'envoie à la
connexion puis toutes les 15 s ; le serveur mémorise `client.VoipPort = remoteEndPoint` et renvoie
les 22 octets ; la première réponse met `UDPVoiceHandler.Ready = true` (`VOIPStatusMessage`).
Tout datagramme > 22 octets est de la voix. 42 s sans rien reçu → le client recrée sa socket.

### Routage serveur (`UDPVoiceRouter.GenerateOutgoingPacket`)
1. `RetransmissionCount > RETRANSMISSION_NODE_LIMIT` → paquet jeté.
2. Émetteur spectateur avec `SPECTATORS_AUDIO_DISABLED`, ou `Muted` → jeté.
3. Pour chaque autre client avec `VoipPort` connu et non `GatewayClient` :
   - fréquence proche (±500 Hz) d'une `GLOBAL_LOBBY_FREQUENCIES`, ou destinataire `Gateway` → envoi ;
   - sinon si `!COALITION_AUDIO_SECURITY || même coalition` et `CanHearTransmission(...)` non null → envoi.
4. L'émetteur reçoit son propre paquet seulement sur `TEST_FREQUENCIES`.

Le serveur ne fait **pas** : LOS, distance, IRL TX/RX, expansion, effets. Ces options sont des
drapeaux appliqués **côté client**.

### Règle « peut entendre » (`PlayerRadioInfoBase.CanHearTransmission`, partagée client/serveur)
- Radio `DISABLED` ignorée.
- `INTERCOM` : seulement si même `unitId` ou `radio.IntercomUnitId == sendingUnitId` (mode instructeur).
- Sinon : `|Δf| < 500 Hz` (`RadioBase.FreqCloseEnough`), même modulation, `freq > 10 kHz`.
- Guard : correspondance sur `secFreq`, modulation ignorée, `IsSecondary = true`.
- Déchiffrable si `(radio.enc ? encKey : 0) == packetKey` ou (`!strict && packetKey == 0`).
- Retourne la meilleure radio **même non déchiffrable** → le client joue du bruit.

### Côté client en plus (`UDPClientAudioProcessor`)
- Fréquences globales : clé forcée à 0.
- `HasLineOfSight` (si `LOS_ENABLED`, AM ≤ 30 MHz exempté) et `InRange` (si `DISTANCE_ENABLED`,
  haversine + Friis).
- `IRL_RADIO_TX` : pendant que j'émets, je n'entends pas sur les radios AM/FM/SINCGARS/MIDS/HQ.
- Priorité des candidats : déchiffrable +16, radio sélectionnée +8, volume > 0 +4.
- `IRL_RADIO_RX_INTERFERENCE` : toutes les candidates passent (effet capture FM), sinon la première.
- Relais : si j'ai des radios `retransmit=true` sur d'autres fréquences, renvoi du même Opus avec
  `RetransmissionCount + 1`, mon GUID en émetteur, GUID d'origine conservé.

## 3. Autres canaux

| Canal | Détail |
|-------|--------|
| Export liste clients | toutes les 5 s, `clients-list.json` si `CLIENT_EXPORT_ENABLED` |
| Export LotATC | toutes les 2 s, UDP JSON vers `LOTATC_EXPORT_IP:PORT` (10712) |
| API HTTP admin | `HttpServer`, `localhost:8080`, header `X-API-KEY`; `GET /clients`, `POST /client/{kick,ban}/{guid,name}...` |
| UPnP | `NatHandler` (SharpOpenNat) ouvre TCP+UDP si `UPNP_ENABLED` |
| LotATC côté client | UDP 10710 (in) / 10711 (out) : position contrôleur, LOS |
| VAICOM | UDP 33501 : inhibition TX |
| ExternalAudio | aucun protocole propre : réutilise `TCPClientHandler` + `UDPVoiceHandler` |

## 4. Compatibilité

- Changer la disposition de `UDPVoicePacket` casse tous les clients. Ajouter une propriété JSON TCP
  est rétro-compatible (inconnus ignorés, nulls omis). Ne jamais réordonner `MessageType`.
- Changement cassant → bump `MINIMUM_PROTOCOL_VERSION`. `UpdaterChecker.VERSION` lit la version de
  **l'assembly d'entrée** (injectée par `-p:VersionPrefix` au publish ; un build dev sans préfixe
  vaut `1.0.0.0` et peut être refusé).
