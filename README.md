# Spiderflayer Launcher

A light Minecraft launcher in the style of the Modrinth app: purple accents, OLED-black default theme, a macOS-inspired layout (labelled sidebar, traffic-light window buttons, Launchpad-style instance tiles), glass panels and squircle corners over a custom background. Works with **offline (cracked)** and **Microsoft (premium)** accounts, and installs from **Modrinth** and **CurseForge**.

## Why it's light

- **Tauri + Rust**, not Electron. The UI runs in the operating system's own web view, so there is no bundled Chromium. Expect an installer of a few MB.
- **No front-end framework, no build step, no web fonts.** The UI is three plain files (`ui/`).
- **No background work.** No animation loops, no polling, no auto-updater, no telemetry.
- Glass blur is used on a handful of large panels only. Cards are plain translucent fills.
- Default game memory is **2 GB** and is a slider. The launcher hides itself while Minecraft runs (toggle in Settings) and comes back when the game exits.
- Release profile: size-optimised, LTO, stripped, `panic = "abort"`.

## What it does

| | |
|---|---|
| Home / Discover / Library | Modrinth-style layout: icon rail, top bar, instance cards, popular packs and mods |
| Instances | Vanilla, **Fabric, Quilt, Forge, NeoForge**; per-instance memory and Java args; mods / resource packs / shaders with enable-disable and delete; live game log |
| Discover | Search **Modrinth** and **CurseForge** for mods, modpacks, resource packs and shaders; filter by version, loader and sort |
| Project pages | Click any result for a Modrinth-style page: full description, **screenshot gallery with a full-size viewer**, creators, links, compatibility |
| Versions | **Pick any version**, not just the newest: filter by game version, loader and release type, read changelogs, install the exact one. A warning appears if it doesn't match the instance. Required dependencies install automatically |
| Modpacks | One-click install of Modrinth modpacks, and import of any `.mrpack` file |
| Accounts | Offline accounts (name only) and Microsoft sign-in (one click, opens Microsoft's own page) |
| Java | Downloads Mojang's own Java runtime for each Minecraft version, or uses the one you point to |
| Look | Three surface styles you pick in Settings: **Aero** (glass with a bright edge, default), **Frosted** (matte blur) and **Solid** (no blur, lightest on weak GPUs); OLED (default) or Dark theme, purple default accent, glass opacity, background blur and dimming, built-in lavender-fields default. **Your own background:** the picture button in the top bar, Settings, or drop an image on the window |

Data lives in `%APPDATA%\SpiderFlayer` (Windows), `~/Library/Application Support/SpiderFlayer` (macOS) or `~/.local/share/SpiderFlayer` (Linux). Put an empty `portable.txt` next to the executable to keep everything in a `data` folder beside it instead.

## Build

**With GitHub Actions (easiest):** push this folder to a GitHub repo. The workflow in `.github/workflows/build.yml` runs the unit tests and builds installers for Windows, macOS and Linux, attached to the run as artifacts.

**Locally:** install [Rust](https://rustup.rs), Node 18+, and on Linux `libwebkit2gtk-4.1-dev build-essential libssl-dev librsvg2-dev`. Then:

```sh
npm install --global @tauri-apps/cli@2
tauri dev      # run
tauri build    # installers in target/release/bundle
```

The UI is tested without Rust against a mock backend: `python3 dev/flows.py` and `python3 dev/flows2.py` (needs Python, Playwright and Pillow).

## Microsoft (premium) sign-in

Players click **Sign in with Microsoft**, Microsoft's own page opens, they approve it, and they are logged in.

Microsoft requires every launcher to have its own app registration, and **Mojang must approve it** before Minecraft's servers accept it. Until then sign-in says "Mojang has not approved this Azure app yet". Offline accounts always work.

1. [Azure portal](https://portal.azure.com) > *App registrations* > *New registration*, accounts: **Personal Microsoft accounts only**.
2. Redirect URI: platform **Public client/native (mobile & desktop)**, value `http://localhost`.
3. *Authentication* > turn on **Allow public client flows**.
4. Copy the **Application (client) ID** and request approval: <https://aka.ms/AppRegInfo>.
5. Either bake it into builds so players never see it (GitHub Actions variable `SPIDERFLAYER_MS_CLIENT_ID`, or `SPIDERFLAYER_MS_CLIENT_ID=<id> tauri build`), or just click *Sign in with Microsoft* in the app: if no ID is set it asks for it once, right there, and remembers it.

## The "Rust" instance (Horizon)

A default instance called **Rust** runs [Horizon](https://github.com/horizon-rust/horizon) (MIT), an unofficial Minecraft: Java Edition rewrite in Rust, instead of Java Minecraft. It is much faster but **cannot run Java mods** (Fabric, Forge). It is created once on first start; delete it and it stays deleted.

- Spiderflayer does not download or bundle Horizon. It runs the copy that **Horizon's own launcher** (playhorizon.dev) installed and prepared. Install a version there and press *Prepare* once (needs your own Minecraft: Java Edition 26.3, and Java 25 for that step).
- It needs a **Microsoft account that owns Minecraft**, the same rule as Horizon's launcher. Offline accounts are refused for this instance.
- The sign-in is handed over the way Horizon documents: an owner-only session file the game reads once.
- Horizon is by its own authors and licensed MIT; see its repository for its licence and third-party notices. Spiderflayer contains none of its code.

## Other setup

**CurseForge:** get a free API key at <https://console.curseforge.com> and paste it in *Settings > Accounts & keys*. Modrinth needs no key.

## Not done yet

- CurseForge **modpacks** (Modrinth packs and `.mrpack` files work).
- Pre-1.7 "legacy" asset layout (affects Minecraft older than 1.7).
- Squircle corners use the CSS `corner-shape` property, which Windows (WebView2) supports. macOS and Linux web views fall back to ordinary rounded corners until they add it.

## Layout

```
core/        launcher logic (no GUI): auth, versions, libraries, assets, Java, loaders, mods, modpacks
src-tauri/   thin Tauri shell: commands + window
ui/          index.html, style.css, app.js, logo, default background
logo/        logo source images
dev/         UI test harness (mock backend + Playwright scripts)
```
