//! HTTP client for the Quest Backlog API. Every request carries
//! `X-CSRF-Token: 1` (the server requires it on every write)
//! and, once signed in, `Authorization: Bearer <device token>`.

use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::{Client, RequestBuilder, Response, StatusCode, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;

use crate::config::APP_VERSION;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiError {
    /// The server could not be reached or did not answer in time.
    Network,
    /// The device token is no longer valid.
    Unauthorized,
    /// Signed in, but the account may not do this (unverified, restricted).
    Forbidden,
    RateLimited,
    /// The server answered with an unexpected status or body.
    Failed,
}

impl ApiError {
    /// Message key the UI translates (`errors` in src/i18n.ts).
    pub fn key(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Unauthorized => "revoked",
            Self::Forbidden => "forbidden",
            Self::RateLimited => "rateLimited",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub cover_image_url: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatus {
    pub status: String,
    #[serde(default)]
    pub hours_played: Option<f64>,
    #[serde(default)]
    pub synced_with_steam: bool,
    #[serde(default)]
    pub steam_app_id: Option<i64>,
}

impl GameStatus {
    /// Steam keeps this entry's hours: the server ignores hours sent
    /// without `manualProgress`, which would unlink it from Steam.
    pub fn steam_owned(&self) -> bool {
        self.synced_with_steam || self.steam_app_id.is_some_and(|id| id > 0)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatedPost {
    pub id: String,
}

pub struct NewPost<'a> {
    pub content: &'a str,
    pub game_id: Option<&'a str>,
    pub spoiler: bool,
    /// JPEG bytes.
    pub image: Option<Vec<u8>>,
    /// Makes it a status update post ("Started playing …"); needs a game.
    pub status: Option<&'a str>,
    pub hours_played: Option<f64>,
}

#[derive(Debug)]
pub enum PostError {
    Api(ApiError),
    /// The server refused the post with a message for the user (image
    /// quota, moderation, validation). The API only speaks English.
    Rejected(String),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStart {
    pub device_code: String,
    pub user_code: String,
    pub expires_in_seconds: u64,
    pub interval_seconds: u64,
}

pub enum PollOutcome {
    Pending,
    Denied,
    Expired,
    Approved { token: String, user: User },
}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
    user: User,
}

#[derive(Deserialize)]
struct ErrorBody {
    code: Option<String>,
    /// Nest sends a string, or a list of strings for validation errors.
    message: Option<serde_json::Value>,
}

impl ErrorBody {
    fn message(&self) -> Option<String> {
        match self.message.as_ref()? {
            serde_json::Value::String(text) => Some(text.clone()),
            serde_json::Value::Array(items) => items.first()?.as_str().map(str::to_string),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct Api {
    client: Client,
    base: Url,
}

fn network(_: reqwest::Error) -> ApiError {
    ApiError::Network
}

fn bearer(request: RequestBuilder, token: &str) -> RequestBuilder {
    request.header(AUTHORIZATION, format!("Bearer {token}"))
}

fn status_error(status: StatusCode) -> ApiError {
    match status {
        StatusCode::UNAUTHORIZED => ApiError::Unauthorized,
        StatusCode::FORBIDDEN => ApiError::Forbidden,
        StatusCode::TOO_MANY_REQUESTS => ApiError::RateLimited,
        _ => ApiError::Failed,
    }
}

async fn json<T: DeserializeOwned>(response: Response) -> Result<T, ApiError> {
    if !response.status().is_success() {
        return Err(status_error(response.status()));
    }
    response.json::<T>().await.map_err(|_| ApiError::Failed)
}

/// Nest answers a `null` return value with an empty body.
async fn optional_json<T: DeserializeOwned>(response: Response) -> Result<Option<T>, ApiError> {
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(status_error(response.status()));
    }
    let body = response.text().await.map_err(network)?;
    let body = body.trim();
    if body.is_empty() || body == "null" {
        return Ok(None);
    }
    serde_json::from_str(body).map(Some).map_err(|_| ApiError::Failed)
}

impl Api {
    pub fn new(base: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("X-CSRF-Token", HeaderValue::from_static("1"));
        let client = Client::builder()
            .user_agent(format!("QuestBacklogDesktop/{APP_VERSION}"))
            .default_headers(headers)
            .timeout(Duration::from_secs(15))
            .build()
            .expect("HTTP client");
        Self {
            client,
            base: Url::parse(base).expect("valid API base URL"),
        }
    }

    /// Base URL plus path segments, each percent-encoded.
    fn endpoint(&self, segments: &[&str]) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("API base URL has a path")
            .pop_if_empty()
            .extend(segments);
        url
    }

    pub async fn start_device(
        &self,
        device_name: &str,
        device_os: &str,
    ) -> Result<DeviceStart, ApiError> {
        let response = self
            .client
            .post(self.endpoint(&["auth", "device", "start"]))
            .json(&json!({
                "deviceName": device_name,
                "deviceOs": device_os,
                "appVersion": APP_VERSION,
            }))
            .send()
            .await
            .map_err(network)?;
        json(response).await
    }

    pub async fn poll_device(&self, device_code: &str) -> Result<PollOutcome, ApiError> {
        let response = self
            .client
            .post(self.endpoint(&["auth", "device", "token"]))
            .json(&json!({ "deviceCode": device_code }))
            .send()
            .await
            .map_err(network)?;

        match response.status() {
            status if status.is_success() => {
                let body: TokenResponse = json(response).await?;
                Ok(PollOutcome::Approved {
                    token: body.token,
                    user: body.user,
                })
            }
            // Rate limited: treat like "not yet" and keep the normal interval.
            StatusCode::TOO_MANY_REQUESTS => Ok(PollOutcome::Pending),
            StatusCode::BAD_REQUEST => {
                let body: ErrorBody = response.json().await.map_err(|_| ApiError::Failed)?;
                match body.code.as_deref() {
                    Some("authorization_pending") => Ok(PollOutcome::Pending),
                    Some("access_denied") => Ok(PollOutcome::Denied),
                    Some("expired_token") => Ok(PollOutcome::Expired),
                    _ => Err(ApiError::Failed),
                }
            }
            _ => Err(ApiError::Failed),
        }
    }

    /// The signed-in user, or `None` when the token is no longer valid.
    pub async fn me(&self, token: &str) -> Result<Option<User>, ApiError> {
        let response = bearer(self.client.get(self.endpoint(&["auth", "me"])), token)
            .send()
            .await
            .map_err(network)?;
        match optional_json(response).await {
            Err(ApiError::Unauthorized) => Ok(None),
            other => other,
        }
    }

    /// Ends this device's session on the server (it disappears from
    /// Connected devices).
    pub async fn logout(&self, token: &str) -> Result<(), ApiError> {
        let response = bearer(self.client.post(self.endpoint(&["auth", "logout"])), token)
            .send()
            .await
            .map_err(network)?;
        if response.status().is_success() || response.status() == StatusCode::UNAUTHORIZED {
            Ok(())
        } else {
            Err(ApiError::Failed)
        }
    }

    /// The game for a Steam app id, imported from IGDB by the server if
    /// needed. `None` when IGDB has no match.
    pub async fn game_by_steam_app(&self, token: &str, app_id: u32) -> Result<Option<Game>, ApiError> {
        let url = self.endpoint(&["games", "by-steam-app", &app_id.to_string()]);
        let response = bearer(self.client.get(url), token)
            .send()
            .await
            .map_err(network)?;
        optional_json(response).await
    }

    pub async fn search_games(&self, token: &str, query: &str) -> Result<Vec<Game>, ApiError> {
        let mut url = self.endpoint(&["games", "search"]);
        url.query_pairs_mut().append_pair("q", query);
        let response = bearer(self.client.get(url), token)
            .send()
            .await
            .map_err(network)?;
        json(response).await
    }

    /// The viewer's status for a game, `None` when it is not tracked.
    pub async fn game_status(&self, token: &str, game_id: &str) -> Result<Option<GameStatus>, ApiError> {
        let url = self.endpoint(&["statuses", "game", game_id]);
        let response = bearer(self.client.get(url), token)
            .send()
            .await
            .map_err(network)?;
        optional_json(response).await
    }

    /// Saves the status, and the hours played when `hours_played` is set
    /// (otherwise the server keeps them).
    pub async fn set_game_status(
        &self,
        token: &str,
        game_id: &str,
        status: &str,
        hours_played: Option<f64>,
    ) -> Result<GameStatus, ApiError> {
        let mut body = json!({ "gameId": game_id, "status": status });
        if let Some(hours) = hours_played {
            body["hoursPlayed"] = json!(hours);
        }
        let response = bearer(self.client.post(self.endpoint(&["statuses"])), token)
            .json(&body)
            .send()
            .await
            .map_err(network)?;
        json(response).await
    }

    /// `POST /api/posts` as multipart, like the web composer.
    pub async fn create_post(&self, token: &str, post: NewPost<'_>) -> Result<CreatedPost, PostError> {
        use reqwest::multipart::{Form, Part};

        let kind = if post.status.is_some() { "status-update" } else { "text" };
        let mut form = Form::new()
            .text("content", post.content.to_string())
            .text("type", kind)
            .text("spoiler", post.spoiler.to_string());
        if let Some(game_id) = post.game_id {
            form = form.text("gameId", game_id.to_string());
        }
        if let Some(status) = post.status {
            form = form.text("status", status.to_string());
        }
        if let Some(hours) = post.hours_played {
            form = form.text("hoursPlayed", hours.to_string());
        }
        if let Some(image) = post.image {
            let part = Part::bytes(image)
                .file_name("screenshot.jpg")
                .mime_str("image/jpeg")
                .map_err(|_| PostError::Api(ApiError::Failed))?;
            form = form.part("image", part);
        }

        let response = bearer(self.client.post(self.endpoint(&["posts"])), token)
            .multipart(form)
            // Uploads and moderation take longer than ordinary calls.
            .timeout(Duration::from_secs(60))
            .send()
            .await
            .map_err(|error| PostError::Api(network(error)))?;

        if response.status() == StatusCode::BAD_REQUEST {
            let message = response
                .json::<ErrorBody>()
                .await
                .ok()
                .and_then(|body| body.message());
            return Err(match message {
                Some(message) => PostError::Rejected(message),
                None => PostError::Api(ApiError::Failed),
            });
        }
        json(response).await.map_err(PostError::Api)
    }
}

#[cfg(test)]
mod tests {
    use super::Api;

    #[test]
    fn endpoint_appends_encoded_segments() {
        let api = Api::new("http://localhost:4000/api");
        assert_eq!(
            api.endpoint(&["statuses", "game", "a/b c"]).as_str(),
            "http://localhost:4000/api/statuses/game/a%2Fb%20c"
        );
        let api = Api::new("https://quest-backlog.com/api/");
        assert_eq!(
            api.endpoint(&["auth", "me"]).as_str(),
            "https://quest-backlog.com/api/auth/me"
        );
    }
}
