//! Which executable is which game.
//!
//! - Remembered: the user's own choices, stored on this computer only in
//!   `games.json` next to `settings.json`. The executable names are never
//!   sent to the server. The stored game is looked up again by its slug each
//!   time, since its id is the server's (it differs between servers, and a
//!   game can be re-imported).
//! - Built in: common non-Steam games. Only used to pre-fill the search;
//!   the user always confirms the match once, and that is remembered.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::api::Game;

/// Executable (lowercase) -> search text.
const BUILT_IN: &[(&str, &str)] = &[
    ("wow.exe", "World of Warcraft"),
    ("wowclassic.exe", "World of Warcraft Classic"),
    ("diablo iv.exe", "Diablo IV"),
    ("diablo iii64.exe", "Diablo III"),
    ("d2r.exe", "Diablo II: Resurrected"),
    ("overwatch.exe", "Overwatch 2"),
    ("hearthstone.exe", "Hearthstone"),
    ("heroesofthestorm_x64.exe", "Heroes of the Storm"),
    ("sc2_x64.exe", "StarCraft II"),
    ("league of legends.exe", "League of Legends"),
    ("valorant-win64-shipping.exe", "Valorant"),
    ("fortniteclient-win64-shipping.exe", "Fortnite"),
    ("minecraft.windows.exe", "Minecraft"),
    ("genshinimpact.exe", "Genshin Impact"),
    ("starrail.exe", "Honkai: Star Rail"),
    ("zenlesszonezero.exe", "Zenless Zone Zero"),
    ("gw2-64.exe", "Guild Wars 2"),
    ("ffxiv_dx11.exe", "Final Fantasy XIV Online"),
    ("pathofexile_x64.exe", "Path of Exile"),
    ("pathofexile2.exe", "Path of Exile 2"),
    ("gta5.exe", "Grand Theft Auto V"),
    ("rdr2.exe", "Red Dead Redemption 2"),
    ("rocketleague.exe", "Rocket League"),
];

pub fn built_in_query(key: &str) -> Option<&'static str> {
    BUILT_IN
        .iter()
        .find(|(executable, _)| *executable == key)
        .map(|(_, query)| *query)
}

/// Search text for an unknown executable: "EldenRing_x64.exe" -> "EldenRing".
pub fn fallback_query(name: &str) -> String {
    let stem = name.strip_suffix(".exe").or_else(|| name.strip_suffix(".EXE")).unwrap_or(name);
    let mut stem = stem.to_string();
    for suffix in ["-Win64-Shipping", "-win64-shipping", "_x64", "-x64", "x64", "_dx11", "_dx12"] {
        if let Some(trimmed) = stem.strip_suffix(suffix) {
            stem = trimmed.to_string();
        }
    }
    stem.replace(['_', '-'], " ").trim().to_string()
}

#[derive(Default, Serialize, Deserialize)]
struct Remembered {
    #[serde(default)]
    games: BTreeMap<String, Game>,
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("games.json"))
}

fn read(app: &AppHandle) -> Remembered {
    path(app)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn remembered(app: &AppHandle, key: &str) -> Option<Game> {
    read(app).games.remove(key)
}

pub fn remember(app: &AppHandle, key: &str, game: Game) -> Result<(), String> {
    let mut remembered = read(app);
    remembered.games.insert(key.to_string(), game);
    write(app, &remembered)
}

/// Drops a choice whose game the server no longer has.
pub fn forget(app: &AppHandle, key: &str) -> Result<(), String> {
    let mut remembered = read(app);
    if remembered.games.remove(key).is_none() {
        return Ok(());
    }
    write(app, &remembered)
}

fn write(app: &AppHandle, remembered: &Remembered) -> Result<(), String> {
    let path = path(app).ok_or("no config directory")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(remembered).map_err(|error| error.to_string())?;
    std::fs::write(&path, text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_matches_lowercase_keys() {
        assert_eq!(built_in_query("wow.exe"), Some("World of Warcraft"));
        assert_eq!(built_in_query("hades.exe"), None);
    }

    #[test]
    fn fallback_query_cleans_executable_names() {
        assert_eq!(fallback_query("Hades.exe"), "Hades");
        assert_eq!(fallback_query("EldenRing_x64.exe"), "EldenRing");
        assert_eq!(fallback_query("Stray-Win64-Shipping.exe"), "Stray");
        assert_eq!(fallback_query("ffxiv_dx11.exe"), "ffxiv");
        assert_eq!(fallback_query("Dead_Cells.exe"), "Dead Cells");
    }
}
