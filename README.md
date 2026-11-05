# Zuno

**Less noise. More web.**

Zuno is a minimalist Windows browser. Websites are rendered by the Microsoft Edge **WebView2**
runtime (Chromium) through **Wry/Tauri 2**; the browser core is written in **Rust** and the
interface in **React + TypeScript**.

## Quick start

### (Short way)
Go to the website, and download the installer

### (Long way)

### First, install the zip and extract it
Then, open up a CMD/Powershell and run these-

```powershell
npm install
npm run tauri dev      # development (hot reload for the interface, auto-rebuild for Rust)
npm run tauri build    # release build + Windows installer
npm run test:rust      # Rust unit and integration tests
npm run typecheck      # TypeScript checks
```

Requirements: Rust (stable, MSVC toolchain), Node.js 20+, Visual Studio Build Tools with the C++
workload, and the WebView2 Runtime (preinstalled on Windows 11; the installer downloads it when
missing).

The installer is written to
`src-tauri/target/release/bundle/nsis/Zuno_<version>_x64-setup.exe` (per-user install, no admin
rights needed).

## Architecture

```
┌──────────────────────────── Main window (frameless) ────────────────────────────┐
│  "ui" webview — Zuno's interface (React), spans the whole window               │
│   ┌ tab strip · toolbar · bookmarks bar · banners ─────────────────────────────┐ │
│   └────────────────────────────────────────────────────────────────────────────┘ │
│   ┌ content area ──────────────────────────────────────────────────────────────┐ │
│   │  "tab-N" webview (one per tab, WebView2 controller) placed exactly here     │ │
│   │  or an internal page (zuno://newtab, settings, history, …) rendered by "ui" │ │
│   └────────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────┘
```

* **Independent tabs.** Every tab is its own WebView2 instance, created as a child webview of the
  window with Tauri's multi-webview API (`Window::add_child`, `unstable` feature). The UI reports
  the height of its chrome; Rust positions the active tab's webview inside the content area and
  hides the others. Restored tabs are created lazily when first shown.
* **Rendering abstraction.** The core talks to pages only through the `RenderingEngine` /
  `ContentView` / `EngineSink` traits (`browser/engine.rs`). `browser/webview.rs` is the WebView2
  implementation; the tab model, storage and commands do not depend on it.
* **WebView2 integration.** Tauri/Wry hooks are used for navigation, page loads, titles and new
  windows. Everything else is wired directly to the WebView2 COM API via `webview2-com`:
  back/forward state, favicons (`GetFavicon`), stop, accelerator keys, download progress and
  control (`DownloadStarting` / `DownloadOperation`), fullscreen, renderer crashes, external URI
  schemes, profile settings (colour scheme, tracking prevention), cookie import and clearing data.
* **Shared profile.** All tabs share one persistent WebView2 profile, so site sessions, cookies and
  storage survive restarts like in any desktop browser.
* **Native menus.** The browser menu and tab/group context menus are native Win32 menus because
  they must draw above website content.

```
src-tauri/src/
  lib.rs                 app setup, window + interface webview, logging, window state
  browser/
    mod.rs               browser core: tabs ↔ views, layout, session, engine events
    engine.rs            rendering-engine abstraction (traits + event types)
    webview.rs           WebView2 implementation (Tauri child webviews + COM)
    tabs.rs              tab & tab-group model (pure, unit tested)
    navigation.rs        address-bar input resolution, search engines, internal pages
    shortcuts.rs         keyboard shortcut table
  authentication/
    oauth.rs             OAuth/OIDC request analysis, PKCE, authorization-code client
    credentials.rs       Windows Credential Manager storage
    mod.rs               connected accounts
  storage/               SQLite (bookmarks, history, favicons, downloads), settings, session
  importer/              Chromium-family and Firefox import
  downloads/             download manager
  incognito/             incognito sessions, cleanup, MAC address randomization (mac.rs)
  commands/              Tauri commands (UI → core), all restricted to the "ui" webview
  menus.rs               native menus
  security/              scheme policy, history sanitising, file-name sanitising, caller checks
src/                     React interface (components/, pages/, lib/, styles/)
```

## Features

| Area | What works |
| --- | --- |
| Rendering | Real websites via WebView2; HTTPS certificate errors are blocked by WebView2 (never bypassed) |
| Navigation | URL vs. search detection, back/forward, reload, stop, loading indicator, page titles, favicons, error states, HTTP security indicator |
| Tabs | Independent tabs, open/close/switch, drag to reorder, middle-click close, `target=_blank` → new tab, reopen closed tab, duplicate, session restore |
| Tab groups | Create, rename (double-click or menu), colour (muted tones), collapse/expand, add/remove tabs, ungroup, close group; persisted with the session |
| Quick switcher | `Ctrl+K`: open tabs, bookmarks and history with keyboard navigation |
| Bookmarks | Add (star / `Ctrl+D`), edit, move between folders, drag to reorder, search, folders, bookmarks bar, HTML export |
| History | Recorded per visit (auth codes/tokens stripped), search, day grouping, delete entries, clear all, optional clear on exit |
| Downloads | Real downloads with progress, pause/resume, cancel, errors, open file, show in folder, configurable folder, never overwrites existing files |
| Settings | Appearance (light/dark/system), search engine (6 built-in + custom), privacy, downloads, startup, accounts, import/export, about |
| Privacy | WebView2 tracking prevention (off/basic/balanced/strict), password autosave & autofill toggles, clear browsing data (history, downloads list, cookies, site storage, cache, passwords, autofill) |
| Incognito | `Ctrl+Shift+N` / menu: a separate process with InPrivate webviews in a throwaway data folder (deleted on close, swept after a crash); no history, favicons, session, download list, settings changes, window state or log file written; regular history never shown; bookmarks shown read-only; download origin URLs stripped from the Mark of the Web; DNS cache flushed on close; optional MAC address randomization while incognito windows are open |
| Import | Chrome, Edge, Brave, Vivaldi, Chromium, Opera (GX): bookmarks, history, settings. Firefox: bookmarks, history, cookies, settings. Per-category availability, progress, summary |
| Authentication | Pasted OAuth/OIDC authorization URLs are reviewed before continuing (provider, client, scopes, redirect, PKCE, state, warnings); completion/denial detected at the redirect; app callbacks handed to Windows after confirmation; scripted sign-in popups keep `window.opener` |
| Accounts | Zuno-managed OpenID Connect sign-in (authorization code + PKCE S256, loopback redirect intercepted in the tab, state/issuer validation); tokens stored only in Windows Credential Manager; revocation on disconnect |

### Keyboard shortcuts

Shortcuts work whether Zuno's interface or a website has focus (WebView2 `AcceleratorKeyPressed`).

| | |
| --- | --- |
| `Ctrl+T` / `Ctrl+W` / `Ctrl+Shift+T` | New tab / close tab / reopen closed tab |
| `Ctrl+Tab`, `Ctrl+Shift+Tab`, `Ctrl+1…8`, `Ctrl+9` | Switch tabs |
| `Ctrl+L`, `Alt+D`, `F6` | Focus address bar (`Alt+Enter` opens in a new tab) |
| `Ctrl+K` | Quick switcher |
| `Ctrl+R`, `F5` / `Alt+←` / `Alt+→` | Reload / back / forward |
| `Ctrl+D` / `Ctrl+Shift+O` / `Ctrl+Shift+B` | Bookmark page / bookmarks / bookmarks bar |
| `Ctrl+H` / `Ctrl+J` / `Ctrl+,` | History / downloads / settings |
| `Ctrl+Shift+N` | New incognito window |
| `F11` / `F12` | Fullscreen / developer tools for the page |

## Security model

* Website tabs (`tab-*`) and popups receive **no Tauri capabilities**. Every Zuno command
  additionally verifies the caller is the `ui` webview. Verified at runtime: calling a command from
  a website is rejected.
* Tauri's opener plugin is configured not to inject its link-hijacking script into pages.
* Navigation policy: `http`, `https`, `about`, `blob`, `file` load in tabs; `javascript:`, `data:`,
  internal and Windows-privileged schemes (`ms-*`, `search-ms`, …) are blocked; other schemes
  (`mailto:`, app callbacks) open only after the user confirms.
* HTTPS validation is left entirely to WebView2; there is no certificate-error override.
* History never stores credentials in URLs or OAuth artefacts (`code`, `access_token`,
  `id_token`, `state`-adjacent secrets, passwords); the importer applies the same sanitiser.
* Downloads: file names are sanitised (no path traversal, reserved names), existing files are never
  overwritten, and only files recorded by the download manager can be opened from the UI.
* No cookie values, tokens or passwords are logged. Logs: `%APPDATA%\app.zuno.browser\logs`.

## Known limitations

* **Cookies and passwords from Chromium browsers are not imported.** Chrome, Edge and other
  Chromium browsers encrypt them with keys bound to the browser (DPAPI and, in Chrome, app-bound
  encryption). Zuno does not circumvent another application's protection. Firefox cookies are
  stored unencrypted and are imported; **no browser's passwords** can be imported because WebView2
  provides no API to add entries to its password store.
* **Modal dialogs hide the live page.** Website views are native child windows above Zuno's HTML
  interface, so while the quick switcher or a dialog is open the page is hidden and a dimmed still
  of it is shown underneath. Menus are native for the same reason. Address-bar suggestions are not
  shown as a dropdown; the quick switcher (`Ctrl+K`) covers that use.
* **All tabs of a window share one browsing profile** (as in a regular browser window).
* **Incognito limits.** Websites, your network operator and anyone watching the network still see
  what you visit; downloaded files stay on disk. Deleting the session folder is not a secure wipe
  of the disk. MAC randomization needs administrator approval each time the first incognito
  window opens, briefly reconnects the network, and only works where the adapter driver honours
  the `NetworkAddress` override (most Ethernet and Intel/Realtek Wi-Fi drivers do; some don't). If
  Windows shuts down while it is active, the original address is restored the next time an
  incognito window starts.
* Tauri injects its IPC bridge object into every webview; websites can detect it but cannot use
  it (all calls are refused).
* Downloads interrupted by closing Zuno cannot be resumed after restart (WebView2 does not persist
  download operations); they can be retried from the downloads page.
* Zuno-managed accounts have been unit tested (PKCE, state/issuer checks, redirect interception)
  but require a provider client registration to exercise end to end.
* Windows only.

## Data locations

* Profile data: `%APPDATA%\app.zuno.browser\` — `zuno.db` (bookmarks, history, favicons,
  downloads), `settings.json`, `session.json`, `accounts.json` (no secrets), `window.json`, `logs\`.
* Website data (cookies, cache, storage): `%LOCALAPPDATA%\app.zuno.browser\EBWebView\`.
* Incognito windows: `%LOCALAPPDATA%\app.zuno.browser\Incognito\<random>\`, only while open.
* Secrets: Windows Credential Manager entries prefixed `Zuno/`.
