# Snapshot du wiki GitHub (upstream)

Copie brute des pages du wiki officiel, récupérée le 2026-10-12.
Source : https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki

Le wiki est un dépôt git à part. Pour le rafraîchir :

```bash
git clone --depth 1 https://github.com/ciribob/DCS-SimpleRadioStandalone.wiki.git /tmp/srs-wiki
cp /tmp/srs-wiki/*.md docs-agents/wiki-upstream/
```

Ces pages sont écrites pour les **utilisateurs** (pilotes, admins serveur). La synthèse
orientée développeur se trouve dans `docs-agents/metier/`. En cas de doute, le code fait foi,
puis le wiki (certaines pages sont marquées WIP et parfois obsolètes).

| Page | Contenu |
|------|---------|
| `Home.md` | Présentation, installation, utilisation en vol, **chiffrement par avion**, troubleshooting |
| `General.md` | Écran principal du client (micro, sorties, serveur, EAM, presets) |
| `Settings-(WIP).md` | Toutes les options client (globales et par profil) |
| `SRS-Server-(WIP).md` | Options serveur (27 réglages), auto-connect, démarrage PowerShell |
| `Presets.md` | Fichiers de channelisation `.txt`, `awacs-radios.json`, presets côté serveur |
| `Per-Aircraft-Profiles-(WIP).md` | Profils auto par aéronef, noms attendus |
| `Development.md` | Stratégie de branches master/beta/release |
| `Client-familiarization-(WIP).md`, `Controls-(WIP).md`, `Favorites-(WIP).md`, `Resources.md` | Courtes, captures d'écran, liens |
