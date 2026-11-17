//! Sign-in state. The device authorization flow (see README.md, "Sign-in")
//! runs here; the webview only ever sees `AuthView`, never the
//! token, and is told about changes through the `auth://changed` event.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::api::{Api, ApiError, PollOutcome, User};
use crate::{config, device, overlay, secret};

pub const CHANGED_EVENT: &str = "auth://changed";

/// What the UI shows. `error` values are message keys the UI translates.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AuthView {
    Loading,
    SignedOut { error: Option<&'static str> },
    Pending { user_code: String },
    SignedIn { user: User },
    /// A token is stored but the server could not be reached.
    Offline,
}

struct Inner {
    view: AuthView,
    token: Option<String>,
    link_url: Option<String>,
    /// Bumped on every new sign-in, cancel or sign-out so a stale polling
    /// task knows to stop.
    attempt: u64,
}

pub struct Auth {
    api: Api,
    inner: Mutex<Inner>,
}

impl Auth {
    pub fn new(api: Api) -> Self {
        Self {
            api,
            inner: Mutex::new(Inner {
                view: AuthView::Loading,
                token: None,
                link_url: None,
                attempt: 0,
            }),
        }
    }

    pub fn view(&self) -> AuthView {
        self.inner.lock().unwrap().view.clone()
    }

    pub fn api(&self) -> &Api {
        &self.api
    }

    /// The device token while signed in (or offline with a stored token).
    pub fn token(&self) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        match inner.view {
            AuthView::SignedIn { .. } | AuthView::Offline => inner.token.clone(),
            _ => None,
        }
    }

    pub fn link_url(&self) -> Option<String> {
        self.inner.lock().unwrap().link_url.clone()
    }

    fn attempt(&self) -> u64 {
        self.inner.lock().unwrap().attempt
    }

    /// Applies `change` and tells the UI, but only while `attempt` is still
    /// the current one (`None` applies unconditionally).
    fn update(&self, app: &AppHandle, attempt: Option<u64>, change: impl FnOnce(&mut Inner)) -> bool {
        let view = {
            let mut inner = self.inner.lock().unwrap();
            if attempt.is_some_and(|attempt| attempt != inner.attempt) {
                return false;
            }
            change(&mut inner);
            inner.view.clone()
        };
        let _ = app.emit(CHANGED_EVENT, view);
        true
    }

    fn signed_out(&self, app: &AppHandle, attempt: Option<u64>, error: Option<&'static str>) {
        self.update(app, attempt, |inner| {
            inner.view = AuthView::SignedOut { error };
            inner.token = None;
            inner.link_url = None;
        });
    }
}

fn auth(app: &AppHandle) -> tauri::State<'_, Auth> {
    app.state::<Auth>()
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or_else(|error| Err(error.to_string()))
}

/// Loads the stored token and checks it with the server. Returns whether a
/// token was stored at all (first run shows the overlay so the user can sign in).
pub async fn restore(app: &AppHandle) -> bool {
    match blocking(secret::load).await {
        Ok(Some(token)) => {
            check_token(app, token).await;
            true
        }
        Ok(None) => {
            auth(app).signed_out(app, None, None);
            false
        }
        Err(error) => {
            eprintln!("[auth] keychain read failed: {error}");
            auth(app).signed_out(app, None, Some("keychain"));
            false
        }
    }
}

/// Re-checks the stored token, for the Offline view's retry button.
pub async fn retry(app: &AppHandle) {
    let token = auth(app).inner.lock().unwrap().token.clone();
    match token {
        Some(token) => check_token(app, token).await,
        None => {
            restore(app).await;
        }
    }
}

async fn check_token(app: &AppHandle, token: String) {
    let state = auth(app);
    match state.api.me(&token).await {
        Ok(Some(user)) => {
            state.update(app, None, |inner| {
                inner.view = AuthView::SignedIn { user };
                inner.token = Some(token);
            });
        }
        Ok(None) => {
            // Revoked from Connected devices, expired, or the account is gone.
            let _ = blocking(secret::clear).await;
            state.signed_out(app, None, Some("revoked"));
        }
        Err(_) => {
            state.update(app, None, |inner| {
                inner.view = AuthView::Offline;
                inner.token = Some(token);
            });
        }
    }
}

pub async fn start_sign_in(app: &AppHandle) {
    let state = auth(app);
    let start = match state.api.start_device(&device::name(), &device::os()).await {
        Ok(start) => start,
        Err(error) => {
            state.signed_out(app, None, Some(error.key()));
            return;
        }
    };

    let link_url = format!("{}/link?code={}", config::web_base(), start.user_code);
    let mut attempt = 0;
    state.update(app, None, |inner| {
        inner.attempt += 1;
        attempt = inner.attempt;
        inner.view = AuthView::Pending {
            user_code: start.user_code.clone(),
        };
        inner.link_url = Some(link_url);
    });

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        poll(
            handle,
            attempt,
            start.device_code,
            start.interval_seconds,
            start.expires_in_seconds,
        )
        .await;
    });
}

async fn poll(app: AppHandle, attempt: u64, device_code: String, interval: u64, expires_in: u64) {
    let deadline = Instant::now() + Duration::from_secs(expires_in);
    let interval = Duration::from_secs(interval.max(5));

    loop {
        tokio::time::sleep(interval).await;
        let state = auth(&app);
        if state.attempt() != attempt {
            return;
        }
        if Instant::now() >= deadline {
            state.signed_out(&app, Some(attempt), Some("expired"));
            return;
        }

        match state.api.poll_device(&device_code).await {
            Ok(PollOutcome::Pending) | Err(ApiError::Network | ApiError::RateLimited) => continue,
            Ok(PollOutcome::Denied) => {
                state.signed_out(&app, Some(attempt), Some("denied"));
                return;
            }
            Ok(PollOutcome::Expired) => {
                state.signed_out(&app, Some(attempt), Some("expired"));
                return;
            }
            Err(_) => {
                state.signed_out(&app, Some(attempt), Some("failed"));
                return;
            }
            Ok(PollOutcome::Approved { token, user }) => {
                finish_sign_in(&app, attempt, token, user).await;
                return;
            }
        }
    }
}

async fn finish_sign_in(app: &AppHandle, attempt: u64, token: String, user: User) {
    let state = auth(app);
    let stored = token.clone();
    if let Err(error) = blocking(move || secret::save(&stored)).await {
        // Without a keychain there is nowhere safe to keep it: revoke it.
        eprintln!("[auth] keychain write failed: {error}");
        let _ = state.api.logout(&token).await;
        state.signed_out(app, Some(attempt), Some("keychain"));
        return;
    }

    let applied = state.update(app, Some(attempt), |inner| {
        inner.view = AuthView::SignedIn { user };
        inner.token = Some(token.clone());
        inner.link_url = None;
    });
    if applied {
        overlay::show(app);
    } else {
        // Cancelled while the last poll was in flight.
        let _ = blocking(secret::clear).await;
        let _ = state.api.logout(&token).await;
    }
}

/// Called when the API rejects the token mid-session (revoked from
/// Connected devices, expired, account banned or deleted).
pub async fn token_rejected(app: &AppHandle) {
    let _ = blocking(secret::clear).await;
    auth(app).signed_out(app, None, Some("revoked"));
}

/// The token for an API call, or the UI key for "signed out".
pub fn required_token(app: &AppHandle) -> Result<String, String> {
    auth(app).token().ok_or_else(|| "signedOut".to_string())
}

/// Maps an API error to a UI message key; a rejected token signs out.
pub async fn api_error(app: &AppHandle, error: ApiError) -> String {
    if error == ApiError::Unauthorized {
        token_rejected(app).await;
    }
    error.key().to_string()
}

pub fn cancel_sign_in(app: &AppHandle) {
    auth(app).update(app, None, |inner| {
        inner.attempt += 1;
        inner.view = AuthView::SignedOut { error: None };
        inner.link_url = None;
    });
}

pub async fn sign_out(app: &AppHandle) {
    let state = auth(app);
    let mut token = None;
    state.update(app, None, |inner| {
        inner.attempt += 1;
        token = inner.token.take();
        inner.view = AuthView::SignedOut { error: None };
        inner.link_url = None;
    });
    if let Err(error) = blocking(secret::clear).await {
        eprintln!("[auth] keychain delete failed: {error}");
    }
    if let Some(token) = token {
        // Best effort; the user can also remove the device on the web.
        let _ = state.api.logout(&token).await;
    }
}
