# 🎈 Blockparty

A cute, fast, cross-platform **Minecraft server manager** — create, run, and manage
Minecraft servers from a friendly desktop app instead of a terminal window.

Built with a **Rust** backend (Tauri v2) and a **Svelte 5** UI. An open,
cross-platform alternative to tools like MC Server Soft.

## ✨ Features

- **Create servers in seconds** — pick a Minecraft version (fetched live from
  Mojang), set memory, accept the EULA, done. Downloads are checksum-verified,
  including the Forge/NeoForge/Quilt installers that are executed to set up.
- **Start / stop / kill** with graceful shutdown (`stop` first, force-kill
  after 30 s). Orphaned server processes are reclaimed automatically.
- **Live console** — streamed in batches from the Rust backend, smooth with a
  5000-line buffer, Minecraft/ANSI colors, quick-command menu, right-click to
  copy.
- **Plugin manager** — browse and install plugins straight from Modrinth,
  filtered to your server's software and Minecraft version and verified against
  their published checksums, then enable, disable, or remove them. Available for
  the Paper family, the hybrids, and the proxies.
- **Players & moderation** — live player list, player history pages (playtime,
  chat log, game mode), and kick/ban with an optional recorded reason.
- **Java auto-detection** — finds installed JVMs (PATH, `JAVA_HOME`, vendor
  directories) and picks the right major version for each Minecraft version.
- **Minecraft-flavoured UI** — blocky beveled buttons, a pixel font, and the
  game's own palette, with colour-based hover/press feedback so nothing jumps
  around under the cursor. Dark mode included; motion respects
  `prefers-reduced-motion`.

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
- [x] Player pages (playtime, chat history, moderation), a scoped file
  browser/editor, a copyable LAN address, a right-click context menu, and
  in-app docs
- [x] Settings stored as human-readable YAML (per server, and a global file
  beside the app) — nothing in the registry
- [x] Plugin manager: install from Modrinth, enable/disable/remove, for the
  Paper family (Paper, Purpur, Spigot, Folia), the hybrids (Mohist, Arclight),
  and the Velocity/BungeeCord proxies

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
│       ├── servers.rs      server registry & config (persisted as YAML)
│       ├── process.rs      java child processes, console streaming, shutdown
│       ├── console.rs      pure log-line parsing (ready/join/leave/chat)
│       ├── installers/     server installers for all 14 server types
│       ├── plugins.rs      plugin folder management + Modrinth install
│       ├── roster.rs       player history (playtime, chat, bans)
│       ├── backups.rs      zip backups, restore, retention
│       ├── files.rs        path-safe file browser scoped to a server
│       ├── scheduler.rs    cron-style scheduled tasks
│       ├── java/           JVM detection, version mapping, Temurin download
│       └── commands.rs     thin Tauri command layer over the services
└── *.config.*      root-level tooling configs (Vite, TypeScript, Svelte)
```

Each server lives in its own folder (`Documents/Blockparty Servers` by default,
configurable) with a `blockparty-server.yaml` holding its settings. Global
settings sit in a `blockparty.yaml` beside the binary — nothing in the registry.
Downloaded Java runtimes live in the per-user app-data directory.

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
