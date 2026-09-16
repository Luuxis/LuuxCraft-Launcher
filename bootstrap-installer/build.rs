fn main() {
    // L'adresse du panel est figée à la compilation (voir `src/config.rs`) ;
    // sans ces lignes, cargo ne recompilerait pas quand un build la change.
    println!("cargo:rerun-if-env-changed=LUUXCRAFT_PANEL_URL");
    println!("cargo:rerun-if-env-changed=PANEL_URL");
    println!("cargo:rerun-if-env-changed=LUUXCRAFT_BUNDLE_ID_PREFIX");
}
