# SRS client in Rust (`client-rust/`)

A rewrite of `DCS-SR-Client` without .NET: one static `SR-ClientRadio.exe`, no runtime, no DLL,
wire-compatible with existing SRS servers and with the Lua/DCS side. The C# client stays in the
repository as the behavioural reference.

Project rules, layout, decisions and milestone status: `../docs-agents/blocs/client-rust.md`.

## Build (native)

Requirements: stable Rust (see `rust-toolchain.toml`) and a C compiler. libopus, speexdsp and the
WebRTC VAD are vendored and compiled with `cc` (no cmake, no system libraries).

```sh
cargo test --workspace          # all crates, unit + golden tests
cargo build --release -p srs-app
```

Linux: `apt install libasound2-dev libudev-dev` (audio devices / input, from M2 on).

## Build for Windows from macOS / Linux

Two options. Both produce a fully static exe (`+crt-static` is set in `.cargo/config.toml`).

### 1. MSVC target with `cargo-xwin` (preferred, matches CI)

```sh
brew install llvm lld               # clang-cl / lld-link
cargo install cargo-xwin
rustup target add x86_64-pc-windows-msvc
cargo xwin build --release --target x86_64-pc-windows-msvc -p srs-app
# -> target/x86_64-pc-windows-msvc/release/SR-ClientRadio.exe
```

The first run downloads the Windows SDK and CRT headers/libs into `~/.cache/cargo-xwin`.

### 2. GNU target with mingw-w64 (fallback)

```sh
brew install mingw-w64
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu -p srs-app
```

## Testing on the Windows PC (what the Mac cannot verify)

```sh
client-rust/tools/package.sh --build     # -> client-rust/dist/SRS-Client-Rust-<version>.zip
```

Copy the zip to the PC, unzip, and:

1. Run `SR-ClientRadio.exe` from the folder (double-click). The window must open, the *Log* tab shows
   `Input: N device(s)` (joysticks, keyboard, mouse found through DirectInput).
2. Enter the server address, *Connect*. TCP and VoIP dots turn green. *Clients* lists the other players.
3. *External AWACS Mode* with a password → the *Radios* tab fills; tune radio 1 to a frequency where
   someone talks: you must hear them. Hold the *PTT* button (or Space) and talk: they must hear you.
4. *Controls* tab → *Set* on `Ptt`, press the HOTAS button: it must be stored and work.
   Profiles written by the C# client (`default.cfg` next to the exe) are read as is.
5. Launch DCS with SRS installed; the client receives the export (DCS dot green, radios follow the
   cockpit) and the in-game overlay shows TX/RX. Using the `-host=` launch from `lua-srs.dll`: point
   `SRPathStandalone` to a folder whose `Client\` contains this exe.
6. Audio effects (M5), A/B against the C# client on the same server: a transmission on UHF must carry
   the radio "colour" (band-passed voice, faint white noise), FM must add the NATO tone, an encrypted
   radio must sound CVSD-like and play the KY-58 tones, PTT press/release must play the click WAVs,
   and `AmbientCockpitNoiseEffect` must loop the aircraft ambient under received voice. The profile
   keys `RadioEffectsRatio`, `RadioBackgroundNoiseEffect`, `NATOTone`, `RadioRxEffects_*`,
   `RadioTxEffects_*` are honoured as in the C# client.

7. UI (M6): the *Radio overlay* and *AWACS overlay* toggles open always-on-top transparent windows
   that must stay above DCS in windowed/borderless mode; drag to move, resize from the corner, position
   and opacity are remembered. Frequency buttons, right-click for guard, channel dropdown, encryption
   key and RT must behave like the C# overlay. *Settings* → profile Create/Copy/Rename/Delete writes
   `<name>.cfg` next to `global.cfg`; *Favourites* writes `FavouriteServers.csv`. Microphone / speaker
   choice uses the same device ids as the C# client, so an existing `global.cfg` keeps its devices.
   Starting the exe twice must show the "already running" box unless `-allowMultiple`.

Report what fails with the *Log* tab content or `clientlog.txt` (written next to the exe; `RUST_LOG=debug`
for more detail).

## Running

Copy `SR-ClientRadio.exe` into the `Client/` folder of an existing SRS installation (next to
`global.cfg`, `AudioEffects/`, `RadioModels/`). It accepts the same arguments as the C# client:
`-host=ip[:port]`, `-cfg=<dir>`, `-allowMultiple`, `-lang:<culture>`.

Logs go to stderr (`RUST_LOG=debug` for more).

## Layout

| Crate | Role |
|---|---|
| `srs-protocol` | TCP JSON messages, UDP voice packet, ShortGuid, "who can hear" rule, radio maths, server settings. Pure. |
| `srs-settings` | `global.cfg` and `<profile>.cfg`, SharpConfig-compatible. |
| `srs-net` | TCP control session (tokio); UDP voice from M2. |
| `srs-audio` | Opus, Speex preprocessor, WebRTC VAD (static). Later: jitter buffer, effects, mixing, devices. |
| `srs-opus-sys` | Vendored libopus 1.5.2 (float, portable C), built with `cc`. |
| `speexdsp-preprocess-sys` | Vendored speexdsp C sources, built with `cc`. |
| `srs-app` | The `SR-ClientRadio` binary (egui UI). |

## Tests against a real server

`ServerCommandLine` from the C# tree runs on Linux; the Docker recipe in `tools/docker/server`
builds it so the Rust client can be exercised on a Mac (see its README). Then:

```sh
export SRS_TEST_SERVER=127.0.0.1:5002
cargo test -p srs-net    --test real_server        -- --ignored   # SYNC, EAM, UDP ping
cargo test -p srs-client --test echo_real_server   -- --ignored   # RX chain via the server echo frequency
cargo test -p srs-client --test tx_echo_real_server -- --ignored  # encoder + PTT + echo + RX
```

Tests with DCS, a HOTAS or the C# client need a Windows machine.
