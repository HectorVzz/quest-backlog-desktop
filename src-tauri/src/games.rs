//! Quick status update: detect the running game, let the user correct it,
//! and save its status. All calls use the device token held by `auth`.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::api::{ApiError, Game, GameStatus, NewPost, PostError};
use crate::auth::{self, Auth};
use crate::detect::{self, Executable};
use crate::known_games;

/// The statuses the Quest Backlog API accepts.
const STATUSES: &[&str] = &["playing", "completed", "platinumed", "backlog", "dropped", "wishlist"];
/// Upper bound for hours played typed in the panel.
const MAX_HOURS: f64 = 100_000.0;

/// The viewer's entry for a game.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// `None` when the game is not in the viewer's backlog.
    pub status: Option<String>,
    pub hours_played: Option<f64>,
    /// Steam keeps the hours; the panel shows them read-only.
    pub steam_hours: bool,
}

impl From<GameStatus> for Progress {
    fn from(status: GameStatus) -> Self {
        Self {
            steam_hours: status.steam_owned(),
            hours_played: status.hours_played,
            status: Some(status.status),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Detection {
    /// A game was found, with the viewer's entry for it.
    Game {
        game: Game,
        progress: Progress,
        executable: Option<String>,
    },
    /// An executable is running but not matched yet; `query` pre-fills search.
    Unknown { executable: String, query: String },
    Nothing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chosen {
    pub game: Game,
    pub progress: Progress,
}

/// The executable from the last detection, so `choose_game` can remember
/// the user's pick for it. Kept in Rust; the webview never names files.
#[derive(Default)]
pub struct GamesState(Mutex<Option<Executable>>);

fn token(auth: &Auth) -> Result<String, String> {
    auth.token().ok_or_else(|| "signedOut".to_string())
}

async fn fail(app: &AppHandle, error: ApiError) -> String {
    auth::api_error(app, error).await
}

async fn progress_of(app: &AppHandle, token: &str, game_id: &str) -> Result<Progress, String> {
    let auth = app.state::<Auth>();
    match auth.api().game_status(token, game_id).await {
        Ok(status) => Ok(status.map(Progress::from).unwrap_or_default()),
        Err(error) => Err(fail(app, error).await),
    }
}

async fn found(app: &AppHandle, token: &str, game: Game, executable: Option<&Executable>) -> Result<Detection, String> {
    let progress = progress_of(app, token, &game.id).await?;
    Ok(Detection::Game {
        game,
        progress,
        executable: executable.map(|executable| executable.name.clone()),
    })
}

#[tauri::command]
pub async fn detect_game(app: AppHandle) -> Result<Detection, String> {
    let auth = app.state::<Auth>();
    let token = token(&auth)?;
    let running = tauri::async_runtime::spawn_blocking(detect::scan)
        .await
        .map_err(|_| "failed".to_string())?;

    let first = running.executables.first().cloned();
    *app.state::<GamesState>().0.lock().unwrap() = first.clone();

    // 1. An executable the user already matched on this computer.
    for executable in &running.executables {
        if let Some(game) = known_games::remembered(&app, &executable.key()) {
            *app.state::<GamesState>().0.lock().unwrap() = Some(executable.clone());
            return found(&app, &token, game, Some(executable)).await;
        }
    }

    // 2. A Steam game.
    for app_id in &running.steam_app_ids {
        match auth.api().game_by_steam_app(&token, *app_id).await {
            Ok(Some(game)) => return found(&app, &token, game, None).await,
            Ok(None) => continue,
            Err(error) => return Err(fail(&app, error).await),
        }
    }

    // 3. A known non-Steam game, then any other executable: ask once.
    let known = running
        .executables
        .iter()
        .find_map(|executable| {
            known_games::built_in_query(&executable.key()).map(|query| (executable, query.to_string()))
        });
    if let Some((executable, query)) = known {
        *app.state::<GamesState>().0.lock().unwrap() = Some(executable.clone());
        return Ok(Detection::Unknown { executable: executable.name.clone(), query });
    }
    if let Some(executable) = first {
        let query = known_games::fallback_query(&executable.name);
        return Ok(Detection::Unknown { executable: executable.name, query });
    }
    Ok(Detection::Nothing)
}

#[tauri::command]
pub async fn search_games(app: AppHandle, query: String) -> Result<Vec<Game>, String> {
    let auth = app.state::<Auth>();
    let token = token(&auth)?;
    let query = query.trim();
    if query.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let query: String = query.chars().take(100).collect();
    match auth.api().search_games(&token, &query).await {
        Ok(mut games) => {
            games.truncate(8);
            Ok(games)
        }
        Err(error) => Err(fail(&app, error).await),
    }
}

/// The user picked a game from search. Remembers it for the detected
/// executable, if any, and returns its current status.
#[tauri::command]
pub async fn choose_game(app: AppHandle, games: State<'_, GamesState>, game: Game) -> Result<Chosen, String> {
    let token = token(&app.state::<Auth>())?;
    let executable = games.0.lock().unwrap().clone();
    if let Some(executable) = executable {
        if let Err(error) = known_games::remember(&app, &executable.key(), game.clone()) {
            eprintln!("[games] could not remember {}: {error}", executable.name);
        }
    }
    let progress = progress_of(&app, &token, &game.id).await?;
    Ok(Chosen { game, progress })
}

/// Hours as typed: finite, not negative, at most `MAX_HOURS`, rounded to
/// a tenth. Errors are UI message keys.
fn valid_hours(hours: f64) -> Result<f64, &'static str> {
    if !hours.is_finite() || !(0.0..=MAX_HOURS).contains(&hours) {
        return Err("hoursInvalid");
    }
    Ok((hours * 10.0).round() / 10.0)
}

fn valid_game(game_id: &str, status: &str) -> Result<(), String> {
    if !STATUSES.contains(&status) || game_id.trim().is_empty() {
        return Err("failed".to_string());
    }
    Ok(())
}

/// Saves the status, and the hours played when given (`None` keeps them).
#[tauri::command]
pub async fn set_game_status(
    app: AppHandle,
    game_id: String,
    status: String,
    hours_played: Option<f64>,
) -> Result<Progress, String> {
    let auth = app.state::<Auth>();
    let token = token(&auth)?;
    valid_game(&game_id, &status)?;
    let hours = hours_played.map(valid_hours).transpose()?;
    match auth.api().set_game_status(&token, game_id.trim(), &status, hours).await {
        Ok(saved) => Ok(saved.into()),
        Err(error) => Err(fail(&app, error).await),
    }
}

/// Shares the game's status to the feed as a status update post, like the
/// website's "Share to feed". Returns the post id.
#[tauri::command]
pub async fn share_status(
    app: AppHandle,
    game_id: String,
    status: String,
    note: String,
    hours_played: Option<f64>,
) -> Result<String, String> {
    let auth = app.state::<Auth>();
    let token = token(&auth)?;
    valid_game(&game_id, &status)?;
    let note = note.trim();
    if note.is_empty() {
        return Err("shareEmpty".to_string());
    }
    if note.chars().count() > crate::compose::POST_MAX_CHARS {
        return Err("postTooLong".to_string());
    }
    let hours = hours_played.map(valid_hours).transpose()?;
    let post = NewPost {
        content: note,
        game_id: Some(game_id.trim()),
        spoiler: false,
        image: None,
        status: Some(&status),
        hours_played: hours,
    };
    match auth.api().create_post(&token, post).await {
        Ok(created) => Ok(created.id),
        Err(PostError::Rejected(message)) => Err(format!("server:{message}")),
        Err(PostError::Api(error)) => Err(fail(&app, error).await),
    }
}

#[cfg(test)]
mod tests {
    use super::valid_hours;

    #[test]
    fn hours_are_checked_and_rounded() {
        assert_eq!(valid_hours(12.34), Ok(12.3));
        assert_eq!(valid_hours(0.0), Ok(0.0));
        assert_eq!(valid_hours(-1.0), Err("hoursInvalid"));
        assert_eq!(valid_hours(f64::NAN), Err("hoursInvalid"));
        assert_eq!(valid_hours(1e9), Err("hoursInvalid"));
    }
}
