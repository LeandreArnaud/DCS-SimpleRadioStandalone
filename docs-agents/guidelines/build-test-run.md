# Build, tests, exécution

## Prérequis
- **Windows 10/11 x64 + .NET 10 SDK** (`global.json` : 10.0.0, rollForward latestMinor) pour tout
  builder (WPF, WinForms, AOT). Visual Studio 2022+ ou Rider pour le XAML.
- Sur **macOS/Linux** : seuls `Common`, `SharedAudio`, `ServerCommandLine` compilent (net10.0 pur).
  Les tests ciblent `net10.0-windows` → Windows seulement. Sur ce Mac, `dotnet` n'était **pas
  installé** au 2026-10-12.
- signtool (Windows Kits 10.0.26100) uniquement pour un publish signé local.

## Commandes

```bash
# Restaurer (lock files obligatoires, comme la CI)
dotnet restore --locked-mode

# Build complet (Windows)
dotnet build DCS-SimpleRadioStandalone.sln -c Release

# Build cross-platform possible
dotnet build Common/Common.csproj
dotnet build ServerCommandLine/ServerCommandLine.csproj -r linux-x64

# Tests (Windows, comme la CI)
dotnet test --configuration Release --runtime win-x64 --no-restore

# Serveur headless en dev
dotnet run --project ServerCommandLine -- --port 5002 --cfg server.cfg

# Publish complet (Windows PowerShell)
./publish.ps1 -NoSign            # dossier install-build/
./publish.ps1 -NoSign -Zip -VersionPrefix 2.4.2.0
```

## Lock files NuGet
Chaque projet (sauf Mobile) a un `packages.lock.json`. Après tout changement de `PackageReference` :

```bash
dotnet restore --force-evaluate
```

et inclure les lock files modifiés, sinon `--locked-mode` échoue en CI (NU1004) et le cache est faux.

## `publish.ps1` (résumé)
Params `-NoSign`, `-Zip`, `-NoRedist`, `-outputPath` (`.\install-build`), `-VersionPrefix`
(défaut `2.4.1.0`, remplacé par le tag en release). Étapes : restore locked → vide la sortie →
télécharge `vc_redist.x64.exe` → copie `Scripts/` et remplace `VersionPrefix` dans les `.lua` →
copie `docs/Readme.txt`, `Examples.txt` → `dotnet clean` → publish par projet :

| Projet | Sortie | Mode |
|--------|--------|------|
| Client | `/Client` | framework-dependent win-x64, single file, R2R |
| Server | `/Server` | idem |
| ServerCommandLine | `/ServerCommandLine-Windows`, `/ServerCommandLine-Linux` | self-contained |
| ExternalAudio | `/ExternalAudio` | framework-dependent |
| AutoUpdater, Installer | racine | framework-dependent |
| DCS-SR-Lua | `/Scripts/DCS-SRS/bin` | NativeAOT win-x64 |

Puis nettoyage (`runtimes/`, `.so`/`.dll` selon OS, `lua-srs.pdb`), signature (`*.exe` +
`lua-srs.dll`), zip `DCS-SimpleRadioStandalone-<version>.zip`.

## Exécuter / tester à la main
- Client : lancer `SR-ClientRadio.exe` (ou `dotnet run --project DCS-SR-Client`), un serveur local
  `SRS-Server.exe`, se connecter sur `127.0.0.1:5002`. Sans DCS, utiliser **EAM** (mot de passe
  coalition défini sur le serveur) pour avoir des radios.
- Deux clients sur une machine : `-allowMultiple` et `-cfg=<dossier>` distincts ; les ports UDP DCS
  (9084…) ne peuvent être écoutés que par un seul client.
- Test de fréquence : parler sur `TEST_FREQUENCIES` (247.2 / 120.3 AM) renvoie l'écho.
- Logs : `clientlog.txt`, `serverlog.txt` à côté des exes ; côté DCS `Saved Games/DCS/Logs/dcs.log`
  (tags `SRS-export`, `SRS-GameGUI`).
- Scripts Lua : copier `Scripts/DCS-SRS` dans `Saved Games/DCS/Mods/Services/DCS-SRS` et le hook dans
  `Scripts/Hooks`, ou relancer l'installeur. Formater avec `stylua Scripts/`.

## Versions
Aucune version dans les csproj. Injectée par `-p:VersionPrefix` au publish ; build dev = `1.0.0.0`
(un serveur dev peut être refusé par un client qui vérifie `MINIMUM_PROTOCOL_VERSION`). Pour une
release : tag `X.Y.Z.W` (chiffres **simples**, le glob du workflow ne matche pas `2.4.10.0`).
