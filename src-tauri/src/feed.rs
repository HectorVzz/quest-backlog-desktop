//! Reading posts in the quick panel: the home feed (For you, Following),
//! the viewer's own posts, likes, comments and deleting the viewer's posts. Everything goes through the
//! device token held by `auth`; the webview gets plain view models.
//!
//! Images are shown with `<img>` in the webview, so only addresses on the
//! Quest Backlog server (and IGDB covers) are passed on, matching the CSP in
//! tauri.conf.json. Other avatar or media hosts are dropped.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Url;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::api::{self, ApiError, Comment, Game, Post, PostError, PostPage, User};
use crate::auth::{self, Auth, AuthView};
use crate::config;

/// The API's comment length limit.
const COMMENT_MAX_CHARS: usize = 500;
/// Longest feed cursor accepted back from the webview.
const MAX_CURSOR_LEN: usize = 2048;
/// Deepest offset accepted for "My posts".
const MAX_OFFSET: u32 = 10_000;

/// The ranking seed per feed view. A first page starts a new one, as a
/// fresh load does on the website; later pages keep it so the order holds.
#[derive(Default)]
pub struct FeedState(Mutex<Seeds>);

#[derive(Default)]
struct Seeds {
    feed: String,
    following: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub url: String,
    pub aspect_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPost {
    pub id: String,
    pub author: Author,
    pub mine: bool,
    pub game: Option<Game>,
    pub content: String,
    pub spoiler: bool,
    pub created_at: String,
    /// "text", "review" or "status-update".
    pub kind: String,
    pub status: Option<String>,
    pub hours_played: Option<f64>,
    pub rating: Option<f64>,
    pub image: Option<Image>,
    /// A YouTube or Twitch video: shown as a link to the post on the web.
    pub has_video: bool,
    pub like_count: u32,
    pub liked_by_me: bool,
    pub comment_count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPage {
    pub posts: Vec<FeedPost>,
    /// Passed back to get the next page; `None` at the end.
    pub next: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Likes {
    pub like_count: u32,
    pub liked_by_me: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedComment {
    pub id: String,
    pub author: Author,
    pub mine: bool,
    pub content: String,
    pub image: Option<Image>,
    pub created_at: String,
    pub like_count: u32,
    pub liked_by_me: bool,
    pub replies: Vec<FeedComment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comments {
    pub comments: Vec<FeedComment>,
    /// All comments and replies on the post; more than shown means older
    /// ones are on the website.
    pub total: u32,
}

pub(crate) fn signed_in_user(app: &AppHandle) -> Option<User> {
    match app.state::<Auth>().view() {
        AuthView::SignedIn { user } => Some(user),
        _ => None,
    }
}

/// Keeps an image address only when the webview may load it.
pub(crate) fn allowed_image(url: Option<&str>) -> Option<String> {
    let url = Url::parse(url?.trim()).ok()?;
    let api = Url::parse(config::api_base()).ok()?;
    let same_server = url.scheme() == api.scheme()
        && url.host_str() == api.host_str()
        && url.port_or_known_default() == api.port_or_known_default();
    let igdb = url.scheme() == "https" && url.host_str() == Some("images.igdb.com");
    (same_server || igdb).then(|| url.to_string())
}

pub(crate) fn author(user: User) -> Author {
    Author {
        avatar_url: allowed_image(user.avatar_url.as_deref()),
        username: user.username,
        display_name: user.display_name,
    }
}

fn feed_post(post: Post, viewer_id: &str) -> FeedPost {
    let has_video = post.media.iter().any(|media| media.kind == "external_video");
    let image = post
        .media
        .iter()
        .find(|media| media.kind == "image")
        .and_then(|media| {
            allowed_image(Some(&media.canonical_url)).map(|url| Image {
                url,
                aspect_ratio: media.aspect_ratio,
            })
        })
        .or_else(|| {
            allowed_image(post.image_url.as_deref()).map(|url| Image { url, aspect_ratio: None })
        });
    let game = post.game.map(|game| Game {
        cover_image_url: allowed_image(game.cover_image_url.as_deref()),
        ..game
    });
    FeedPost {
        mine: post.author.id == viewer_id,
        author: author(post.author),
        id: post.id,
        game,
        content: post.content,
        spoiler: post.spoiler,
        created_at: post.created_at,
        kind: post.kind,
        status: post.status,
        hours_played: post.hours_played,
        rating: post.rating,
        image,
        has_video,
        like_count: post.like_count,
        liked_by_me: post.liked_by_me,
        comment_count: post.comment_count,
    }
}

fn feed_comment(comment: Comment, viewer_id: &str) -> FeedComment {
    FeedComment {
        mine: comment.author.id == viewer_id,
        author: author(comment.author),
        id: comment.id,
        content: comment.content.unwrap_or_default(),
        image: comment
            .image
            .and_then(|image| allowed_image(Some(&image.url)))
            .map(|url| Image { url, aspect_ratio: None }),
        created_at: comment.created_at,
        like_count: comment.like_count,
        liked_by_me: comment.liked_by_me,
        replies: comment
            .replies
            .into_iter()
            .map(|reply| feed_comment(reply, viewer_id))
            .collect(),
    }
}

fn page(page: PostPage, viewer_id: &str, next: Option<String>) -> FeedPage {
    FeedPage {
        posts: page.items.into_iter().map(|post| feed_post(post, viewer_id)).collect(),
        next: if page.has_more { next } else { None },
    }
}

fn new_seed() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}")
}

pub(crate) async fn fail(app: &AppHandle, error: ApiError) -> String {
    auth::api_error(app, error).await
}

/// A page of the home feed. `view` is "feed" or "following"; `next` is the
/// value a previous page returned, or nothing for the first page.
#[tauri::command]
pub async fn feed_page(
    app: AppHandle,
    state: State<'_, FeedState>,
    view: String,
    next: Option<String>,
) -> Result<FeedPage, String> {
    let token = auth::required_token(&app)?;
    let viewer = signed_in_user(&app).ok_or("signedOut")?;
    if let Some(cursor) = &next {
        if cursor.is_empty() || cursor.len() > MAX_CURSOR_LEN || !cursor.chars().all(|c| c.is_ascii_graphic()) {
            return Err("failed".to_string());
        }
    }
    let seed = {
        let mut seeds = state.0.lock().unwrap();
        let seed = match view.as_str() {
            "feed" => &mut seeds.feed,
            "following" => &mut seeds.following,
            _ => return Err("failed".to_string()),
        };
        if next.is_none() || seed.is_empty() {
            *seed = new_seed();
        }
        seed.clone()
    };
    match app.state::<Auth>().api().feed_page(&token, &view, &seed, next.as_deref()).await {
        Ok(result) => {
            let cursor = result.next_cursor.clone();
            Ok(page(result, &viewer.id, cursor))
        }
        Err(error) => Err(fail(&app, error).await),
    }
}

/// A page of the signed-in user's own posts.
#[tauri::command]
pub async fn my_posts_page(app: AppHandle, next: Option<String>) -> Result<FeedPage, String> {
    let token = auth::required_token(&app)?;
    let viewer = signed_in_user(&app).ok_or("signedOut")?;
    let offset = match next.as_deref() {
        None => 0,
        Some(value) => value
            .parse::<u32>()
            .ok()
            .filter(|offset| *offset <= MAX_OFFSET)
            .ok_or("failed")?,
    };
    match app.state::<Auth>().api().user_posts_page(&token, &viewer.id, offset).await {
        Ok(result) => {
            let next = result.next_offset.map(|offset| offset.to_string());
            Ok(page(result, &viewer.id, next))
        }
        Err(error) => Err(fail(&app, error).await),
    }
}

#[tauri::command]
pub async fn like_post(app: AppHandle, post_id: String, liked: bool) -> Result<Likes, String> {
    let token = auth::required_token(&app)?;
    if !api::valid_id(&post_id) {
        return Err("failed".to_string());
    }
    match app.state::<Auth>().api().set_post_like(&token, &post_id, liked).await {
        Ok(post) => Ok(Likes {
            like_count: post.like_count,
            liked_by_me: post.liked_by_me,
        }),
        Err(error) => Err(fail(&app, error).await),
    }
}

/// One post, for opening it from a notification. `postGone` when it was
/// deleted or is no longer visible.
#[tauri::command]
pub async fn get_post(app: AppHandle, post_id: String) -> Result<FeedPost, String> {
    let token = auth::required_token(&app)?;
    let viewer = signed_in_user(&app).ok_or("signedOut")?;
    if !api::valid_id(&post_id) {
        return Err("failed".to_string());
    }
    match app.state::<Auth>().api().post(&token, &post_id).await {
        Ok(Some(post)) => Ok(feed_post(post, &viewer.id)),
        Ok(None) => Err("postGone".to_string()),
        Err(error) => Err(fail(&app, error).await),
    }
}

/// Deletes one of the viewer's own posts.
#[tauri::command]
pub async fn delete_post(app: AppHandle, post_id: String) -> Result<(), String> {
    let token = auth::required_token(&app)?;
    if !api::valid_id(&post_id) {
        return Err("failed".to_string());
    }
    match app.state::<Auth>().api().delete_post(&token, &post_id).await {
        Ok(()) => Ok(()),
        Err(PostError::Rejected(message)) => Err(format!("server:{message}")),
        Err(PostError::Api(error)) => Err(fail(&app, error).await),
    }
}

#[tauri::command]
pub async fn like_comment(app: AppHandle, comment_id: String, liked: bool) -> Result<Likes, String> {
    let token = auth::required_token(&app)?;
    if !api::valid_id(&comment_id) {
        return Err("failed".to_string());
    }
    match app.state::<Auth>().api().set_comment_like(&token, &comment_id, liked).await {
        Ok(comment) => Ok(Likes {
            like_count: comment.like_count,
            liked_by_me: comment.liked_by_me,
        }),
        Err(error) => Err(fail(&app, error).await),
    }
}

#[tauri::command]
pub async fn post_comments(app: AppHandle, post_id: String) -> Result<Comments, String> {
    let token = auth::required_token(&app)?;
    let viewer = signed_in_user(&app).ok_or("signedOut")?;
    if !api::valid_id(&post_id) {
        return Err("failed".to_string());
    }
    match app.state::<Auth>().api().comments(&token, &post_id).await {
        Ok(result) => Ok(Comments {
            total: result.total_count,
            comments: result
                .items
                .into_iter()
                .map(|comment| feed_comment(comment, &viewer.id))
                .collect(),
        }),
        Err(error) => Err(fail(&app, error).await),
    }
}

/// Comments on a post, or replies to a top-level comment (`parent_id`).
#[tauri::command]
pub async fn add_comment(
    app: AppHandle,
    post_id: String,
    content: String,
    parent_id: Option<String>,
) -> Result<FeedComment, String> {
    let token = auth::required_token(&app)?;
    let viewer = signed_in_user(&app).ok_or("signedOut")?;
    let ids_valid = api::valid_id(&post_id) && parent_id.as_deref().is_none_or(api::valid_id);
    if !ids_valid {
        return Err("failed".to_string());
    }
    let content = content.trim();
    if content.is_empty() {
        return Err("commentEmpty".to_string());
    }
    if content.chars().count() > COMMENT_MAX_CHARS {
        return Err("commentTooLong".to_string());
    }
    match app
        .state::<Auth>()
        .api()
        .create_comment(&token, &post_id, content, parent_id.as_deref())
        .await
    {
        Ok(comment) => Ok(feed_comment(comment, &viewer.id)),
        Err(PostError::Rejected(message)) => Err(format!("server:{message}")),
        Err(PostError::Api(error)) => Err(fail(&app, error).await),
    }
}

#[cfg(test)]
mod tests {
    use super::{allowed_image, feed_comment};
    use crate::api::Comment;

    #[test]
    fn comment_mapping_keeps_likes_and_ownership() {
        let comment: Comment = serde_json::from_str(
            r#"{"id":"c1","content":"hi","likeCount":2,"likedByMe":true,"createdAt":"2026-11-20T10:00:00.000Z",
            "author":{"id":"me","username":"me","displayName":"Me","avatarUrl":"https://example.com/a.png"},
            "replies":[{"id":"c2","content":null,"likeCount":1,"likedByMe":false,"createdAt":"2026-11-20T10:01:00.000Z",
            "author":{"id":"u2","username":"bo","displayName":"Bo"}}]}"#,
        )
        .unwrap();
        let mapped = feed_comment(comment, "me");
        assert!(mapped.mine);
        assert_eq!((mapped.like_count, mapped.liked_by_me), (2, true));
        assert_eq!(mapped.author.avatar_url, None);
        let reply = &mapped.replies[0];
        assert!(!reply.mine);
        assert_eq!(reply.content, "");
        assert_eq!((reply.like_count, reply.liked_by_me), (1, false));
    }

    #[test]
    fn only_server_and_igdb_images_pass() {
        let base = crate::config::api_base();
        let origin = base.trim_end_matches("/api");
        let upload = format!("{origin}/api/uploads/post-images/a.webp");
        assert_eq!(allowed_image(Some(&upload)).as_deref(), Some(upload.as_str()));
        assert!(allowed_image(Some("https://images.igdb.com/igdb/image/upload/t_cover_big/x.jpg")).is_some());
        assert_eq!(allowed_image(Some("https://example.com/avatar.png")), None);
        assert_eq!(allowed_image(Some("http://images.igdb.com/x.jpg")), None);
        assert_eq!(allowed_image(Some("not a url")), None);
        assert_eq!(allowed_image(None), None);
    }
}
