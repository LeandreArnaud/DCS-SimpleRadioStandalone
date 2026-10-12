# Bloc : `DCS-SR-ExternalAudio` (TTS / MP3 vers une fréquence)

Outil console Windows (`DCS-SR-ExternalAudio.exe`, `net10.0-windows`, `UseWindowsForms`) qui se
connecte au serveur SRS comme un **faux client** et émet un fichier audio ou une synthèse vocale sur
des fréquences. C'est ce qu'appellent les scripts de mission (`STTS.TextToSpeech(...)`,
`STTS.PlayMP3(...)` dans `docs/Examples.txt`, DCS-gRPC, MOOSE…). Le script Lua STTS n'est pas dans
ce dépôt.

## Fichiers
- `Client/Program.cs` : `Main`, NLog console, `Parser.Default.ParseArguments<Options>` ; en cas
  d'erreur liste les voix Windows installées. Classe `Options` = toutes les options.
- `Client/ExternalAudioClient.cs` : construit `PlayerRadioInfoBase` (radio[1] = première fréquence),
  `SRClientBase` (ShortGuid neuf), `TCPClientHandler.TryConnect(loopback:port)` ; sur connexion crée
  `UDPVoiceHandler`, puis `SendAudioAsync` : une trame Opus de 40 ms par tick de timer 40 ms →
  `UDPVoicePacket` (`Encryptions` à 0, `PacketNumber++`) → déconnexion à la fin.
- `Audio/AudioGenerator.cs` : sources → PCM 16 kHz mono 16 bit → `OpusEncoder.Create(16000, 1, Voip)`
  par blocs de 1280 octets (dernier bloc complété de zéros). MP3 via `Mp3FileReader`, OGG via
  `NAudio.Vorbis`, resample `Audio/EventDrivenResampler.cs` (taux < 16 kHz → `Exit(1)`).
- TTS : `GoogleTTS` (`--googleCredentials`, Linear16 16 kHz, voix par nom ou langue+genre), `AzureTTS`
  (`--azureCredentials "key;region"`, **NRE si `--voice` absent**), sinon `LocalTTS`
  (`System.Speech.Synthesis`).
- `README.md` du projet = README de NAudio.Vorbis (fichier parasite). `FodyWeavers.xml` orphelin.

## Options (groupes exclusifs `file` / `TTS` / `textFile`)
`-i/--file` (mp3/ogg), `-t/--text` (+ `-z/--ssml` Google), `-I/--textFile`, `-f/--freqs` (MHz,
virgules, requis), `-m/--modulations` (`AM,FM`, requis ; un écart de nombre affiche « QUITTING! » mais
**continue**), `-c/--coalition` (0/1/2, requis), `-p/--port` (5002 ; hôte toujours loopback,
`--IP` commenté), `-n/--name` (`DCS-STTS`), `-v/--volume`, `-s/--speed` (TTS local), `-l/--culture`
(`en-GB`), `-g/--gender`, `-V/--voice`, `-G/--googleCredentials`, `-a/--azureCredentials`,
`-L/-O/-A` latitude/longitude/altitude, `-R/--Record` (true), `--unitId` (1000 ; égal à l'unitId
d'un avion pour parler sur son intercom), `--ambient`, `--ambientVolume`, `-h/--minimise`.

## Étendre
Nouvelle option → `Program.Options` ; nouvelle source → méthode dans `AudioGenerator` renvoyant du
PCM 16 kHz mono et branche dans `GetOpusBytes()`. Le réseau est entièrement dans `Common`. Hôte
distant : remplacer `IPAddress.Loopback` dans le ctor d'`ExternalAudioClient`.
