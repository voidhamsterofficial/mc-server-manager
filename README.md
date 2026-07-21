# 🎮 ServerForge

> **The friendliest Minecraft server manager.** Create, run, and manage servers from a beautiful desktop app—no terminal required.

<div align="center">

![Rust](https://img.shields.io/badge/rust-%23CE422B.svg?style=for-the-badge&logo=rust&logoColor=white)
![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?style=for-the-badge&logo=typescript&logoColor=white)
![Tauri](https://img.shields.io/badge/tauri-%2324C8DB.svg?style=for-the-badge&logo=tauri&logoColor=white)
![Svelte](https://img.shields.io/badge/svelte-%23f1413d.svg?style=for-the-badge&logo=svelte&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)

**Cross-platform** • **Open source** • **Free**

[⬇️ Download](#-download) • [🚀 Quick Start](#-quick-start) • [🧑‍💻 Contribute](#-contributing) • [📖 Docs](https://github.com/voidhamsterofficial/mc-server-manager/wiki)

</div>

---

Built with **Rust** (Tauri v2 backend) and **Svelte 5** (TypeScript UI).

## 📸 Screenshots

<div align="center">

**Dashboard & Server Management**

<img width="600" alt="Main Dashboard" src="https://github.com/user-attachments/assets/60f984f8-c0e1-4aea-b406-7242dbf44bba" />

**Live Stats**

<img width="600" alt="Live Console" src="https://github.com/user-attachments/assets/a9ca8f99-6d5d-4030-a8ef-379933cdcbd3" />

**Server Overview Dashboard**

<img width="600" alt="Player Management" src="https://github.com/user-attachments/assets/c1f43de3-b7a1-4781-9f34-494f3aafba3f" />

**Server Settings Tab**

<img width="600" alt="Plugin Browser" src="https://github.com/user-attachments/assets/eaefdfbf-32a8-4c0a-887e-48718c17ecf6" />

</div>

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
- Live console with colors, 5000-line buffer, quick commands & command autocomplete
- Player list, history, playtime tracking, kick/ban/op
- Whitelist & server.properties editor, plus a file browser with syntax highlighting

</td>
</tr>
<tr>
<td width="50%">

#### 🧩 **Plugins & Mods**
- Browse & install from **Modrinth** in one click, or drag a `.jar` straight in
- Filtered by server software & MC version, checksum-verified
- Enable, disable, remove, and one-click update
- Plugins on the Paper family, hybrids & proxies; mods on Fabric/Forge/Quilt/NeoForge (CurseForge too, with your API key)

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

**Supports 14 server types:** Vanilla, Paper, Purpur, Spigot (BuildTools), Folia, Fabric, Quilt, Forge, NeoForge, Mohist, Arclight, Bedrock, Velocity, and BungeeCord.

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

**Code quality checks** — all must pass:
```sh
cd src-tauri && cargo clippy --all-targets && cargo fmt --check && cargo test
npm run check && npm run test:run && npm run build
```

See [AGENTS.md](AGENTS.md) for coding standards.

## 🗂️ Architecture

```
src/                       Frontend (Svelte 5 + TypeScript)
├── App.svelte            Main shell, routing, event listeners
├── lib/
│   ├── views/            Dashboard, ServerDetail, AppSettings, Docs, tabs/
│   ├── components/       Button, ContextMenu, ConsoleView, Toasts, dialogs
│   ├── stores/           Reactive state (servers, stats, toasts, etc.)
│   ├── ipc/              api.ts (command wrappers), events.ts (backend → UI)
│   ├── util/             Formatting, highlighting, command autocomplete + data
│   └── theme.css         Minecraft-inspired colors & typography
└── assets/               Fonts, icons

src-tauri/src/            Backend (Rust + Tauri)
├── main.rs               Entry point
├── lib.rs                Tauri app setup & command registration
├── commands.rs           RPC handlers
├── error.rs              AppError / AppResult
├── events.rs             Events emitted to the UI
├── platform.rs           OS-specific process details
├── portforward.rs        UPnP mapping & CGNAT detection
├── servers/              Registry, YAML persistence, scheduler, app state
├── process/              Java child processes, console parsing, stats sampling
├── installers/           11 server-software installers
├── addons/               Plugins, mods, marketplaces (Modrinth/CurseForge), cache
├── java/                 JVM detection, auto-download Temurin
├── players/              Roster, history, playerdata
└── storage/              SQLite db, backups, scoped file browser, properties

Config & Tooling
├── Cargo.toml            Rust dependencies & metadata
├── package.json          Node.js dev dependencies
├── vite.config.ts        Frontend build config
├── tsconfig.json         TypeScript config
└── svelte.config.js      Svelte config
```


**Data Storage:**
- **Server configs:** `blockparty-server.yaml` per server folder (human-readable, editable)
- **App data:** `blockparty.db` (SQLite) in the per-user app-data directory — known servers, settings, player history, scheduled tasks, addon install records
- **Java runtimes:** Per-user app-data directory (auto-downloaded)
- **Backups:** Timestamped `.zip` files with per-server retention policies
- **No registry:** every server folder carries its own config.

## 🤝 Contributing

We love contributions! Whether it's bug fixes, features, installers, or docs, all help is welcome.

**Before you start:**
- Read [AGENTS.md](AGENTS.md) — our coding standards (DRY, flat code, proper error handling, no panics)
- Ensure `cargo clippy`, `cargo fmt --check`, `cargo test`, `npm run check`, `npm run test:run` and `npm run build` all pass
- Coding standards are not optional

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
| Lint Rust | `cd src-tauri && cargo clippy --all-targets` |
| Format Rust | `cd src-tauri && cargo fmt` |
| Test Rust | `cd src-tauri && cargo test` |
| Check types | `npm run check` |
| Test frontend | `npm run test:run` |
| Build frontend | `npm run build` |

**All of these must pass before submitting a PR.**

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
