fn main() {
    let windows = tauri_build::WindowsAttributes::new()
        .app_manifest(include_str!("../resources/windows/myalbuns.manifest"));
    let attributes = tauri_build::Attributes::new()
        .windows_attributes(windows)
        .app_manifest(tauri_build::AppManifest::new());
    tauri_build::try_build(attributes).expect("failed to run the Tauri build script");
}
