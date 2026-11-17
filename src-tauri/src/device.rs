//! What the app tells the server about this computer when it asks to sign
//! in. The user sees it on the /link approval page and in Connected devices.

const MAX_LEN: usize = 64;

fn clip(value: &str, fallback: &str) -> String {
    let value = value.trim();
    let value = if value.is_empty() { fallback } else { value };
    value.chars().take(MAX_LEN).collect()
}

pub fn name() -> String {
    #[cfg(target_os = "windows")]
    let raw = std::env::var("COMPUTERNAME").unwrap_or_default();
    #[cfg(target_os = "linux")]
    let raw = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .or_else(|_| std::fs::read_to_string("/etc/hostname"))
        .unwrap_or_default();
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    let raw = String::new();

    clip(&raw, "Desktop")
}

pub fn os() -> String {
    #[cfg(target_os = "windows")]
    return "Windows".to_string();

    #[cfg(target_os = "linux")]
    {
        let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        let field = |key: &str| {
            release.lines().find_map(|line| {
                line.strip_prefix(key)
                    .and_then(|rest| rest.strip_prefix('='))
                    .map(|value| value.trim_matches('"').to_string())
            })
        };
        let pretty = field("PRETTY_NAME").or_else(|| field("NAME")).unwrap_or_default();
        return clip(&pretty, "Linux");
    }

    #[allow(unreachable_code)]
    "Desktop".to_string()
}
