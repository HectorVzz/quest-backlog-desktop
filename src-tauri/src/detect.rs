//! Finds the game that is running, without touching the game itself: only
//! process metadata the OS already exposes.
//!
//! - Steam: the running app id. Windows keeps it in the registry
//!   (`HKCU\Software\Valve\Steam\RunningAppID`); on Linux every process
//!   Steam starts for a game, Proton included, has `SteamAppId` in its
//!   environment. Launchers built on umu (Faugus, Lutris, Heroic) also set
//!   `SteamAppId`, but to a non-numeric id, which is ignored here.
//! - Other games: the game's executable. On Windows, the process that owned
//!   the foreground window when the overlay was opened. On Linux (Wayland
//!   has no foreground-window API), the Windows executables running under
//!   Wine/Proton, minus Wine's own processes, launchers and helpers.
//!
//! Matching an executable to a Quest Backlog game happens in `games.rs`.

/// A Windows executable name as found, e.g. "Wow.exe".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Executable {
    pub name: String,
}

impl Executable {
    /// Case-insensitive key used for remembered games and the built-in list.
    pub fn key(&self) -> String {
        self.name.to_lowercase()
    }
}

#[derive(Debug, Default)]
pub struct Running {
    /// Most recently started first.
    pub steam_app_ids: Vec<u32>,
    /// Most recently started first.
    pub executables: Vec<Executable>,
}

/// Wine's own processes and well-known launchers, helpers and anti-cheat
/// services that run next to games. Lowercase.
const IGNORED_EXECUTABLES: &[&str] = &[
    // Wine / Proton
    "services.exe", "winedevice.exe", "plugplay.exe", "svchost.exe", "lsass.exe",
    "explorer.exe", "rpcss.exe", "tabtip.exe", "xalia.exe", "conhost.exe", "start.exe",
    "steam.exe", "wineboot.exe", "winemenubuilder.exe", "rundll32.exe", "cmd.exe",
    "regedit.exe", "msiexec.exe", "iexplore.exe", "mscorsvw.exe",
    // Launchers
    "battle.net.exe", "agent.exe", "epicgameslauncher.exe", "galaxyclient.exe",
    "ubisoftconnect.exe", "upc.exe", "eadesktop.exe", "eabackgroundservice.exe",
    "origin.exe", "riotclientservices.exe", "riotclientux.exe", "riotclientuxrender.exe",
    "rockstarservice.exe", "launcher.exe", "launcherpatcher.exe", "playnite.desktopapp.exe",
    // Anti-cheat
    "easyanticheat.exe", "easyanticheat_eos.exe", "beservice.exe", "beservice_x64.exe",
];

/// Substrings of helper processes (crash reporters, web helpers, voice
/// proxies, updaters) that are never the game. Lowercase.
const IGNORED_FRAGMENTS: &[&str] = &[
    "crash", "helper", "updater", "installer", "setup", "uninst", "voiceproxy",
    "webhelper", "renderer", "overlay", "launcher",
];

pub fn is_ignored(key: &str) -> bool {
    IGNORED_EXECUTABLES.contains(&key) || IGNORED_FRAGMENTS.iter().any(|part| key.contains(part))
}

/// The file name of a Windows or Unix path ("C:\Games\Wow.exe" -> "Wow.exe").
pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path).trim()
}

/// A game executable worth matching, or `None` for anything else.
fn game_executable(path: &str) -> Option<Executable> {
    let name = file_name(path);
    let key = name.to_lowercase();
    if !key.ends_with(".exe") || key.len() <= 4 || is_ignored(&key) {
        return None;
    }
    // Wine's own programs live under C:\windows.
    let lower_path = path.to_lowercase().replace('/', "\\");
    if lower_path.contains(":\\windows\\") {
        return None;
    }
    Some(Executable { name: name.to_string() })
}

/// A real Steam app id: numeric and not zero. umu-based launchers set
/// values such as "faugus5dd8f7d6" or "umu-default".
pub fn steam_app_id(value: &str) -> Option<u32> {
    value.trim().parse::<u32>().ok().filter(|id| *id > 0)
}

fn push_unique<T: PartialEq>(list: &mut Vec<T>, item: T) {
    if !list.contains(&item) {
        list.push(item);
    }
}

#[cfg(target_os = "linux")]
pub fn scan() -> Running {
    let mut steam: Vec<(u32, u32)> = Vec::new();
    let mut executables: Vec<(u32, Executable)> = Vec::new();

    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Running::default();
    };
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|name| name.parse::<u32>().ok()) else {
            continue;
        };
        let dir = entry.path();

        // Other users' processes are unreadable; skipping them is fine.
        if let Ok(environ) = std::fs::read(dir.join("environ")) {
            let app_id = environ
                .split(|byte| *byte == 0)
                .filter_map(|pair| std::str::from_utf8(pair).ok())
                .find_map(|pair| pair.strip_prefix("SteamAppId="))
                .and_then(steam_app_id);
            if let Some(app_id) = app_id {
                steam.push((pid, app_id));
            }
        }

        if let Ok(cmdline) = std::fs::read(dir.join("cmdline")) {
            let argv0 = cmdline.split(|byte| *byte == 0).next().unwrap_or_default();
            if let Some(executable) = std::str::from_utf8(argv0).ok().and_then(game_executable) {
                executables.push((pid, executable));
            }
        }
    }

    // Higher pid ~ started later: prefer the newest game.
    steam.sort_by(|a, b| b.0.cmp(&a.0));
    executables.sort_by(|a, b| b.0.cmp(&a.0));

    let mut running = Running::default();
    for (_, app_id) in steam {
        push_unique(&mut running.steam_app_ids, app_id);
    }
    for (_, executable) in executables {
        push_unique(&mut running.executables, executable);
    }
    running
}

#[cfg(target_os = "windows")]
mod windows {
    use std::sync::Mutex;

    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    use super::{game_executable, Executable};

    /// The game the user was in when the overlay was opened.
    static FOREGROUND: Mutex<Option<Executable>> = Mutex::new(None);

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn foreground_path() -> Option<String> {
        // SAFETY: plain Win32 queries; the process handle is closed before
        // returning and the buffer length is passed in and out.
        unsafe {
            let window = GetForegroundWindow();
            if window.is_null() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(window, &mut pid);
            if pid == 0 || pid == std::process::id() {
                return None;
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let mut buffer = [0u16; 1024];
            let mut length = buffer.len() as u32;
            let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut length);
            CloseHandle(process);
            (ok != 0).then(|| String::from_utf16_lossy(&buffer[..length as usize]))
        }
    }

    /// Runs just before the overlay takes focus. Keeps the last game seen,
    /// so opening the overlay from the tray still knows it.
    pub fn capture_foreground() {
        if let Some(executable) = foreground_path().as_deref().and_then(game_executable) {
            *FOREGROUND.lock().unwrap() = Some(executable);
        }
    }

    pub fn foreground() -> Option<Executable> {
        FOREGROUND.lock().unwrap().clone()
    }

    pub fn running_steam_app_id() -> Option<u32> {
        let key = wide("Software\\Valve\\Steam");
        let value = wide("RunningAppID");
        let mut data = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        // SAFETY: reads one DWORD into `data`; `size` holds its byte length.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut data as *mut u32).cast(),
                &mut size,
            )
        };
        (status == 0 && data > 0).then_some(data)
    }
}

#[cfg(target_os = "windows")]
pub use windows::capture_foreground;

#[cfg(target_os = "windows")]
pub fn scan() -> Running {
    Running {
        steam_app_ids: windows::running_steam_app_id().into_iter().collect(),
        executables: windows::foreground().into_iter().collect(),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn capture_foreground() {}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn scan() -> Running {
    Running::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_app_ids_must_be_numeric() {
        assert_eq!(steam_app_id("1245620"), Some(1245620));
        assert_eq!(steam_app_id("0"), None);
        assert_eq!(steam_app_id("faugus5dd8f7d6"), None);
        assert_eq!(steam_app_id("umu-default"), None);
    }

    #[test]
    fn finds_game_executables_and_skips_wine_and_launchers() {
        let found = |path: &str| game_executable(path).map(|exe| exe.name);
        assert_eq!(
            found(r"C:\Program Files (x86)\World of Warcraft\_forever_\Wow.exe"),
            Some("Wow.exe".to_string())
        );
        assert_eq!(found("C:/Games/Hades/Hades.exe"), Some("Hades.exe".to_string()));
        assert_eq!(found(r"C:\windows\system32\services.exe"), None);
        assert_eq!(found(r"c:\windows\system32\steam.exe"), None);
        assert_eq!(
            found("/home/u/Faugus/battlenet/drive_c/Program Files (x86)/Battle.net/Battle.net.exe"),
            None
        );
        assert_eq!(found("C:/ProgramData/Battle.net/Agent/Agent.9873/Agent.exe"), None);
        assert_eq!(found("./Utils/WowVoiceProxy.exe"), None);
        assert_eq!(found("/usr/bin/firefox"), None);
        assert_eq!(found(".exe"), None);
    }
}
