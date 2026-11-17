fn main() {
    // The server URLs are baked in at compile time (see src/config.rs).
    println!("cargo:rerun-if-env-changed=QB_API_BASE_URL");
    println!("cargo:rerun-if-env-changed=QB_WEB_BASE_URL");
    tauri_build::build()
}
