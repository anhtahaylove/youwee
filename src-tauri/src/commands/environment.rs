use std::path::Path;
#[cfg(any(target_os = "windows", test))]
use std::path::PathBuf;
use tauri::AppHandle;
#[cfg(target_os = "windows")]
use tauri::Manager;

#[cfg(any(target_os = "windows", test))]
fn chromium_extension_path(resource_dir: &Path) -> PathBuf {
    resource_dir.join("Youwee-Extension-Chromium")
}

#[cfg(any(target_os = "windows", test))]
fn firefox_extension_path(resource_dir: &Path) -> PathBuf {
    resource_dir.join("Youwee-Extension-Firefox-signed.xpi")
}

#[tauri::command]
pub fn is_flatpak_environment() -> bool {
    cfg!(target_os = "linux")
        && (std::env::var_os("FLATPAK_ID").is_some() || Path::new("/.flatpak-info").exists())
}

#[tauri::command]
pub fn get_bundled_chromium_extension_path(app: AppHandle) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let path = chromium_extension_path(&app.path().resource_dir().ok()?);
        path.join("manifest.json")
            .is_file()
            .then(|| path.to_string_lossy().to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        None
    }
}

#[tauri::command]
pub fn get_bundled_firefox_extension_path(app: AppHandle) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let path = firefox_extension_path(&app.path().resource_dir().ok()?);
        path.is_file().then(|| path.to_string_lossy().to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{chromium_extension_path, firefox_extension_path};
    use std::path::Path;

    #[test]
    fn bundled_chromium_extension_uses_a_stable_resource_folder() {
        assert_eq!(
            chromium_extension_path(Path::new("C:/Youwee/resources")),
            Path::new("C:/Youwee/resources/Youwee-Extension-Chromium")
        );
    }

    #[test]
    fn bundled_firefox_extension_uses_a_stable_resource_file() {
        assert_eq!(
            firefox_extension_path(Path::new("C:/Youwee/resources")),
            Path::new("C:/Youwee/resources/Youwee-Extension-Firefox-signed.xpi")
        );
    }
}
