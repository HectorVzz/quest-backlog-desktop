# Quest Backlog for desktop

A small companion app for [Quest Backlog](https://quest-backlog.com) on
Windows and Linux. It lives in the system tray, like Flameshot. Press a
shortcut while playing and a quick panel opens over the game. From it you
can update the game's status and hours played, share the update to your
feed, or post to Quest Backlog with a screenshot of the game.

Status: works against quest-backlog.com since 2026-11-17. Tested on Linux
(KDE Plasma, Wayland). Windows builds are made by CI but have not been
tested on a Windows machine yet.

## Download

Get the latest version from
[GitHub Releases](https://github.com/HectorVzz/quest-backlog-desktop/releases/latest):

- **Windows:** `Quest.Backlog_<version>_x64-setup.exe`. It installs for
  your user only (no administrator rights) and adds a Start menu entry.
  The installer is not code signed, so SmartScreen may show "Windows
  protected your PC" for an unknown publisher: choose *More info*, then
  *Run anyway*. If you would rather not, build it from source (below).
- **Linux:** the `.AppImage` (`chmod +x` it and run it) or the `.deb` for
  Debian and Ubuntu (`sudo apt install ./Quest.Backlog_<version>_amd64.deb`).

There is no auto-updater; install a new version over the old one.

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
  status and hours for it), search for a game, save a status or hours,
  post, or open the *Feed* or *My posts* tab (to load posts and comments,
  like a post or comment on it). Post images and avatars are loaded from
  there too.
- `images.igdb.com`, to show game covers.

Avatars or videos hosted anywhere else are not loaded; a video post shows a
link to the post on the website instead.

What the server receives:

- At sign-in: your computer's name, your OS name and the app version. You
  see them when you approve the sign-in and later under Connected devices on
  the website, where you can sign the app out.
- A detected Steam game's app id, to look the game up. For a game you
  matched to an executable, its Quest Backlog address name (for example
  `world-of-warcraft-forever`), to get its current record.
- What you choose to save or post: a status, hours played, text, a
  screenshot, a like, a comment.

The executables you matched to games (`games.json`) stay on your computer.

## Using it

- **Tray icon:** left click opens or closes the quick panel; right click
  shows *Open quick panel*, *Screenshot a window and post…*,
  *Preferences…*, *Quit*. The app never sits in the taskbar.
- **Quick panel:** always on top, Escape closes it. The detected game's
  cover and title sit at the top. Four tabs: *Status* (one-click status; the
  number keys 1 to 6 pick Playing, Completed, Platinum'd, Backlog, Dropped,
  Wishlist), *Post* (text, game, screenshot, spoiler; Ctrl+Enter posts),
  *Feed* and *My posts*. The first time you open it, it lists these keys
  until you press *Got it*. Closing it with X explains that the app stays in
  the tray and how to come back (the shortcut, the tray icon or the app
  launcher), until you tick *Don't show this again*. Preferences can turn
  either one back on.
- **Feed and My posts:** the website's activity feed (*For you* and
  *Following*) and your own posts, ten at a time with *Load more*. Like a
  post, open its comments to read the latest 20 and reply, or open it on the
  website. A tab loads when you open it and again the next time the panel
  is shown; the refresh button reloads it. Spoilers stay hidden until you
  press *Show spoiler*.
- **Hours played:** under the status buttons; saved on Enter or when you
  leave the field (empty keeps the saved hours). Games synced from Steam
  show Steam's hours read-only, because typing hours there would unlink
  them from Steam on Quest Backlog.
- **Post update:** shares the game's current status to your feed, like
  "Post update" on the website, with an editable sentence and your hours.
  The "Saved as …" message offers it right after you change a status.
  Nothing is shared unless you press *Post*.
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

`games.json` keeps the game you chose, and each time it is detected the app
asks the server for that game's current record by its slug: game ids are
the server's own and differ between servers. If the server no longer has
that game, the choice is forgotten and the panel asks again.

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
- *Crop* shows the whole screenshot: drag a box over the part to post, drag
  the box to move it or its corners to resize it, then *Apply* (Enter).
  Only the rectangle goes back to the Rust core, which cuts it from the
  full-size capture and encodes it again; *Whole screenshot* undoes it. The
  local PNG copy, when on, is always the whole capture.
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
  "tipsSeen": false,
  "closeNoticeSeen": false
}
```

`screenshotFolder: null` means `<Pictures>/Quest Backlog`. `language` is
`"en"`, `"es"` or `null` (the system language). `tipsSeen` turns into
`true` when you dismiss the quick panel's shortcut list, and back to
`false` when Preferences turns it on again; `closeNoticeSeen` works the
same way for the notice shown when X closes the panel.

## Building

You need Rust (stable), Node.js with pnpm and, on Linux, `webkit2gtk-4.1`
and `gtk-layer-shell` (on Debian/Ubuntu the `-dev` packages listed in
`LINUX_DEPS` in `.github/workflows/release.yml`).

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

Installers: `pnpm tauri build --bundles nsis` on Windows,
`pnpm tauri build --bundles appimage,deb` on Linux; they land in
`src-tauri/target/release/bundle/`.

The app talks to https://quest-backlog.com. To work against a local
Quest Backlog server, set `QB_API_BASE_URL` and `QB_WEB_BASE_URL` at build
time, for example `QB_API_BASE_URL=http://localhost:4000/api
QB_WEB_BASE_URL=http://localhost:3000 pnpm tauri dev`.

Command-line flags: `--toggle` toggles the quick panel of the running
instance, `--screenshot` takes a screenshot of the game and opens the
composer, `--autostart` is passed by the login entry. Starting the app opens
the quick panel, except with `--autostart`, where it stays in the tray. The
log line
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
  - `feed.rs` Feed and My posts: pages of posts, likes, comments
  - `device.rs` the computer name and OS sent at sign-in
  - `config.rs` server addresses
- `src/`: the interface (Vite + Preact), one bundle for both windows
  (`main.tsx` picks by window label). `i18n.ts` holds the English and
  Spanish strings; `tokens.css` holds the web app's color tokens.
- `scripts/install-local.sh`: build and install for the current Linux user.
- `src-tauri/icons/`: the app icon. `icon.svg` is the source; the PNG
  sizes and `icon.ico` (Windows) are rendered from it:
  `for s in 16 32 48 64 128 256; do rsvg-convert -w $s -h $s icon.svg -o ${s}x${s}.png; done`,
  `rsvg-convert -w 512 -h 512 icon.svg -o icon.png`, and
  `magick 16x16.png 32x32.png 48x48.png 64x64.png 256x256.png icon.ico`.

The web view has only Tauri's `core:default` permissions. It never sees the
token or the screenshot bytes (only small previews; cropping sends a
rectangle back), and makes no API requests; it calls the commands
registered in `src-tauri/src/main.rs`. It only loads images, from the hosts
the CSP in `src-tauri/tauri.conf.json` allows (`images.igdb.com`,
`quest-backlog.com` and `localhost:4000` for a local server); `feed.rs`
drops image addresses on any other host before they reach it.

Rules for changes:

- Keep the promises in "What it does on your computer". Anything new that
  runs in the background, touches a game or sends data needs a strong
  reason and a line in this README.
- Every interface string goes into both `en` and `es` in `src/i18n.ts`
  (tray labels are in `src-tauri/src/tray.rs`).

Memory: WebKitGTK costs about 190 MB per open web view on Linux. Only the
quick panel's web view is kept; Preferences is created when opened and
destroyed when closed.

## Troubleshooting

The app logs one line per event, in the journal on Linux
(`journalctl --user | grep 'quest-backlog-desktop\['`) or on stderr when run
from a terminal:

- `[hotkey] portal bound …` at start: the shortcuts the desktop assigned.
- `[hotkey] activated toggle-overlay` / `screenshot`: a shortcut fired.
- `[overlay] show (visible before: …)` / `[overlay] hide`: the panel was
  opened or closed, from any source (shortcut, tray, `--toggle`).
- `[screenshot] ActiveWindow captured in N ms, ready in N ms`, with
  `(overlay kept)` when the panel stayed open during the capture.
- `[games] …`: a remembered game could not be refreshed or saved.

If the panel "does not open": when the log shows `[overlay] show` but
nothing appears, the app did its part and the compositor did not display
it; see the notes below. If there is no `show` line, the click or shortcut
never reached the app.

If saving a status fails for a detected non-Steam game, delete its entry in
`games.json` (next to `settings.json`) and choose the game again.

## Notes for maintainers

Things learned building and testing the app, so they are not relearned.

The quick panel on KDE Plasma (Wayland):

- It is a `gtk-layer-shell` surface on the overlay layer, which is what
  keeps it above borderless-fullscreen games. The layer shell has to be
  initialized after unrealizing the window Tauri already realized
  (`overlay.rs`).
- Keep the window **opaque**. A transparent window (for rounded corners and
  a shadow) was tried and the panel could not be confirmed to show.
- **No closing animation.** When the panel is shown again, KWin can first
  display the last frame drawn before it was hidden, and the web view does
  not always repaint at once. A fade-out left that last frame transparent,
  so the panel looked like it never opened from the tray or the screenshot
  shortcut. The opening animation starts from 55% opacity for the same
  reason (`src/app.css`).
- To check whether the panel is really on screen without capturing the
  screen, ask KWin. Save this as `probe.js`:

  ```js
  for (const w of workspace.stackingOrder) {
    if (w.resourceClass === "quest-backlog-desktop")
      console.info("QBPROBE", "layer=" + w.layer, "hidden=" + w.hidden, JSON.stringify(w.frameGeometry));
  }
  ```

  then run it and read the result in the journal:

  ```bash
  id=$(qdbus6 org.kde.KWin /Scripting org.kde.kwin.Scripting.loadScript "$PWD/probe.js" qbprobe)
  qdbus6 org.kde.KWin /Scripting/Script$id org.kde.kwin.Script.run
  qdbus6 org.kde.KWin /Scripting org.kde.kwin.Scripting.unloadScript qbprobe
  journalctl --user --since -10s -o cat | grep QBPROBE
  ```

- The tray's left click can be triggered like Plasma does:
  `gdbus call --session --dest org.kde.StatusNotifierItem-<pid>-1
  --object-path /StatusNotifierItem --method
  org.kde.StatusNotifierItem.Activate 0 0`.
- Global shortcuts: the app id must not end in `.desktop`, and the portal
  needs the installed launcher entry (`install-local.sh` writes it).

Releases and CI (`.github/workflows/release.yml`):

- Every pull request (and a manual run) typechecks, runs the Rust tests
  and builds the installers on `windows-latest` and `ubuntu-22.04`. The
  installers are kept as workflow artifacts (`quest-backlog-Windows`,
  `quest-backlog-Linux`), so a PR shows whether Windows still compiles.
- To cut a release: set the same version in `src-tauri/tauri.conf.json`,
  `src-tauri/Cargo.toml` (then `cargo check` to update `Cargo.lock`) and
  `package.json`, merge, then tag that commit `desktop-vX.Y.Z` and push the
  tag. The workflow refuses a tag that does not match the version. It
  creates a **draft** release with the Windows NSIS installer, the AppImage
  and the `.deb`; check them, edit the notes and publish the draft by hand.
  Re-running the workflow for the same tag reuses the draft.
- Linux bundles are built on Ubuntu 22.04 so they run on older glibc too.
  The `.deb` depends on `libgtk-layer-shell0`; its launcher entry comes from
  `src-tauri/linux/desktop.hbs` (declares KWin's screenshot interface).
- Nothing is code signed. Signing on Windows would need a certificate and a
  secret in the workflow; until then SmartScreen warns.

Servers and builds:

- The server address is fixed at build time. A build without
  `QB_API_BASE_URL` / `QB_WEB_BASE_URL` talks to production; `cargo` does
  rebuild when they change. A device token from one server is invalid on
  another, so switching servers means signing in again.
- Check which server a binary uses:
  `strings ~/.local/bin/quest-backlog-desktop | grep -o 'https://quest-backlog.com/api'`.
- A release build (`install-local.sh`) takes about 75 seconds after the
  first one.

Working on the interface:

- The web view only talks to Rust through `window.__TAURI_INTERNALS__`, so
  the panel can be previewed in a browser with `pnpm vite` and a small
  script that defines `__TAURI_INTERNALS__.invoke` (returning canned
  `get_app_info`, `get_auth`, `detect_game`, … results) and
  `transformCallback`, injected before the page's own scripts. Size the
  viewport to the window (560 x 600) and check that nothing scrolls.
- Style rules (from the Quest Backlog web redesign): flat surfaces, one
  accent for the primary action, status colors only for statuses, sentence
  case, no uppercase labels, quiet motion, cover art carries the color.

## Not done yet

- Windows: built by CI, never run. Detection (registry, foreground window),
  capture, tray, shortcut, keychain, the installer and the icon are
  untested there.
- Installers: built and attached to draft releases by CI, but not code
  signed (Windows SmartScreen warns about an unknown publisher). No
  download page on the website yet.
- Packaged Linux builds (AppImage, `.deb`) are untested: their launcher
  entry is not named `com.questbacklog.QuestBacklog.desktop`, so on Wayland
  the shortcut portal may file the shortcuts under another name; use
  `scripts/install-local.sh` on KDE for now.
- No update notice or auto-updater; a new version means reinstalling.
- Showing the panel in the taskbar while open: possible on Windows; not on
  KDE Wayland without giving up the overlay layer.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

Quest Backlog's server and website are not part of this repository.
