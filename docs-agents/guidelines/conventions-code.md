# Conventions de code

## Général
- Pas d'`.editorconfig`, pas de `Directory.Build.props`, pas de formateur C#/XAML en CI. Suivre le
  style du fichier voisin.
- **Fins de ligne CRLF** et **BOM UTF-8** pour `.cs`, `.xaml`, `.xml`, `.lua`, `.sln`, `.c/.h/.cpp`
  (`.gitattributes`). Conserver quand on édite depuis macOS/Linux. `publish.ps1` est en LF.
- Commentaires et identifiants en **anglais**. Messages de commit courts, impératif, anglais.
- Ne pas déplacer ni renommer les fichiers parasites connus (`Server/Server_s3vspq0t_wpftmp.csproj`,
  `DCS-SimpleRadioStandalone.VC.db`, `DCS-SR-ExternalAudio/README.md`) sans demande.

## C#
- 4 espaces. Namespaces **file-scoped** (`namespace X.Y;`) dans le code récent ; les anciens fichiers
  (Installer, ExternalAudio `Program.cs`, LAME) sont en bloc : ne pas convertir en masse.
- `var` partout, `PascalCase` public, `_camelCase` privé (pas strict), `UPPER_SNAKE` pour les
  constantes « protocole » (`MINIMUM_PROTOCOL_VERSION`, `MIC_SAMPLE_RATE`) et les clés serveur.
- Modèles réseau : **champs publics minuscules** (`freq`, `enc`, `radios`) alignés sur le JSON Lua ;
  classes UI : propriétés PascalCase. D'où `IncludeFields = true` dans tout JSON.
- Nullable : désactivé dans Common/Client/Server (ne pas ajouter de `?` partout), activé dans
  SharedAudio, DCS-SR-Lua, Mobile. `ImplicitUsings` seulement ServerCommandLine, SharedAudio, Mobile.
- C# 13/14 autorisé (SDK .NET 10) : `Lock`, `field`, collection expressions, `GeneratedRegex`.
- Logging NLog : `private static readonly Logger Logger = LogManager.GetCurrentClassLogger();`,
  `Logger.Error(ex, "message")`, Debug/Trace pour le détail par message.
- Singletons : propriété `Instance` double-checked lock avec `private static readonly object _lock`.
- Messaging : POCO + `IHandle<T>.HandleAsync(T, CancellationToken)` + `EventBus.Instance` (client)
  ou `IEventAggregator` injecté (serveur). Se désabonner pour les objets transitoires.
- WPF client : Fody notifie les auto-propriétés ; `NotifyPropertyChanged(nameof(X))` pour le calculé ;
  mutations UI via `Application.Current.Dispatcher` ; collections avec
  `BindingOperations.EnableCollectionSynchronization`.
- Serveur WPF : conventions Caliburn (`x:Name` = méthode/propriété du VM).
- Chemins audio chauds : pas d'allocation par trame (`ArrayPool`, `Span`), relire les settings
  périodiquement (3 s) plutôt qu'à chaque appel.
- Strings UI dans `Properties/Resources.resx` (+ `.zh.resx`) ; XAML `{x:Static p:Resources.Key}`.
- Enums de settings : les **noms** sont les clés INI → n'ajouter qu'à la fin, ne jamais renommer.
  `MessageType` sérialisé en int → n'ajouter qu'à la fin.

## Lua (`.stylua.toml`)
Lua 5.1, 120 colonnes, 4 espaces, **CRLF**, guillemets doubles préférés, `call_parentheses = Always`,
pas de collapse. `Export.lua` ignoré. CI StyLua 2.1.0 (workflow actuellement inopérant, formater à la
main : `stylua Scripts/`). Exporteur `exportRadio<Unit>(_data, SR)`, état privé dans une table
locale, locaux préfixés `_`, `log.write("SRS-export", log.INFO, ...)`. Littéral `VersionPrefix`
pour la version.

## XAML
Pas de formateur. Suivre l'indentation du fichier. MahApps dans le client (`MetroWindow`,
`ToggleSwitch`), ressources dans `Themes/Styles.xaml`.

## Tests
MSTest 4 (`[TestClass]`, `[TestMethod]`, `Assert.HasCount`…), projet `DCS-SR-CommonTests`. Toute
modification du protocole (`UDPVoicePacket`, `NetworkMessage`) ou d'un helper pur mérite un test.
