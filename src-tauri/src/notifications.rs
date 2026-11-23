//! Notifications in the quick panel: the unread count, asked for each time
//! the panel opens (never in the background), and the latest notifications
//! when their tab is opened. Opening a post notification shows the post in
//! the panel; a follow opens the follower's profile on the website.

use reqwest::Url;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::api::{self, Notification};
use crate::auth::{self, Auth};
use crate::feed::{author, fail, Author};
use crate::{config, overlay};

/// What opening a notification shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Target {
    /// The post with its comments, the comment highlighted when there is one.
    #[serde(rename_all = "camelCase")]
    Post { post_id: String, comment_id: Option<String> },
    /// The actor's profile on the website.
    Profile { username: String },
    /// Nothing to open (a follow from a deleted account).
    None,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelNotification {
    pub id: String,
    /// "like", "commentLike", "comment", "commentReply", "mention" or "follow".
    pub kind: &'static str,
    /// `None` for a deleted account.
    pub actor: Option<Author>,
    /// The start of the comment, for comments, replies and mentions.
    pub preview: Option<String>,
    pub read: bool,
    pub created_at: String,
    pub target: Target,
}

/// The webview's name for a notification type, `None` for types this
/// version does not know (they are left out rather than shown wrong).
fn kind(server: &str) -> Option<&'static str> {
    Some(match server {
        "like" => "like",
        "comment_like" => "commentLike",
        "comment" => "comment",
        "comment_reply" => "commentReply",
        "mention" => "mention",
        "follow" => "follow",
        _ => return None,
    })
}

fn panel_notification(notification: Notification) -> Option<PanelNotification> {
    let kind = kind(&notification.kind)?;
    let target = if kind == "follow" {
        match &notification.actor {
            Some(actor) if valid_username(&actor.username) => Target::Profile {
                username: actor.username.clone(),
            },
            _ => Target::None,
        }
    } else {
        match notification.post_id.filter(|id| api::valid_id(id)) {
            Some(post_id) => Target::Post {
                post_id,
                comment_id: notification.comment_id.filter(|id| api::valid_id(id)),
            },
            None => Target::None,
        }
    };
    let preview = notification
        .comment_body_preview
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    Some(PanelNotification {
        id: notification.id,
        kind,
        actor: notification.actor.map(author),
        preview,
        read: notification.read_at.is_some(),
        created_at: notification.created_at,
        target,
    })
}

/// Usernames are lowercase letters, digits and hyphens; underscores are
/// let through for older accounts. Anything else is not put in a URL.
fn valid_username(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= 64
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The web address of a profile, or `None` for an invalid username.
fn profile_url(web_base: &str, username: &str) -> Option<Url> {
    if !valid_username(username) {
        return None;
    }
    let mut url = Url::parse(web_base).ok()?;
    url.path_segments_mut().ok()?.pop_if_empty().extend(["u", username]);
    Some(url)
}

/// The number of unread notifications. Called when the panel is shown.
#[tauri::command]
pub async fn unread_notifications(app: AppHandle) -> Result<u32, String> {
    let token = auth::required_token(&app)?;
    match app.state::<Auth>().api().unread_notifications(&token).await {
        Ok(count) => Ok(count),
        Err(error) => Err(fail(&app, error).await),
    }
}

/// The latest notifications, newest first.
#[tauri::command]
pub async fn notifications(app: AppHandle) -> Result<Vec<PanelNotification>, String> {
    let token = auth::required_token(&app)?;
    match app.state::<Auth>().api().notifications(&token).await {
        Ok(list) => Ok(list.into_iter().filter_map(panel_notification).collect()),
        Err(error) => Err(fail(&app, error).await),
    }
}

#[tauri::command]
pub async fn mark_notification_read(app: AppHandle, notification_id: String) -> Result<(), String> {
    let token = auth::required_token(&app)?;
    if !api::valid_id(&notification_id) {
        return Err("failed".to_string());
    }
    match app.state::<Auth>().api().mark_notification_read(&token, &notification_id).await {
        Ok(()) => Ok(()),
        Err(error) => Err(fail(&app, error).await),
    }
}

#[tauri::command]
pub async fn mark_all_notifications_read(app: AppHandle) -> Result<(), String> {
    let token = auth::required_token(&app)?;
    match app.state::<Auth>().api().mark_all_notifications_read(&token).await {
        Ok(()) => Ok(()),
        Err(error) => Err(fail(&app, error).await),
    }
}

/// Opens a profile on the website. The overlay hides so the browser can
/// take input, as with opening a post.
#[tauri::command]
pub fn open_profile(app: AppHandle, username: String) -> Result<(), String> {
    let url = profile_url(config::web_base(), &username).ok_or("failed")?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|_| "failed".to_string())?;
    overlay::hide(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{panel_notification, profile_url, Target};
    use crate::api::Notification;

    fn parse(json: &str) -> Notification {
        serde_json::from_str(json).unwrap()
    }

    const ANA: &str = r#"{"id":"u1","username":"ana","displayName":"Ana","avatarUrl":null}"#;

    #[test]
    fn post_notifications_open_the_post() {
        let n = panel_notification(parse(&format!(
            r#"{{"id":"n1","type":"comment_reply","actor":{ANA},"postId":"p1","commentId":"c1",
            "commentBodyPreview":"  nice run  ","readAt":null,"createdAt":"2026-11-20T10:00:00.000Z"}}"#
        )))
        .unwrap();
        assert_eq!(n.kind, "commentReply");
        assert!(!n.read);
        assert_eq!(n.preview.as_deref(), Some("nice run"));
        assert_eq!(
            n.target,
            Target::Post { post_id: "p1".into(), comment_id: Some("c1".into()) }
        );

        let like = panel_notification(parse(&format!(
            r#"{{"id":"n2","type":"like","actor":{ANA},"postId":"p1","commentId":null,
            "commentBodyPreview":"","readAt":"2026-11-21T10:00:00.000Z","createdAt":"2026-11-20T10:00:00.000Z"}}"#
        )))
        .unwrap();
        assert!(like.read);
        assert_eq!(like.preview, None);
        assert_eq!(like.target, Target::Post { post_id: "p1".into(), comment_id: None });
    }

    #[test]
    fn follows_open_the_profile_unless_the_account_is_gone() {
        let follow = panel_notification(parse(&format!(
            r#"{{"id":"n3","type":"follow","actor":{ANA},"createdAt":"2026-11-20T10:00:00.000Z"}}"#
        )))
        .unwrap();
        assert_eq!(follow.target, Target::Profile { username: "ana".into() });

        let orphan = panel_notification(parse(
            r#"{"id":"n4","type":"follow","actor":null,"createdAt":"2026-11-20T10:00:00.000Z"}"#,
        ))
        .unwrap();
        assert_eq!(orphan.target, Target::None);
        assert!(orphan.actor.is_none());
    }

    #[test]
    fn unknown_types_and_bad_ids_are_not_passed_on() {
        assert!(panel_notification(parse(
            r#"{"id":"n5","type":"badge","createdAt":"2026-11-20T10:00:00.000Z"}"#
        ))
        .is_none());
        let odd = panel_notification(parse(
            r#"{"id":"n6","type":"like","postId":"../x","createdAt":"2026-11-20T10:00:00.000Z"}"#,
        ))
        .unwrap();
        assert_eq!(odd.target, Target::None);
    }

    #[test]
    fn target_serializes_for_the_webview() {
        let json = serde_json::to_value(Target::Post { post_id: "p1".into(), comment_id: None }).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "post", "postId": "p1", "commentId": null }));
        let json = serde_json::to_value(Target::None).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "none" }));
    }

    #[test]
    fn profile_urls_are_built_from_safe_usernames() {
        assert_eq!(
            profile_url("https://quest-backlog.com", "ana-1").unwrap().as_str(),
            "https://quest-backlog.com/u/ana-1"
        );
        assert_eq!(
            profile_url("http://localhost:3000/", "ana").unwrap().as_str(),
            "http://localhost:3000/u/ana"
        );
        assert!(profile_url("https://quest-backlog.com", "../admin").is_none());
        assert!(profile_url("https://quest-backlog.com", "").is_none());
    }
}
