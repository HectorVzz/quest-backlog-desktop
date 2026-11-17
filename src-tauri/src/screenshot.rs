//! Screen capture for posts. Only what the OS compositor already shows is
//! captured; nothing is injected into the game.
//!
//! The active window is captured, which is the game: the overlay is hidden
//! first and focus goes back to the window below it.
//!
//! - KDE Plasma: KWin's `org.kde.KWin.ScreenShot2.CaptureActiveWindow`,
//!   without the window frame. Silent and takes a few milliseconds. KWin
//!   may require `X-KDE-DBUS-Restricted-Interfaces` in the launcher entry,
//!   which `scripts/install-local.sh` writes.
//! - Other Linux desktops: the XDG Screenshot portal (non-interactive),
//!   which captures the whole desktop. Portals have no way to ask for the
//!   active window without a dialog.
//! - Windows: the foreground window's client area, cut from a capture of
//!   its monitor; the whole monitor when the foreground window is unusable.
//!
//! The image stays in Rust. The webview only gets a small preview.

use std::path::{Path, PathBuf};

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::DynamicImage;

/// Longest edge sent to the API. The server resizes to 1600 anyway; this
/// keeps uploads small and inside its 10 MB / 8000 px / 40 MP limits.
const UPLOAD_MAX_EDGE: u32 = 2560;
const UPLOAD_JPEG_QUALITY: u8 = 90;
const PREVIEW_WIDTH: u32 = 640;
const PREVIEW_HEIGHT: u32 = 360;

/// A captured screenshot, ready to post.
pub struct Prepared {
    pub upload_jpeg: Vec<u8>,
    pub preview_data_url: String,
}

fn encode_jpeg(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode_image(&image.to_rgb8())
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

/// CPU-heavy: call from a blocking thread.
pub fn prepare(image: &DynamicImage) -> Result<Prepared, String> {
    let upload = if image.width().max(image.height()) > UPLOAD_MAX_EDGE {
        image.resize(UPLOAD_MAX_EDGE, UPLOAD_MAX_EDGE, FilterType::Triangle)
    } else {
        image.clone()
    };
    let upload_jpeg = encode_jpeg(&upload, UPLOAD_JPEG_QUALITY)?;
    let preview = encode_jpeg(&image.thumbnail(PREVIEW_WIDTH, PREVIEW_HEIGHT), 75)?;
    let preview_data_url = format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(preview)
    );
    Ok(Prepared { upload_jpeg, preview_data_url })
}

/// Saves a full-quality PNG copy in `folder` ("Quest Backlog 2026-11-17
/// 21-04-05.png"). CPU-heavy: call from a blocking thread.
pub fn save_copy(image: &DynamicImage, folder: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let stamp = chrono::Local::now().format("%Y-%m-%d %H-%M-%S");
    let mut path = folder.join(format!("Quest Backlog {stamp}.png"));
    let mut counter = 2;
    while path.exists() {
        path = folder.join(format!("Quest Backlog {stamp} ({counter}).png"));
        counter += 1;
    }
    image
        .save_with_format(&path, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(path)
}

/// What to capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The window in front: the game, once the overlay is out of the way.
    ActiveWindow,
    /// The user clicks a window (KWin's crosshair, or the portal's dialog on
    /// other Linux desktops). Windows has no picker yet and uses the window
    /// in front.
    ChosenWindow,
}

#[derive(Debug)]
pub enum CaptureError {
    /// The user backed out of choosing a window.
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("cancelled"),
            Self::Failed(error) => formatter.write_str(error),
        }
    }
}

fn failed(error: impl std::fmt::Display) -> CaptureError {
    CaptureError::Failed(error.to_string())
}

#[cfg(target_os = "linux")]
pub async fn capture(target: Target) -> Result<DynamicImage, CaptureError> {
    match kwin::capture(target).await {
        Ok(image) => return Ok(image),
        Err(kwin::Error::Cancelled) => return Err(CaptureError::Cancelled),
        Err(kwin::Error::Unavailable) => {}
        Err(kwin::Error::Failed(error)) => {
            eprintln!("[screenshot] KWin capture failed, using the portal: {error}");
        }
    }
    capture_with_portal(target == Target::ChosenWindow).await
}

/// KDE only: the active window, or `None`. No portal fallback, because the
/// portal would capture the whole desktop, overlay included.
#[cfg(target_os = "linux")]
pub async fn capture_active_window_kwin() -> Option<DynamicImage> {
    kwin::capture(Target::ActiveWindow).await.ok()
}

#[cfg(target_os = "linux")]
async fn capture_with_portal(interactive: bool) -> Result<DynamicImage, CaptureError> {
    use ashpd::desktop::screenshot::Screenshot;
    use ashpd::desktop::ResponseError;

    let response = Screenshot::request()
        .interactive(interactive)
        .modal(false)
        .send()
        .await
        .and_then(|request| request.response())
        .map_err(|error| match error {
            ashpd::Error::Response(ResponseError::Cancelled) => CaptureError::Cancelled,
            other => failed(other),
        })?;
    let path = reqwest::Url::parse(response.uri().as_str())
        .ok()
        .and_then(|url| url.to_file_path().ok())
        .ok_or_else(|| failed(format!("unexpected screenshot location: {}", response.uri().as_str())))?;

    let decoded = tauri::async_runtime::spawn_blocking({
        let path = path.clone();
        move || image::open(&path).map_err(failed)
    })
    .await
    .map_err(failed)?;
    // The portal wrote this file for our request only; the user's own copy,
    // if they want one, is saved by `save_copy`.
    let _ = std::fs::remove_file(&path);
    decoded
}

/// KWin's screenshot interface (Plasma 6). The image comes back as raw
/// pixels through a pipe we pass in; the reply describes the layout.
#[cfg(target_os = "linux")]
mod kwin {
    use std::collections::HashMap;
    use std::io::Read;
    use std::os::fd::OwnedFd;

    use ashpd::zbus;
    use image::{DynamicImage, RgbaImage};

    use super::Target;

    pub enum Error {
        /// Not KDE, or this KWin has no ScreenShot2: use the portal.
        Unavailable,
        Cancelled,
        Failed(String),
    }

    fn failed(error: impl std::fmt::Display) -> Error {
        Error::Failed(error.to_string())
    }

    /// `CaptureInteractive`'s kind for "a window" (1 would be a screen).
    const PICK_WINDOW: u32 = 0;

    pub async fn capture(target: Target) -> Result<DynamicImage, Error> {
        let connection = zbus::Connection::session().await.map_err(|_| Error::Unavailable)?;
        let (mut reader, writer) = std::io::pipe().map_err(failed)?;
        let reading = tauri::async_runtime::spawn_blocking(move || {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).map(|_| bytes)
        });

        let options: HashMap<&str, zbus::Variant> = HashMap::from([
            ("include-decoration", false.into()),
            ("include-cursor", false.into()),
            ("native-resolution", true.into()),
        ]);
        let writer: OwnedFd = writer.into();
        let fd = zbus::Fd::from(&writer);
        const DESTINATION: Option<&str> = Some("org.kde.KWin");
        const PATH: &str = "/org/kde/KWin/ScreenShot2";
        const INTERFACE: Option<&str> = Some("org.kde.KWin.ScreenShot2");
        let reply = match target {
            Target::ActiveWindow => {
                connection
                    .call_method(DESTINATION, PATH, INTERFACE, "CaptureActiveWindow", &(options, fd))
                    .await
            }
            Target::ChosenWindow => {
                connection
                    .call_method(DESTINATION, PATH, INTERFACE, "CaptureInteractive", &(PICK_WINDOW, options, fd))
                    .await
            }
        };
        // Our copy of the write end must close, or the read never ends.
        drop(writer);
        let reply = reply.map_err(|error| match error {
            zbus::Error::MethodError(name, _, _) => match name.as_str() {
                "org.freedesktop.DBus.Error.ServiceUnknown"
                | "org.freedesktop.DBus.Error.UnknownObject"
                | "org.freedesktop.DBus.Error.UnknownInterface"
                | "org.freedesktop.DBus.Error.UnknownMethod" => Error::Unavailable,
                name if name.ends_with(".Cancelled") => Error::Cancelled,
                name => failed(name),
            },
            other => failed(other),
        })?;
        let results: HashMap<String, zbus::OwnedVariant> = reply.body().deserialize().map_err(failed)?;
        let bytes = reading.await.map_err(failed)?.map_err(failed)?;

        let number = |key: &str| -> Result<u32, Error> {
            results
                .get(key)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| failed(format!("reply has no {key}")))
        };
        to_image(number("width")?, number("height")?, number("stride")?, number("format")?, &bytes)
            .ok_or_else(|| failed("unexpected image layout"))
            .map(DynamicImage::ImageRgba8)
    }

    /// `format` is a `QImage::Format`. KWin sends 32-bit pixels: RGBA byte
    /// order (16, 17, 18) or 0xAARRGGBB words, BGRA bytes on little-endian
    /// machines (4, 5, 6). Alpha is dropped later, when encoding to JPEG.
    pub(super) fn to_image(width: u32, height: u32, stride: u32, format: u32, bytes: &[u8]) -> Option<RgbaImage> {
        let swap_red_blue = match format {
            16..=18 => false,
            4..=6 if cfg!(target_endian = "little") => true,
            _ => return None,
        };
        let (row_len, stride) = (width as usize * 4, stride as usize);
        if width == 0 || height == 0 || stride < row_len || bytes.len() < stride * (height as usize - 1) + row_len {
            return None;
        }
        let mut pixels = Vec::with_capacity(row_len * height as usize);
        for row in bytes.chunks(stride).take(height as usize) {
            for pixel in row[..row_len].chunks_exact(4) {
                let (red, blue) = if swap_red_blue { (pixel[2], pixel[0]) } else { (pixel[0], pixel[2]) };
                pixels.extend_from_slice(&[red, pixel[1], blue, 255]);
            }
        }
        RgbaImage::from_raw(width, height, pixels)
    }
}

#[cfg(target_os = "windows")]
pub async fn capture(_target: Target) -> Result<DynamicImage, CaptureError> {
    tauri::async_runtime::spawn_blocking(|| windows::capture().map(DynamicImage::ImageRgba8))
        .await
        .map_err(failed)?
        .map_err(CaptureError::Failed)
}

#[cfg(target_os = "windows")]
mod windows {
    use image::RgbaImage;
    use windows_sys::Win32::Foundation::{POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetCursorPos, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
    };

    use super::{region_within, Area};

    /// The foreground window's client area (no title bar or borders) in
    /// screen pixels, unless the window is ours or minimized.
    fn foreground_area() -> Option<Area> {
        // SAFETY: plain Win32 queries writing into the local structs.
        unsafe {
            let window = GetForegroundWindow();
            if window.is_null() || IsIconic(window) != 0 {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(window, &mut pid);
            if pid == std::process::id() {
                return None;
            }
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            let mut origin = POINT { x: 0, y: 0 };
            if GetClientRect(window, &mut rect) == 0 || ClientToScreen(window, &mut origin) == 0 {
                return None;
            }
            let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
            (width > 0 && height > 0).then(|| Area { x: origin.x, y: origin.y, width: width as u32, height: height as u32 })
        }
    }

    fn cursor() -> (i32, i32) {
        let mut cursor = POINT { x: 0, y: 0 };
        // SAFETY: writes the cursor position into `cursor`.
        unsafe { GetCursorPos(&mut cursor) };
        (cursor.x, cursor.y)
    }

    pub fn capture() -> Result<RgbaImage, String> {
        let area = foreground_area();
        let (x, y) = area.map_or_else(cursor, |area| area.center());
        let monitor = xcap::Monitor::from_point(x, y)
            .or_else(|_| {
                xcap::Monitor::all()?
                    .into_iter()
                    .find(|monitor| monitor.is_primary().unwrap_or(false))
                    .ok_or_else(|| xcap::XCapError::new("no primary monitor"))
            })
            .map_err(|error| error.to_string())?;
        let bounds = (|| {
            Ok::<_, xcap::XCapError>(Area {
                x: monitor.x()?,
                y: monitor.y()?,
                width: monitor.width()?,
                height: monitor.height()?,
            })
        })();
        if let (Some(area), Ok(bounds)) = (area, bounds) {
            if let Some((x, y, width, height)) = region_within(area, bounds) {
                return monitor.capture_region(x, y, width, height).map_err(|error| error.to_string());
            }
        }
        monitor.capture_image().map_err(|error| error.to_string())
    }
}

/// A rectangle in screen pixels.
#[derive(Clone, Copy)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
struct Area {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
impl Area {
    fn right(self) -> i64 {
        self.x as i64 + self.width as i64
    }
    fn bottom(self) -> i64 {
        self.y as i64 + self.height as i64
    }
    fn center(self) -> (i32, i32) {
        ((self.x as i64 + self.width as i64 / 2) as i32, (self.y as i64 + self.height as i64 / 2) as i32)
    }
}

/// The part of `area` on `monitor`, relative to the monitor's corner.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn region_within(area: Area, monitor: Area) -> Option<(u32, u32, u32, u32)> {
    let left = area.x.max(monitor.x);
    let top = area.y.max(monitor.y);
    let right = area.right().min(monitor.right());
    let bottom = area.bottom().min(monitor.bottom());
    (right > left as i64 && bottom > top as i64).then(|| {
        (
            (left - monitor.x) as u32,
            (top - monitor.y) as u32,
            (right - left as i64) as u32,
            (bottom - top as i64) as u32,
        )
    })
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub async fn capture(_target: Target) -> Result<DynamicImage, CaptureError> {
    Err(failed("screenshots are not supported on this platform"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_shrinks_large_captures_and_builds_a_preview() {
        let image = DynamicImage::new_rgb8(5120, 1440);
        let prepared = prepare(&image).unwrap();
        let upload = image::load_from_memory(&prepared.upload_jpeg).unwrap();
        assert_eq!((upload.width(), upload.height()), (2560, 720));
        assert!(prepared.preview_data_url.starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn window_region_is_clipped_to_its_monitor() {
        let monitor = Area { x: 1920, y: 0, width: 2560, height: 1440 };
        let window = Area { x: 2000, y: 100, width: 800, height: 600 };
        assert_eq!(region_within(window, monitor), Some((80, 100, 800, 600)));
        let overhanging = Area { x: 1800, y: -50, width: 400, height: 300 };
        assert_eq!(region_within(overhanging, monitor), Some((0, 0, 280, 250)));
        let elsewhere = Area { x: 0, y: 0, width: 800, height: 600 };
        assert_eq!(region_within(elsewhere, monitor), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn kwin_pixels_honour_stride_and_byte_order() {
        // 2x2, stride 12 (4 bytes of padding per row).
        let rgba = [1, 2, 3, 9, 4, 5, 6, 9, 0, 0, 0, 0, 7, 8, 9, 9, 10, 11, 12, 9, 0, 0, 0, 0];
        let image = kwin::to_image(2, 2, 12, 18, &rgba).unwrap();
        assert_eq!(image.get_pixel(1, 1).0, [10, 11, 12, 255]);
        let bgra = kwin::to_image(2, 2, 12, 5, &rgba).unwrap();
        assert_eq!(bgra.get_pixel(0, 0).0, [3, 2, 1, 255]);
        assert!(kwin::to_image(2, 2, 12, 99, &rgba).is_none());
        assert!(kwin::to_image(2, 3, 12, 18, &rgba).is_none());
    }

    #[test]
    fn save_copy_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("qb-shot-test-{}", std::process::id()));
        let image = DynamicImage::new_rgb8(4, 4);
        let first = save_copy(&image, &dir).unwrap();
        let second = save_copy(&image, &dir).unwrap();
        assert_ne!(first, second);
        assert!(first.exists() && second.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
