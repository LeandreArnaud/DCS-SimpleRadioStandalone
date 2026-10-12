# Bloc : Installer, AutoUpdater, SharedAudio, Mobile

## Installer (`Installer/MainWindow.xaml.cs`, ~1280 lignes, WPF MahApps, aucune dépendance à Common)
- Pré-check `CheckExtracted()` : exige `Client\opus.dll`, `Client\speexdsp.dll`,
  `Client\awacs-radios.json`, `Client\SR-ClientRadio.exe`,
  `Scripts\DCS-SRS\Scripts\DCS-SimpleRadioStandalone.lua`, `Server\SRS-Server.exe` à côté.
- Registre `HKCU\SOFTWARE\DCS-SR-Standalone` : `SRPathStandalone` (dossier programme), `ScriptsPath`
  (racine Saved Games). Lu aussi par `lua-srs.dll` pour lancer le client.
- Détection DCS `FindValidDCSFolders` : sous-dossiers de `Saved Games` contenant `DCS`/`MCS` avec
  `config\network.vault` ou `config\options.lua`, hors `SERVER`/`DEDICATED`.
- Install : tue `sr-server*`, `srs-server*`, `sr-client*` ; refuse si DCS tourne (scripts). Par dossier
  DCS : retire les anciens scripts puis `InstallScripts()` (ligne Export.lua, hook,
  `Mods\Services\DCS-SRS`). `InstallProgram()` : nettoie, copie `Installer.exe`, `SRS-AutoUpdater.exe`,
  `Examples.txt`, `Readme.txt`, dossiers `Server`, `ServerCommandLine-Windows/-Linux`, `Client`,
  `ExternalAudio`, `Scripts` ; migre `server.cfg` → `Server\`, `*.cfg/*.txt/*.json/*.csv/Recordings`
  → `Client\` ; écrit le registre ; raccourci menu Démarrer optionnel (`ShortcutHelper.cs`) ;
  `VC_redist.x64.exe /install /norestart /quiet`.
- Désinstall `UninstallSR` : ligne Export.lua, hook, `Mods\Services\DCS-SRS`, registre, raccourci.
  Ne supprime **pas** le dossier programme.
- Args : `-autoupdate` (mode silencieux, demande seulement « changer les chemins ? »), `-server`
  (+ `-path=<dir>`, scripts décochés), `-restart` (relance `Server\srs-server.exe`). Pas de `-silent`.
- Log `installer-log.txt`. Resx en + zh.

## AutoUpdater (`AutoUpdater/MainWindow.xaml.cs`, WPF, dépend de Common pour `GitHubUpdater`)
- Tue client/serveur, instance unique, `DownloadLatestVersion()`.
- `GitHubUpdater.GetAllReleasesAsync()` (Octokit, 3 retries rate-limit). Stable = dernière non
  prerelease ; beta/alpha = prerelease dont le **nom** contient `beta`/`alpha`. Flags `-beta`,
  `-alpha`. Version = `new Version(TagName.Replace("v",""))`.
- Asset = premier dont le nom commence par `dcs-simpleradiostandalone` et contient `.zip`.
- `-server -path=<dir>` : compare `FileVersionInfo` de `Server\SRS-Server.exe`, saute si à jour.
- Téléchargement `WebClient` (UA navigateur, détecteur de blocage 10 s), dézippe dans `extract\`,
  attend la fermeture de DCS (client), lance `extract\installer.exe -autoupdate [-server -path=…
  [-restart]]`. `-wait` attend la fin de l'installeur.
- Lancé par `UpdaterChecker.LaunchUpdater(beta)` (cherche `SRS-AutoUpdater.exe` dans `..\` ou `.`,
  `runas` si besoin). `CheckForUpdateAsync` renvoie « à jour » en DEBUG.

## SharedAudio
Bibliothèque net10.0 **sans code** : copie `opus.dll/.so`, `libmp3lame.dll/.so` en sortie
(`CopyToOutputDirectory=Always`). Référencée par Client, Server, ServerCommandLine, ExternalAudio.
Le P/Invoke est dans `Common`. `publish.ps1` supprime ensuite les `.so` des sorties Windows et les
`.dll` de la sortie Linux. `speexdsp.dll` vient de `DCS-SR-Client/speexdsp.dll` (Content).

## DCS-SRS-Mobile
.NET MAUI **Android seulement** (`net8.0-android`, min SDK 30), **hors .sln**, hors CI, hors
`publish.ps1`, 2 commits (2025-05-03, « non fully working app »). Client EAM mobile : connexion
IP:port, radios, presets, service Android premier plan, réutilise `TCPClientHandler`,
`UDPVoiceHandler`, `ClientAudioProvider`, `OpusEncoder` de Common ; duplique les modèles DCS dans
`Models/DCS/`. Natifs `libopus.so`, `libspeex*.so` dans `Resources/lib/`. `$(MauiVersion)` non
défini dans le repo. **Prototype abandonné : ne pas y investir sans demande explicite.**
