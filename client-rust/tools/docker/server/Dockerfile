# Reference SRS server (the C# ServerCommandLine) for testing the Rust client
# on a Mac/Linux box without the .NET SDK installed locally.
#
# Build context must be the repository root:
#   docker build --platform linux/amd64 -f client-rust/tools/docker/server/Dockerfile -t srs-server .
#
# x64 only: Common.csproj restricts RuntimeIdentifiers to win-x64;linux-x64.
# On Apple Silicon this runs under Rosetta/QEMU emulation (slow build, fine at runtime).

FROM --platform=linux/amd64 mcr.microsoft.com/dotnet/sdk:10.0 AS build
WORKDIR /src
COPY global.json Directory.Build.props* nuget.config* ./
COPY Common/ Common/
COPY SharedAudio/ SharedAudio/
COPY ServerCommandLine/ ServerCommandLine/
# Restore for every RID the projects declare (win-x64;linux-x64): the lock file
# was generated that way and locked mode refuses a narrower RID set.
RUN dotnet restore ServerCommandLine/ServerCommandLine.csproj --locked-mode
RUN dotnet publish ServerCommandLine/ServerCommandLine.csproj \
      --configuration Release --runtime linux-x64 --self-contained \
      -p:VersionPrefix=2.4.1.0 -p:IncludeSourceRevisionInInformationalVersion=false \
      -p:PublishSingleFile=true -p:DebugType=None -p:DebugSymbols=false \
      --no-restore --output /out

FROM --platform=linux/amd64 mcr.microsoft.com/dotnet/runtime-deps:10.0
WORKDIR /srs
COPY --from=build /out/ /srs/
EXPOSE 5002/tcp 5002/udp
ENTRYPOINT ["/srs/SRS-Server-Commandline"]
