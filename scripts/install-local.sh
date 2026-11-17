#!/usr/bin/env bash
# Builds the desktop app and installs it for the current user on Linux:
#   ~/.local/bin/quest-backlog-desktop
#   ~/.local/share/applications/com.questbacklog.QuestBacklog.desktop  (launcher entry)
#   ~/.local/share/icons/hicolor/<size>/apps/com.questbacklog.QuestBacklog.{png,svg}
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
ICON_DIR="$DATA_DIR/icons/hicolor"
ICON_SIZES=(16 32 48 64 128 256 512)

icon_files() {
  for size in "${ICON_SIZES[@]}"; do
    echo "$ICON_DIR/${size}x${size}/apps/$APP_ID.png"
  done
  echo "$ICON_DIR/scalable/apps/$APP_ID.svg"
}

refresh_menus() {
  update-desktop-database -q "$DATA_DIR/applications" 2>/dev/null || true
  gtk-update-icon-cache -q -t "$ICON_DIR" 2>/dev/null || true
}

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$BIN_DIR/$BIN_NAME" "$DESKTOP_FILE"
  icon_files | xargs rm -f
  refresh_menus
  echo "Removed Quest Backlog. Settings stay in ~/.config/$APP_ID."
  exit 0
fi

cd "$(dirname "$0")/.."

pnpm install --frozen-lockfile
pnpm tauri build --no-bundle

install -Dm755 "src-tauri/target/release/$BIN_NAME" "$BIN_DIR/$BIN_NAME"
for size in "${ICON_SIZES[@]}"; do
  source="src-tauri/icons/${size}x${size}.png"
  [[ $size == 512 ]] && source="src-tauri/icons/icon.png"
  install -Dm644 "$source" "$ICON_DIR/${size}x${size}/apps/$APP_ID.png"
done
install -Dm644 "src-tauri/icons/icon.svg" "$ICON_DIR/scalable/apps/$APP_ID.svg"
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
