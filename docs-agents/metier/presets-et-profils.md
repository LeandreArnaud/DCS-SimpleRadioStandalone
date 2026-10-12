# Presets, channelisation et profils

## Channelisation (`.txt`)

- Un fichier par radio, nommé d'après le **nom de la radio** normalisé (alphanumérique, minuscules,
  casse ignorée). Ex. `ANARC-164 UHF1.txt` ou `anarc164 UHF1.txt` pour la 1re UHF du F-15C.
  `AN/ARC-210 COMM2` → `anarc210comm2`.
- Une fréquence par ligne, en MHz. Optionnel : `NOM|freq` (`TACOM|267.75`).
- Emplacement : dossier du client (`SR-ClientRadio.exe`) ou dossier choisi via « Browse Presets folder ».
- Chargé automatiquement quand la radio apparaît ; bouton reload dans l'overlay.
- S'applique aux radios FC3 et aux radios d'expansion (pas aux radios cockpit full-fidelity).
- Noms de radios : voir `Scripts/DCS-SRS/Scripts/DCS-SimpleRadioStandalone.lua` et les modules.

## Presets côté serveur (≥ 2.2.0.4)

Dossier `Presets/` à côté du `server.cfg`, même format. Option serveur « Server Channel Presets ».
Les clients ayant opté pour les presets serveur voient la liste du serveur.

## `awacs-radios.json` (radios EAM)

Définit les radios disponibles en External AWACS Mode. Copier en `awacs-radios-custom.json` pour
survivre aux mises à jour. Champs par radio :

```json
{
  "enc": false, "encKey": 1, "encMode": 1,
  "freqMax": 1060000000.0, "freqMin": 1030000000.0, "freq": 1030100000,
  "modulation": 6, "name": "MIDS Radio", "secFreq": 0.0, "volume": 1.0,
  "freqMode": 1, "volMode": 1, "expansion": false, "channel": -1,
  "simul": false, "rtMode": 1
}
```

Modulation : `0 AM, 1 FM, 2 INTERCOM, 3 DISABLED, 4 HAVEQUICK, 5 SATCOM, 6 MIDS, 7 SINCGARS`.
Une radio MIDS expose 126 canaux dans la liste déroulante. Fréquences en **Hz**.

## Profils client

- Un profil = ensemble de `ProfileSettingsKeys` (touches, effets, audio).
- Actions : créer, copier (hériter du défaut), renommer, supprimer.
- **Sélection automatique** (option globale) : nom du profil = nom de l'avion au tableau de score,
  symboles/espaces ignorés, casse ignorée, préfixe suffisant (`F14` couvre F-14A et F-14B ; `F14A`
  pour être spécifique). Suffixe `_2` pour le second siège (F-14 RIO, F-15E WSO, AH-64D front).
- Liste des noms attendus : voir `docs-agents/metier/aeronefs-supportes.md`.
