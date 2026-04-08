# GameLexicon

GameLexicon is a local-first desktop app for tracking a personal game library, importing owned games from storefronts, organizing a play queue, enriching metadata, and backing up data to CSV.

The app is built with:

- React + Vite for the frontend in `app/`
- Rust + Tauri 2 for the desktop shell and local services in `src-tauri/`
- SQLite for the local library database

## Documentation

- `docs/GameLexicon_User_Manual_v1.0.md` is the full user manual.
- `docs/GameLexicon_User_Manual_v1.0.docx` is the Word version of the same manual.
- `CHANGELOG.md` tracks project changes.
- `TODO.md` tracks pending work.

## Repository Layout

```text
game-collection/
  app/                         React + Vite frontend
    package.json
    package-lock.json
    index.html
    src/
  src-tauri/                   Rust + Tauri app
    Cargo.toml
    Cargo.lock
    tauri.conf.json
    src/
    src/db/migrations/
    icons/
  docs/                        User-facing documentation
  scripts/                     Helper scripts
  .env.example                 Optional environment examples
  .gitignore
  README.md
```

## Prerequisites

Windows development needs:

1. Git for Windows: https://git-scm.com/download/win
2. Node.js LTS: https://nodejs.org/
3. Rust with the MSVC toolchain: https://win.rustup.rs/x86_64
4. Visual Studio Build Tools with the `Desktop development with C++` workload.
5. Microsoft Edge WebView2 Runtime. It is already present on many Windows 10/11 machines, but install the Evergreen runtime if Tauri reports that WebView2 is missing.

After installing Git, Node, Rust, or GitHub CLI, open a new terminal so PATH changes are picked up.

Optional tools for specific importers:

- Legendary CLI for Epic import: https://github.com/derrod/legendary
- GOG Galaxy installed locally for GOG owned import
- Amazon Games installed locally for Amazon Games import
- A Steam profile, and optionally a Steam Web API key, for Steam import
- Twitch developer credentials for IGDB enrichment

## Install From Source

Clone the repo and install frontend dependencies:

```powershell
git clone <repository-url>
cd GameLexicon
cd app
npm ci
```

Install the Tauri CLI if you do not already have Tauri CLI v2:

```powershell
cargo install tauri-cli --locked
```

Run the app in development mode from the repo root:

```powershell
cd ..
cargo tauri dev
```

`cargo tauri dev` starts the Vite dev server automatically through the `beforeDevCommand` in `src-tauri/tauri.conf.json`.

## Build An Installer

Install dependencies first, then run from the repo root:

```powershell
cargo tauri build
```

On Windows, Tauri creates release output under `src-tauri/target/release/` and bundled installers under `src-tauri/target/release/bundle/`.

For a Rust-only compile check without creating installers:

```powershell
cd src-tauri
cargo check
```

For a frontend production build only:

```powershell
cd app
npm run build
```

## Local Data

GameLexicon creates its default database locally at:

```text
%LOCALAPPDATA%\GameLexicon\gamelexicon.sqlite
```

That database is intentionally not committed to Git. The repository ignores common local database files, generated frontend output, `node_modules`, and Rust build output.

You do not need a `.env` file for normal local use. If you want to override the database location for development, copy `.env.example` to `.env` and edit `DATABASE_URL`.

```powershell
copy .env.example .env
```

## Credentials

Twitch credentials are used for IGDB metadata enrichment. Enter them in the app Settings dialog, or use `.env` only for local development experiments.

Steam and Twitch credentials are saved through the app's local credential storage. Xbox import uses Microsoft device-code sign-in and does not require a client secret.

Do not commit a real `.env` file or a local SQLite database. `.gitignore` excludes both.

## Import Notes

- Steam import can use a Steam API key or the public Steam Community XML fallback.
- Epic import requires Legendary, and you should run `legendary auth` outside GameLexicon at least once.
- GOG owned import reads the local Galaxy SQLite database, usually from `C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db`.
- Amazon Games import reads a local Amazon Games SQLite database.
- CSV import supports spreadsheet-style imports with at least `Title` and `Platform` columns.

## Troubleshooting

If `npm` is blocked in PowerShell, use Command Prompt or allow local script execution for your user.

If `cargo tauri dev` cannot find `npm`, `node`, or `cargo`, close and reopen your terminal after installing prerequisites.

If Tauri cannot find WebView2, install the Microsoft Edge WebView2 Evergreen Runtime.

If packaging fails because a platform tool is missing, install the Visual Studio Build Tools C++ workload and retry from a fresh terminal.
