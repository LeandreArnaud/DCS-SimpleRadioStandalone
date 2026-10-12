# Fonctionnalités radio (règles métier)

Ce document décrit **ce que SRS doit faire** vu de l'utilisateur. Le « comment » est dans
`docs-agents/architecture/` et `docs-agents/blocs/`.

## 1. Modèle d'une radio

Chaque client expose jusqu'à N radios (index 0 = intercom en général, puis radios 1..n).
Une radio a :

- une **fréquence** principale en Hz (double), ex. `251000000` = 251.000 MHz ;
- une **fréquence secondaire** (`secFreq`) = Guard, souvent 121.5 ou 243.0 MHz, 0 si inactive ;
- une **modulation** : `AM=0, FM=1, INTERCOM=2, DISABLED=3, HAVEQUICK=4, SATCOM=5, MIDS=6, SINCGARS=7` ;
- un **volume** (0..1) et un **nom** (ex. `AN/ARC-164 UHF`) ;
- un état de **chiffrement** : `enc` (bool), `encKey` (1..252), `encMode` (qui pilote : cockpit ou overlay) ;
- des modes de contrôle : `freqMode`, `volMode` (cockpit ou overlay), `rtMode` (retransmit) ;
- `simul` : peut émettre simultanément avec d'autres radios ;
- `expansion` : radio ajoutée par l'option serveur « Radio Expansion » ;
- `channel` : canal preset courant (-1 si libre) ;
- `freqMin` / `freqMax` : bornes réglables depuis l'overlay.

## 2. Qui peut entendre qui

Deux clients s'entendent sur une radio si (règle cumulative) :

1. Même fréquence (tolérance faible) **et** même modulation (ou Guard : RX sur `secFreq`).
2. Si « Secure Coalition Radios » est actif : même coalition. Les spectateurs ne parlent que si « Spectator Audio ».
3. Chiffrement : 
   - les deux ont `enc` avec la **même clé** → clair ;
   - l'émetteur chiffre et le récepteur non (ou autre clé) → bruit/statique ;
   - l'émetteur en clair et le récepteur chiffré → audible, sauf si « Strict Radio Encryption » (alors bruit).
4. HAVE QUICK : seuls les clients en HQ s'entendent entre eux.
5. INTERCOM : uniquement les membres du **même appareil** (même `unitId`).
6. Si « Line of Sight » actif : le terrain ne doit pas bloquer (sinon perte / atténuation).
7. Si « Distance Limit » actif : la distance doit être dans la portée de la radio ; atténuation progressive.
8. Fréquences **globales** (lobby) : audibles par tous, sans contrainte de coalition, effets optionnellement désactivés.
9. Fréquences de **test** : le serveur renvoie l'audio à l'émetteur (echo).
10. **Retransmit** : une radio en relais rediffuse sur ses autres radios, nombre de sauts limité.

Le serveur applique ces règles pour **router** les paquets. Le client réapplique une partie
(fréquence, modulation, clé) pour décider sur quelle radio mixer et quels effets jouer.

## 3. Émission (TX)

- PTT commun (touche SRS) émet sur la radio **active** (`selected`).
- « Radio Switch works as PTT » : la touche « Radio N » sélectionne ET émet.
- In-cockpit PTT : certains avions envoient le PTT depuis DCS.
- Délais configurables : PTT start delay / release delay.
- Effets TX : clic début/fin, tonalité chiffrement, tonalité MIDS.
- IRL Tx behaviour : pendant l'émission on n'entend pas les autres sur cette radio.
- Émission simultanée possible sur plusieurs radios si `simul` (ex. AV-8B).

## 4. Réception (RX)

- Chaque radio a un **canal audio** (gauche / droite / les deux) et un volume.
- Effets RX : clic début/fin, filtre voix radio (passe-bande), clipping, bruit de fond par bande
  (UHF/VHF/HF/FM), tonalité FM, effet HAVE QUICK, tonalités chiffrement, bruit ambiant cockpit.
- Indicateur : point vert (actif, silence), blanc (réception), orange (inactif), rouge (désactivé).
- Nom de l'émetteur affichable si le serveur l'autorise (« Show Transmitter Name »).
- Compteur de clients « tuned » sur la fréquence si autorisé.

## 5. Contrôle des radios

| Type d'avion | Fréquence / volume | Radio active | Chiffrement |
|--------------|--------------------|--------------|-------------|
| FC3 / spectateur / EAM | Overlay SRS (boutons +/-, slider) ou presets | clic overlay ou touche SRS | onglet Enc de l'overlay |
| Full fidelity | lu depuis le cockpit (lecture seule) | cockpit si l'anim existe, sinon touche SRS | cockpit (KY-58, ODU…) ou overlay si l'avion n'en a pas |

Les options « ALWAYS allow SRS Hotkeys » et « ALWAYS allow Transponder Overlay Controls » forcent le
contrôle via SRS même sur un full-fidelity.

## 6. External AWACS Mode (EAM)

- Connexion sans DCS : nom affiché + mot de passe de coalition (bleu ou rouge, définis sur le serveur).
- Radios définies par `awacs-radios.json` (ou `awacs-radios-custom.json` pour survivre aux mises à jour).
- Cas d'usage : GCI avec LotATC, ATC, un autre jeu.

## 7. Multi-équipage

- `unitId` identique = même appareil → intercom partagé.
- Siège (`seat`) : des profils `_2` pour le second siège (F-14 RIO, AH-64 CPG, F-15E WSO).
- Hot mic intercom possible selon l'avion.

## 8. Transpondeur / IFF

- Le client lit le transpondeur (mode 1/3/4, codes) pour LotATC.
- Option serveur « LotATC Transponder Export ».

## 9. Mise à jour et versions

- Client et serveur vérifient les mises à jour sur GitHub (option « beta »).
- Le serveur refuse ou avertit les clients de version incompatible (vérifier la logique exacte dans
  `docs-agents/architecture/protocole-reseau.md`).
