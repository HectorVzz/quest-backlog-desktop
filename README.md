# Quest Backlog for desktop

A small companion app for [Quest Backlog](https://quest-backlog.com) on
Windows and Linux. It lives in the system tray, like Flameshot. Press a
shortcut while playing and a quick panel opens over the game. From it you
can update the game's status or post to Quest Backlog, with a screenshot of
the game if you want one.

Status: in development, no releases yet. Tested on Linux (KDE Plasma,
Wayland); the Windows build is not tested yet.

## What it does on your computer

The source is open so you can check these claims yourself.

- **Nothing is injected into games.** No graphics API is hooked and no game
  files are touched. The quick panel is an ordinary always-on-top window.
  Because of this, games in exclusive fullscreen are not supported;
  borderless fullscreen works.
- **Nothing runs in the background.** While the panel is closed the app
  only waits for its shortcuts and the tray icon. There is no polling, no
  update checker and no telemetry.
- **Game detection reads process metadata only, and only when you open the
  panel.** It reads names and environment variables of running processes,
  never their memory. Details below.
- **Screenshots capture one window, never the whole desktop on KDE or
  Windows.** A screenshot stays in memory until you post or discard it. A
  copy is kept on disk only if you turn that on in Preferences.
- **Your sign-in** is a device token kept in the OS keychain (Windows
  Credential Manager, or Secret Service on Linux). If no keychain is
  available, sign-in is refused instead of storing the token in a file.
- **Start with the system** is off until you turn it on in Preferences.

### What it sends, and where

The app connects to two places only:

- `quest-backlog.com`, the Quest Backlog API, and only when you do
  something: sign in, open the panel (to look up the detected game and your
  status for it), search for a game, save a status or post.
- `images.igdb.com`, to show game covers.

What the server receives:

- At sign-in: your computer's name, your OS name and the app version. You
  see them when you approve the sign-in and later under Connected devices on
  the website, where you can sign the app out.
- A detected Steam game's app id, to look the game up.
- What you choose to save or post: a status, text, a screenshot.

The executables you matched to games (`games.json`) stay on your computer.

## Using it

- **Tray icon:** left click opens or closes the quick panel; right click
  shows *Open quick panel*, *Screenshot a window and post…*,
  *Preferences…*, *Quit*. The app never sits in the taskbar.
- **Quick panel:** always on top, Escape closes it. The detected game's
  cover and title sit at the top. Two tabs: *Status* (one-click status; the
  number keys 1 to 6 pick Playing, Completed, Platinum'd, Backlog, Dropped,
  Wishlist) and *Post* (text, game, screenshot, spoiler; Ctrl+Enter posts).
  The first time you open it, it lists these keys until you press *Got it*.
- **Preferences:** a normal window. Shortcuts, keeping a copy of screenshots
  and where, language (English or Spanish), start with the system.
- **Shortcuts:** `Ctrl+Shift+G` opens the quick panel; `Ctrl+Shift+F12`
  takes a screenshot of the game and opens the post composer with it.

### Sign-in

The app never sees your password. It uses a device authorization flow:

1. The app asks the server for a short code.
2. Your browser opens quest-backlog.com/link with that code. You sign in
   there as usual and approve the device.
3. The app receives a device token and stores it in the keychain.

Signing out in the app revokes the token on the server. Revoking it from
Connected devices on the website signs the app out.

### Shortcuts

On Windows and X11 the app registers the shortcuts itself; change them in
Preferences with the key recorder.

On Wayland the desktop owns global shortcuts. The app asks for them through
the XDG GlobalShortcuts portal with the defaults above as suggestions.
Preferences shows what the desktop actually assigned, and *Change
shortcuts* opens the desktop's own dialog. KDE only applies the suggested
keys the first time an app registers a shortcut; one added later stays
unassigned until you set it.

The portal knows the app by its launcher entry,
`com.questbacklog.QuestBacklog.desktop`. Without it (for example under
`pnpm tauri dev`) the desktop files the shortcuts under whatever started the
app, such as your terminal. The app id must not end in `.desktop`: KDE's
shortcut service reads such a name as a launcher file and silently drops
the shortcuts.

### Game detection

Each time the quick panel opens, the app looks for the running game:

1. An executable you already matched on this computer (`games.json`).
2. A Steam game: `RunningAppID` in the registry on Windows. On Linux, the
   `SteamAppId` environment variable of the game's processes, which also
   covers Proton. Launchers based on umu (Faugus, Lutris, Heroic) set a
   non-numeric value there, which is ignored.
3. Any other game executable. On Windows, the program whose window was in
   front when the panel opened. On Linux, Windows `.exe` programs running
   under Wine or Proton, minus Wine's own programs, launchers, anti-cheat
   and helpers (the `IGNORED_*` lists in `src-tauri/src/detect.rs`). The
   panel asks once "Which game is Wow.exe?", with the search pre-filled from
   a small built-in list (`src-tauri/src/known_games.rs`) or the file name,
   and remembers your answer.

Native Linux games outside Steam are not detected; search for them instead.

### Posts and screenshots

A post has up to 500 characters of text, an optional game, a spoiler flag
and an optional image, and may be only an image. Screenshots are scaled to
at most 2560 px and sent as JPEG; the server resizes them again, moderates
them and counts them in the daily image limit. If the server refuses a post,
its message is shown as it is.

- The shortcut, `--screenshot` and *Add screenshot* capture the active
  window, which is the game. With the panel open on KDE, the panel lets go
  of the keyboard for about 60 ms so KWin makes the game active, captures it
  and takes the keyboard back; the panel stays on screen.
- *Choose window…* and the tray item show KWin's crosshair: click a window,
  Escape cancels.
- KDE Plasma uses KWin's `org.kde.KWin.ScreenShot2` (no window frame, a few
  milliseconds). The launcher entry declares it in
  `X-KDE-DBUS-Restricted-Interfaces`, like Flameshot's.
- Other Linux desktops use the XDG Screenshot portal, which captures the
  whole desktop (or shows its own dialog when choosing a window).
- Windows cuts the foreground window's client area from a capture of its
  monitor. There is no window picker there yet.

### Settings

`~/.config/com.questbacklog.QuestBacklog/settings.json` on Linux,
`%APPDATA%\com.questbacklog.QuestBacklog\settings.json` on Windows. Edit
them from Preferences:

```json
{
  "hotkey": "Ctrl+Shift+G",
  "screenshotHotkey": "Ctrl+Shift+F12",
  "saveScreenshots": false,
  "screenshotFolder": null,
  "language": null,
  "tipsSeen": false
}
```

`screenshotFolder: null` means `<Pictures>/Quest Backlog`. `language` is
`"en"`, `"es"` or `null` (the system language). `tipsSeen` turns into
`true` when you dismiss the quick panel's shortcut list.

## Building

You need Rust (stable), Node.js with pnpm and, on Linux, `webkit2gtk-4.1`
and `gtk-layer-shell`.

```bash
pnpm install
pnpm tauri dev                    # run from source
pnpm typecheck
cargo test --manifest-path src-tauri/Cargo.toml
```

To install it for your user on Linux, with a launcher entry and icon:

```bash
scripts/install-local.sh              # build and install into ~/.local
scripts/install-local.sh --uninstall
```

Start it from your application launcher.

The app talks to https://quest-backlog.com. To work against a local
Quest Backlog server, set `QB_API_BASE_URL` and `QB_WEB_BASE_URL` at build
time, for example `QB_API_BASE_URL=http://localhost:4000/api
QB_WEB_BASE_URL=http://localhost:3000 pnpm tauri dev`.

Command-line flags: `--toggle` toggles the quick panel of the running
instance, `--screenshot` takes a screenshot of the game and opens the
composer, `--show` opens the panel at start, `--autostart` is passed by the
login entry (the panel then stays closed). The log line
`[screenshot] ... captured in N ms, ready in N ms` shows capture timings.

## How the code is laid out

- `src-tauri/`: the Rust core (Tauri 2). It owns all HTTP, the device token,
  shortcuts, the tray, screenshots and the windows.
  - `api.rs` HTTP client for the Quest Backlog API (always sends
    `X-CSRF-Token: 1`, and the bearer token once signed in)
  - `auth.rs` device flow and sign-in state; the web view only sees `AuthView`
  - `secret.rs` token in the OS keychain
  - `overlay.rs` quick panel; on Wayland a layer-shell surface on the overlay layer
  - `hotkey.rs` shortcuts: plugin on Windows/X11, GlobalShortcuts portal on Wayland
  - `tray.rs` tray icon: `ksni` on Linux, Tauri's tray on Windows
  - `preferences.rs` Preferences window and its commands
  - `settings.rs` `settings.json`
  - `detect.rs` finds the running game
  - `known_games.rs` executable to game: your choices (`games.json`) and the built-in list
  - `games.rs` detection, search, choosing a game, saving its status
  - `screenshot.rs` capture, resize and encode, optional local PNG copy
  - `compose.rs` screenshot flow and posting
  - `device.rs` the computer name and OS sent at sign-in
  - `config.rs` server addresses
- `src/`: the interface (Vite + Preact), one bundle for both windows
  (`main.tsx` picks by window label). `i18n.ts` holds the English and
  Spanish strings; `tokens.css` holds the web app's color tokens.
- `scripts/install-local.sh`: build and install for the current Linux user.

The web view has only Tauri's `core:default` permissions. It never sees the
token or the screenshot bytes (only a small preview), and makes no HTTP
requests; it calls the commands registered in `src-tauri/src/main.rs`.

Rules for changes:

- Keep the promises in "What it does on your computer". Anything new that
  runs in the background, touches a game or sends data needs a strong
  reason and a line in this README.
- Every interface string goes into both `en` and `es` in `src/i18n.ts`
  (tray labels are in `src-tauri/src/tray.rs`).

Memory: WebKitGTK costs about 190 MB per open web view on Linux. Only the
quick panel's web view is kept; Preferences is created when opened and
destroyed when closed.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

Quest Backlog's server and website are not part of this repository.
