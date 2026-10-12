# CLAUDE.md

Lis d'abord `AGENT.md` à la racine : il contient les règles du projet, la carte du dépôt et le
sommaire de la documentation technique et métier (`docs-agents/`). Applique-le intégralement.

Rappels critiques (détails dans `AGENT.md`) :
- Aucun `git add` / `commit` / `stash` / `push` sans demande explicite ; laisser les changements non stagés.
- La logique vit dans `Common/` ; client et serveurs sont des hôtes.
- Protocole et enums de settings : ajouter à la fin, ne jamais réordonner ni renommer.
- Fichiers `.cs`, `.xaml`, `.lua` en CRLF + BOM UTF-8 ; pas de reformatage massif.
- Build/tests complets = Windows + .NET 10 ; dire clairement ce qui n'a pas pu être vérifié.
- Mettre à jour `docs-agents/` quand une feature change un comportement documenté.
