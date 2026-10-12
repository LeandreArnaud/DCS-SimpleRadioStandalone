# Git, branches, CI et release

## Règle absolue pour l'agent

**Ne jamais lancer `git add`, `git commit`, `git stash`, `git push`** ni toute commande qui modifie
l'index ou l'historique, sauf demande explicite dans le message de l'utilisateur. Une feature finie =
modifications **non stagées** dans le working tree. Plusieurs sessions agent tournent en parallèle sur
le même repo ; un `git add` automatique mélange leurs changements. Lecture seule libre : `git status`,
`git diff`, `git log`, `git show`.

## Branches (wiki `Development.md`)

| Branche | Rôle |
|---------|------|
| `master` | Développement principal. |
| `beta` | Tampon pour changements risqués ; tags possibles ; stabilisé puis reversé dans `release`. |
| `release` | Version courante ; correctifs seulement. |

Sens des merges : `master → beta/release`, `beta → release`, `release → master`. Tags depuis
`release` ou `beta`.

## Versions

Format `X.Y.Z.W` (ex. `2.4.1.0`). La version est **pilotée par le tag** : `release.yml` passe
`-VersionPrefix "<tag>"` à `publish.ps1`, qui l'injecte dans les assemblies (`-p:VersionPrefix`)
et remplace le texte `VersionPrefix` dans les `.lua` copiés. Le défaut de `publish.ps1`
(`$VersionPrefix="2.4.1.0"`) ne sert qu'en build local.

## CI GitHub Actions (`.github/workflows/`)

| Workflow | Déclencheur | Étapes |
|----------|-------------|--------|
| `build.yml` | push, PR, manuel | `dotnet restore --locked-mode` → `dotnet test` (Release, win-x64) ; `publish.ps1 -NoSign` → artefact zip ; puis signature test (SignPath) et scan VirusTotal hors PR. |
| `release.yml` | tag `[0-9].[0-9].[0-9].[0-9]` | publish avec VersionPrefix = tag → signature release → scan → **draft release** GitHub avec `SRS-AutoUpdater.exe`, le zip et `SRS-Server-Commandline-Linux`, notes depuis `release-notes.template.md`. |
| `formatter.yml` | PR | StyLua 2.1.0 sur `Scripts/` puis commit auto. Pas encore de formateur C#/XAML. |
| `signing.yml`, `virustotal-scan.yml` | réutilisables | Signature via SignPath, scan via vt-cli. |

Runner Windows 2025, .NET 10. Le cache NuGet dépend de `**/packages.lock.json` : toute modification
de package doit régénérer les lock files (`dotnet restore --force-evaluate`), sinon `--locked-mode`
échoue en CI.

## Release notes

Modèle `release-notes.template.md` : liens, instructions d'installation, section Changelog à
remplir, overlay in-game (raccourci par défaut Left Ctrl + Left Shift + Esc), hosting (port 5002
TCP+UDP, `banned.txt`).

## Conventions de commit observées

Messages courts à l'impératif, en anglais, souvent suffixés du numéro de PR `(#1033)`. Pas de
convention « conventional commits ».
