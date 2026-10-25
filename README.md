# Spiderflayer Launcher

A light Minecraft launcher in the style of the Modrinth app: red accents, OLED-black default theme, glass panels and squircle corners over a custom background. Works with **offline (cracked)** and **Microsoft (premium)** accounts, and installs from **Modrinth** and **CurseForge**.

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
| Discover | Search **Modrinth** and **CurseForge** for mods, modpacks, resource packs and shaders; filter by version, loader and sort; required dependencies install automatically |
| Modpacks | One-click install of Modrinth modpacks, and import of any `.mrpack` file |
| Accounts | Offline accounts (name only) and Microsoft sign-in (device code, no embedded browser) |
| Java | Downloads Mojang's own Java runtime for each Minecraft version, or uses the one you point to |
| Look | OLED or Dark theme, accent colour, glass opacity, background blur and dimming, **your own background image** (Settings), built-in lavender-fields default |

Data lives in `%APPDATA%\SpiderFlayer` (Windows), `~/Library/Application Support/SpiderFlayer` (macOS) or `~/.local/share/SpiderFlayer` (Linux). Put an empty `portable.txt` next to the executable to keep everything in a `data` folder beside it instead.

## Build

The project was written without access to crates.io, so **the Rust code has not been compiled yet**. The first real build is the GitHub Actions workflow, which also runs the unit tests. Expect to fix a handful of small compiler errors on the first run.

**With GitHub Actions (easiest):** push this folder to a GitHub repo. The workflow in `.github/workflows/build.yml` builds installers for Windows, macOS and Linux and attaches them to the run as artifacts.

**Locally:** install [Rust](https://rustup.rs), Node 18+, and on Linux `libwebkit2gtk-4.1-dev build-essential libssl-dev librsvg2-dev`. Then:

```sh
npm install --global @tauri-apps/cli@2
tauri dev      # run
tauri build    # installers in target/release/bundle
```

Preview the UI alone in a browser with `python3 -m http.server -d ui`. Without the Rust backend the page has nothing to talk to; `dev/mock.js` is a stand-in used by the tests in `dev/`.

## Setting things up

**CurseForge:** get a free API key at <https://console.curseforge.com> and paste it in *Settings → Accounts & keys*. Modrinth needs no key.

**Microsoft (premium) sign-in:** Microsoft requires every launcher to use its own Azure app, and **Mojang must approve that app** before Minecraft's API accepts it. Until then sign-in fails with "Invalid app registration".

1. Azure portal → *App registrations* → *New registration*. Account type: *Personal Microsoft accounts only*.
2. *Authentication* → add the platform *Mobile and desktop applications*, and turn on **Allow public client flows**.
3. Copy the *Application (client) ID* into *Settings → Accounts & keys*.
4. Request Mojang's approval at <https://aka.ms/AppRegInfo>.

Offline accounts work immediately with none of this. Offline accounts can't join online-mode servers.

## Not done yet

- CurseForge **modpacks** (Modrinth packs and `.mrpack` files work).
- Pre-1.7 "legacy" asset layout (affects Minecraft older than 1.7).
- Squircle corners use the CSS `corner-shape` property, which Windows (WebView2) supports. macOS and Linux web views fall back to ordinary rounded corners until they add it.

## Layout

```
core/        launcher logic (no GUI): auth, versions, libraries, assets, Java, loaders, mods, modpacks
src-tauri/   thin Tauri shell: commands + window
ui/          index.html, style.css, app.js, logo, default background
logo/        logo source (SVG) and render
dev/         UI test harness (mock backend + Playwright scripts)
```
