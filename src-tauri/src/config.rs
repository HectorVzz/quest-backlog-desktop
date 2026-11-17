//! Server addresses, fixed at build time. Release builds use production;
//! local development sets `QB_API_BASE_URL` and `QB_WEB_BASE_URL` when
//! running `pnpm tauri dev` (see README.md).

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn api_base() -> &'static str {
    option_env!("QB_API_BASE_URL").unwrap_or("https://quest-backlog.com/api")
}

pub fn web_base() -> &'static str {
    option_env!("QB_WEB_BASE_URL").unwrap_or("https://quest-backlog.com")
}
