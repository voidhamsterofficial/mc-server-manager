# 🎈 Blockparty

A cute, fast, cross-platform **Minecraft server manager** — create, run, and manage
Minecraft servers from a friendly desktop app instead of a terminal window.

Built with a **Rust** backend (Tauri v2) and a **Svelte 5** UI. An open,
cross-platform alternative to tools like MC Server Soft.

## ✨ Features

- **Create servers in seconds** — pick a Minecraft version (fetched live from
  Mojang), set memory, accept the EULA, done. The official server jar is
  downloaded and SHA-1 verified for you.
- **Start / stop / kill** with graceful shutdown (`stop` first, force-kill
  after 30 s).
- **Live console** — streamed in batches from the Rust backend, virtualized
  rendering (smooth even with thousands of lines), color-coded log levels,
  command input.
- **Java auto-detection** — finds installed JVMs (PATH, `JAVA_HOME`, vendor
  directories) and picks the right major version for each Minecraft version.
- **Cute & bubbly UI** — pastel design, springy animations, wobbly status
  blobs, confetti when your server comes online. Dark mode included.
  Animations respect `prefers-reduced-motion`.

### Roadmap

- [x] Automatic Temurin JRE download when no suitable Java is installed
- [x] Online player list, kick/ban/op, whitelist editor, `server.properties` editor
- [x] Backups (manual + scheduled), per-server retention, and a task scheduler
- [x] Live CPU / memory / uptime dashboard
- [x] Installers for every major server type: Vanilla, Paper, Purpur, Folia,
  Spigot (BuildTools), Fabric, Quilt, Forge, NeoForge, Mohist, Arclight,
  Bedrock, Velocity, and BungeeCord
- [x] Port selection, per-server Java/JVM args/start command, server icons,
  and an MOTD editor with live preview

## 📦 Installing

Grab the installer for your platform from the
[releases page](https://github.com/Squ1ggly/mc-server-manager/releases)
(built automatically by CI when a version tag is pushed):

- **Windows**: run the `.exe` (NSIS) or `.msi` installer.
- **macOS**: open the `.dmg` and drag Blockparty to Applications. Builds are
  unsigned, so the first launch needs right-click → Open (or
  `xattr -dr com.apple.quarantine /Applications/blockparty.app`).
- **Linux**: either install the `.deb` (`sudo apt install ./blockparty_*.deb`)
  or `.rpm`, or use the portable `.AppImage`
  (`chmod +x Blockparty_*.AppImage && ./Blockparty_*.AppImage`).
  The deb/rpm pull in the WebKitGTK runtime automatically; the AppImage
  bundles everything.

Servers themselves run wherever the app runs — the same Java auto-download
works on all three platforms.

## 🚀 Developing

Prerequisites: [Rust](https://rustup.rs), [Node.js](https://nodejs.org), and the
[Tauri CLI](https://tauri.app) (`cargo install tauri-cli`). On Linux you also
need the WebKitGTK dev packages
([full list](https://tauri.app/start/prerequisites/#linux)):

```sh
sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev \
  librsvg2-dev libappindicator3-dev patchelf
```

Then:

```sh
cargo tauri dev    # installs deps, builds the UI, and launches the app
```

That's it — one command, no dev server, no open ports. The UI is loaded as
built static files exactly like in production. For a distributable build:

```sh
cargo tauri build  # bundles the platform installer (NSIS/MSI, dmg, deb/rpm/AppImage)
```

Installers land in `src-tauri/target/release/bundle/`. Each OS builds its own
format — Windows builds Windows installers, macOS builds dmg, Linux builds
deb/rpm/AppImage — which is why the release workflow runs on all three.

## 🗂 Project structure

```
├── src/            Svelte 5 frontend (index.html, views, components, stores)
├── src-tauri/      Rust backend (Tauri commands, process manager, installers)
│   └── src/
│       ├── servers.rs      server registry & config (persisted as JSON)
│       ├── process.rs      java child processes, console streaming, shutdown
│       ├── console.rs      pure log-line parsing (ready/join/leave detection)
│       ├── installers/     server-jar installers (vanilla via Mojang)
│       ├── java.rs         JVM detection & version mapping
│       └── commands.rs     thin Tauri command layer over the services
└── *.config.*      root-level tooling configs (Vite, TypeScript, Svelte)
```

Server data (registry, server directories, downloaded runtimes) lives in the
per-user app-data directory, not in the repo.

## 🧑‍💻 Development

- Rust changes recompile automatically under `cargo tauri dev`. After UI
  changes, re-run `cargo tauri dev` (the UI build takes well under a second).
- Checks that must pass: `cargo clippy`, `cargo fmt --check`, `cargo test`
  (in `src-tauri/`) and `npm run check` + `npm run build`.
- Coding standards live in [AGENTS.md](AGENTS.md).

## 🙏 Credits

- [Monocraft](https://github.com/IdreesInc/Monocraft) by Idrees Hassan — the
  Minecraft-inspired pixel font used for headings and the console (SIL OFL 1.1,
  contains no Mojang assets; license bundled in `src/assets/fonts/`).
- Player avatars rendered via [mc-heads.net](https://mc-heads.net).
- Blockparty contains no Minecraft game assets and is not affiliated with,
  endorsed by, or supported by Mojang or Microsoft.

## 📄 License & EULA

Blockparty downloads the official Minecraft server software from Mojang. You
must accept the [Minecraft EULA](https://aka.ms/MinecraftEULA) for each server
you create; the app records your acceptance in the server's `eula.txt`.
