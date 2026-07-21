# 🎮 ServerForge

> **The friendliest Minecraft server manager.** Create, run, and manage servers from a beautiful desktop app—no terminal required.

<div align="center">

![Rust](https://img.shields.io/badge/rust-%23CE422B.svg?style=for-the-badge&logo=rust&logoColor=white)
![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?style=for-the-badge&logo=typescript&logoColor=white)
![Tauri](https://img.shields.io/badge/tauri-%2324C8DB.svg?style=for-the-badge&logo=tauri&logoColor=white)
![Svelte](https://img.shields.io/badge/svelte-%23f1413d.svg?style=for-the-badge&logo=svelte&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)

**Cross-platform** • **Open source** • **Free** • **Zero registry cruft**

[⬇️ Download](#-download) • [🚀 Quick Start](#-quick-start) • [🧑‍💻 Contribute](#-contributing) • [📖 Docs](https://github.com/voidhamsterofficial/mc-server-manager/wiki)

</div>

---

Built with **Rust** (Tauri v2 backend) and **Svelte 5** (TypeScript UI). A modern, open alternative to MC Server Soft.

## ✨ Features at a Glance

<table>
<tr>
<td width="50%">

#### 🖱️ **Dead Simple**
- Create servers in **seconds**—pick version, set memory, done
- Live version list pulled fresh from Mojang
- Auto-detect Java or download Temurin JRE
- Settings stored as readable YAML (not the registry 🎉)

</td>
<td width="50%">

#### 🎮 **Full Control**
- Start, stop, force-kill with graceful shutdown
- Live console with colors, 5000-line buffer, quick-command menu
- Player list, history, playtime tracking, kick/ban/op
- Whitelist & server.properties editor

</td>
</tr>
<tr>
<td width="50%">

#### 🧩 **Plugin Ready**
- Browse & install plugins from **Modrinth** in one click
- Filtered by server software & MC version
- Enable, disable, remove—no restart needed for most
- Supports Paper, Purpur, Spigot, Folia, Fabric, Quilt + more

</td>
<td width="50%">

#### ⚙️ **Built for Production**
- Checksum-verified downloads (server JARs & installers)
- Automatic backups with per-server retention
- Scheduled tasks (cron-style backup automation)
- CPU/memory/uptime live dashboard
- Port forwarding (UPnP) with CGNAT detection

</td>
</tr>
</table>

**Supports 16 server types:** Vanilla, Paper, Purpur, Folia, Spigot (BuildTools), Fabric, Quilt, Forge, NeoForge, Mohist, Arclight, Bedrock, Velocity, BungeeCord, and more.

**Minecraft-flavoured UI:** Blocky buttons, pixel font, game-accurate colors, dark mode, respects `prefers-reduced-motion`. It feels like Minecraft. 🧱

## ⬇️ Download

| Platform | Installer | Format |
|----------|-----------|--------|
| **Windows** | [Download](https://github.com/voidhamsterofficial/mc-server-manager/releases) | `.exe` (NSIS) or `.msi` |
| **macOS** | [Download](https://github.com/voidhamsterofficial/mc-server-manager/releases) | `.dmg` (unsigned; first launch: right-click → Open) |
| **Linux** | [Download](https://github.com/voidhamsterofficial/mc-server-manager/releases) | `.deb`, `.rpm`, or `.AppImage` |

> **Java included!** Servers run anywhere the app runs. Java auto-detection works on all platforms—or we'll download Temurin JRE for you.

## 🚀 Quick Start

### For Users

1. **Grab the installer** → [Releases](https://github.com/voidhamsterofficial/mc-server-manager/releases)
2. **Run it** → App launches with setup wizard
3. **Create a server** → Click **+ New Server**, pick Minecraft version & type, set memory
4. **Click Start** → Live console streams in. That's it! ✨

No terminal. No config files. Just click and play.

### For Developers

**Prerequisites:** [Rust](https://rustup.rs), [Node.js](https://nodejs.org), [Tauri CLI](https://tauri.app) (`cargo install tauri-cli`)

**Linux?** Also install WebKitGTK dev packages:
```sh
sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev \
  librsvg2-dev libappindicator3-dev patchelf
```

**One command to start developing:**
```sh
cargo tauri dev
```
This boots the app in dev mode with instant Rust recompiles and auto-reloading UI. For a release build:
```sh
cargo tauri build
```
Installers land in `src-tauri/target/release/bundle/` (Windows `.exe`, macOS `.dmg`, Linux `.deb`/`.rpm`/`.AppImage`).

**Code quality checks:**
```sh
cd src-tauri && cargo clippy && cargo fmt --check && cargo test
npm run check && npm run build
```

All must pass. See [AGENTS.md](AGENTS.md) for coding standards.

## 🗂️ Architecture

```
src/                       Frontend (Svelte 5 + TypeScript)
├── App.svelte            Main shell, routing, event listeners
├── lib/
│   ├── views/            Dashboard, ServerDetail, AppSettings, Docs
│   ├── components/       Button, ContextMenu, Toasts, dialogs
│   ├── stores/           Reactive state (servers, stats, toasts, etc.)
│   ├── api.ts            Tauri command wrapper
│   ├── events.ts         Event listeners (backend → UI)
│   └── theme.css         Minecraft-inspired colors & typography
└── assets/               Fonts, icons

src-tauri/src/            Backend (Rust + Tauri)
├── main.rs               Entry point
├── lib.rs                Tauri app setup
├── commands.rs           RPC handlers
├── servers.rs            Server registry & YAML persistence
├── process.rs            Java child processes, console streaming
├── console.rs            Log parsing (ready, join/leave, etc.)
├── installers/           14 server type installers
├── java/                 JVM detection, auto-download Temurin
├── plugins.rs            Modrinth plugin manager
├── roster.rs             Player history & moderation
├── backups.rs            Zip backups & restore
├── files.rs              Scoped file browser
├── scheduler.rs          Cron-style task scheduler
└── [stats, portforward, properties, playerdata, etc.]

Config & Tooling
├── Cargo.toml            Rust dependencies & metadata
├── package.json          Node.js dev dependencies
├── vite.config.ts        Frontend build config
├── tsconfig.json         TypeScript config
└── svelte.config.js      Svelte config
```

**Data Storage:**
- **Server configs:** `blockparty-server.yaml` per server folder (human-readable, editable)
- **Global settings:** `blockparty.yaml` beside the app binary
- **Java runtimes:** Per-user app-data directory (auto-downloaded)
- **Player history:** Stored server-side, queried on demand
- **Backups:** Versioned `.zip` files with retention policies
- **No registry:** Everything is portable—move the folder, it just works.

## 🤝 Contributing

We love contributions! Whether it's bug fixes, features, installers, or docs, all help is welcome.

**Before you start:**
- Read [AGENTS.md](AGENTS.md) — our coding standards (DRY, flat code, proper error handling, no panics)
- Ensure `cargo clippy`, `cargo fmt`, `cargo test`, `npm run check`, and `npm run build` all pass
- Coding standards are not optional—enforced by CI

**Your first PR?**
1. Fork the repo
2. Create a branch: `git checkout -b feature/my-awesome-thing`
3. Make your changes (follow AGENTS.md)
4. Test locally: `cargo tauri dev`
5. Push & open a PR with a clear description of what & why
6. We'll review and iterate together ✨

**Found a bug?** Open an issue with:
- What you expected
- What happened instead
- Steps to reproduce
- OS & app version

## 📝 Development Workflow

| Task | Command |
|------|---------|
| Run dev app | `cargo tauri dev` |
| Build for release | `cargo tauri build` |
| Format code | `cargo fmt` && `npm run build` |
| Lint | `cargo clippy` |
| Run tests | `cargo test` (in `src-tauri/`) |
| Check types | `npm run check` |

All checks must pass before submitting a PR. The CI will catch anything that slips through. 🤖

## 📚 Resources

- **[Wiki](https://github.com/voidhamsterofficial/mc-server-manager/wiki)** — User guides & troubleshooting
- **[AGENTS.md](AGENTS.md)** — Coding standards (read before contributing!)
- **[Issues](https://github.com/voidhamsterofficial/mc-server-manager/issues)** — Bug reports & feature requests
- **[Discussions](https://github.com/voidhamsterofficial/mc-server-manager/discussions)** — Chat, ideas, questions

## 🙏 Credits

- **[Monocraft](https://github.com/IdreesInc/Monocraft)** by Idrees Hassan — Minecraft-inspired pixel font (SIL OFL 1.1, no Mojang assets)
- **[mc-heads.net](https://mc-heads.net)** — Player avatar rendering
- **[Tauri](https://tauri.app)** — Rock-solid desktop framework
- **Mojang** — For the best game ever 🎮

ServerForge contains no Minecraft game assets and is not affiliated with, endorsed by, or supported by Mojang or Microsoft.

## 📄 License & EULA

ServerForge is licensed under [MIT](LICENSE). You're free to use, modify, and distribute it.

**Important:** ServerForge downloads official Minecraft server software from Mojang. You must accept the [Minecraft EULA](https://aka.ms/MinecraftEULA) for each server you create. The app records your acceptance in `eula.txt`.

---

<div align="center">

**Made with ❤️ for the Minecraft community**

Have fun! Questions? [Join our discussions](https://github.com/voidhamsterofficial/mc-server-manager/discussions) 🎮

</div>
