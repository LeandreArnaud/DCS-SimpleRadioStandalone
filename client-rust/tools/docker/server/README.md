# Reference server in Docker

Builds `ServerCommandLine` (C#) from this repository and runs it on port 5002.
Used to test the Rust client on a machine without .NET.

```sh
# from the repository root
docker compose -f client-rust/tools/docker/server/compose.yml up --build
# connected clients: client-rust/tools/docker/server/data/clients-list.json
```

EAM passwords: `blue` / `red`. Test (echo) frequencies: 247.2 and 120.3 MHz.
