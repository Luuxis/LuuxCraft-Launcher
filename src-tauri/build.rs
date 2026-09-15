fn main() {
    // The panel address is baked in from these at compile time (see config.rs);
    // without this, cargo would not rebuild when a build overrides them.
    println!("cargo:rerun-if-env-changed=LUUXCRAFT_API_URL");
    println!("cargo:rerun-if-env-changed=LUUXCRAFT_USER_ID");
    tauri_build::build()
}
