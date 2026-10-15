# docs-agents — index détaillé

Documentation destinée aux agents IA (et aux humains pressés). Le point d'entrée court est
`AGENT.md` à la racine. Ici, chaque fichier avec ce qu'il contient.

## architecture/ — comment le système tient ensemble
- `vue-d-ensemble.md` : composants, schéma, flux (état radio, voix, connexion, auto-connect), carte « je veux toucher X → fichier ».
- `protocole-reseau.md` : TCP JSON (`MessageType`, handshake, throttling), UDP voix (layout octet par octet, ping, routage serveur, règle « peut entendre », règles client), autres canaux, compatibilité.
- `modeles-donnees.md` : `SRClientBase`, `RadioBase`, enums, constantes, **toutes les clés de settings** (serveur/global/profil) avec défauts, `InputBinding`, fichiers de données.

## blocs/ — un document par partie du code
- `common.md` : carte de `Common/`, architecture (pas de DI, deux bus), normalisation, invariants.
- `client-audio.md` : chaînes TX et RX pas à pas, effets, modèles de radio, constantes, pièges.
- `client-ui-input-settings.md` : démarrage, MVVM maison, fenêtres/overlays, singletons, DirectInput, settings/profils/favoris/presets, utils.
- `client-network-dcs.md` : `TCPClientHandler`, EAM, tous les listeners UDP DCS/LotATC/VAICOM, multi-équipage, messages du bus.
- `serveur.md` : hôtes GUI/CLI, démarrage, TCP, UDP, GUI Caliburn, Linux, pièges.
- `lua-dcs-integration.md` : scripts Lua, déploiement, ports, payload export, helpers `SR`, LOS, GameGUI, overlay in-game, `lua-srs.dll`, plugins, conventions, pièges.
- `external-audio.md` : outil TTS/MP3, options, pipeline, extension.
- `installer-autoupdater.md` : Installer (registre, scripts, args), AutoUpdater (GitHub), SharedAudio, Mobile (abandonné).
- `client-rust.md` : client Rust (`client-rust/`) : règles de compatibilité, layout des crates, décisions techniques, bizarreries C# reproduites, état des jalons.

## metier/ — ce que le logiciel doit faire (issu du wiki + code)
- `glossaire.md` : vocabulaire SRS/DCS.
- `fonctionnalites-radio.md` : modèle d'une radio, qui entend qui, TX, RX, contrôle, EAM, multi-équipage, IFF.
- `parametres-serveur.md` : les 27 options GUI, auto-connect, headless.
- `parametres-client.md` : écran General, options globales, de profil, bindings, favoris.
- `presets-et-profils.md` : channelisation `.txt`, presets serveur, `awacs-radios.json`, profils auto.
- `aeronefs-supportes.md` : modules présents, chiffrement par avion, noms de profils.

## guidelines/ — comment travailler
- `conventions-code.md` : CRLF/BOM, C#, Lua (StyLua), XAML, tests.
- `build-test-run.md` : prérequis, commandes, lock files, `publish.ps1`, exécution manuelle, versions.
- `git-et-release.md` : **interdiction de commit automatique**, branches, CI, release, versions.
- `recettes.md` : 12 recettes pas à pas (option serveur/client, binding, message TCP, champ propagé, routage, effet audio, modèle radio, avion, commande UDP, fenêtre, capture).
- `pieges-et-dette.md` : bugs repérés, incohérences, fichiers morts, zones fragiles.

## wiki-upstream/ — snapshot brut du wiki GitHub (utilisateurs)
Voir `wiki-upstream/README.md` pour la commande de rafraîchissement.

## Maintenance de cette documentation
- Quand une feature change un comportement décrit ici, **mettre à jour le fichier concerné** dans le
  même changement (surtout `modeles-donnees.md` pour une clé de setting, `aeronefs-supportes.md`
  pour un avion, `pieges-et-dette.md` quand un bug listé est corrigé).
- Rester factuel et court : chemins de fichiers, noms de classes, valeurs. Pas de prose.
- Le code fait foi sur la doc ; la doc fait foi sur le wiki.
