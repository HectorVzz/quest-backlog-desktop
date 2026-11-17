#!/usr/bin/env bash
# Builds the desktop app and installs it for the current user on Linux:
#   ~/.local/bin/quest-backlog-desktop
#   ~/.local/share/applications/com.questbacklog.QuestBacklog.desktop  (launcher entry)
#   ~/.local/share/icons/hicolor/32x32/apps/com.questbacklog.QuestBacklog.png
# The launcher entry's name must match the app identifier: it is how the
# desktop's shortcut portal knows the app (KDE then lists the shortcuts
# under "Quest Backlog" instead of the terminal that started it). It also
# declares KWin's screenshot interface, which KDE may require before it
# lets the app capture the game window (Flameshot's entry does the same).
#
# Talks to https://quest-backlog.com unless QB_API_BASE_URL and
# QB_WEB_BASE_URL are set (read at build time), e.g. for the local stack:
#   QB_API_BASE_URL=http://localhost:4000/api QB_WEB_BASE_URL=http://localhost:3000 scripts/install-local.sh
#
# scripts/install-local.sh --uninstall removes the files again (settings,
# remembered games and the keychain entry stay).
set -euo pipefail

APP_ID="com.questbacklog.QuestBacklog"
BIN_NAME="quest-backlog-desktop"
BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
DESKTOP_FILE="$DATA_DIR/applications/$APP_ID.desktop"
ICON_FILE="$DATA_DIR/icons/hicolor/32x32/apps/$APP_ID.png"

refresh_menus() {
  update-desktop-database -q "$DATA_DIR/applications" 2>/dev/null || true
}

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$BIN_DIR/$BIN_NAME" "$DESKTOP_FILE" "$ICON_FILE"
  refresh_menus
  echo "Removed Quest Backlog. Settings stay in ~/.config/$APP_ID."
  exit 0
fi

cd "$(dirname "$0")/.."

pnpm install --frozen-lockfile
pnpm tauri build --no-bundle

install -Dm755 "src-tauri/target/release/$BIN_NAME" "$BIN_DIR/$BIN_NAME"
install -Dm644 "src-tauri/icons/icon.png" "$ICON_FILE"
mkdir -p "$(dirname "$DESKTOP_FILE")"
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Quest Backlog
Comment=Update your game status and share screenshots
Exec=$BIN_DIR/$BIN_NAME
Icon=$APP_ID
Terminal=false
Categories=Game;Utility;
StartupNotify=false
X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2
EOF
refresh_menus

echo "Installed Quest Backlog for ${QB_API_BASE_URL:-https://quest-backlog.com/api}."
echo "Start it from your application launcher; it lives in the system tray."
