fn main() {
    // Declaring the app's commands makes each one need an explicit permission
    // (`allow-<command>`) in `capabilities/`, so nothing is callable by default.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_info",
            "vault_status",
            "vault_setup",
            "vault_confirm_recovery_saved",
            "unlock",
            "lock",
            "change_password",
        ]),
    ))
    .expect("failed to run tauri-build");
}
