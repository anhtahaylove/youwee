mod ai;
mod deno;
mod ffmpeg;
mod gallerydl;
mod plugin;
pub mod polling;
pub mod telegram;
mod whisper;
mod youtube_search;
mod ytdlp;

use crate::types::DependencySource;
use std::path::PathBuf;
use tauri::AppHandle;
#[cfg(windows)]
use tauri::Manager;

pub(super) fn get_packaged_dependency_path(app: &AppHandle, binary_name: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let binary_path = app
            .path()
            .resource_dir()
            .ok()?
            .join("dependencies")
            .join(binary_name);
        binary_path.is_file().then_some(binary_path)
    }

    #[cfg(not(windows))]
    {
        let _ = (app, binary_name);
        None
    }
}

pub(super) fn select_preferred_dependency_path(
    app_managed: Option<PathBuf>,
    packaged: Option<PathBuf>,
    system: Option<PathBuf>,
) -> Option<PathBuf> {
    app_managed.or(packaged).or(system)
}

pub(super) fn select_dependency_path_for_source(
    source: &DependencySource,
    app_managed: Option<PathBuf>,
    packaged: Option<PathBuf>,
    system: Option<PathBuf>,
) -> Option<PathBuf> {
    match source {
        DependencySource::System => system,
        DependencySource::App => app_managed.or(packaged),
        DependencySource::Auto => select_preferred_dependency_path(app_managed, packaged, system),
    }
}

pub use ai::*;
pub use deno::*;
pub use ffmpeg::*;
pub use gallerydl::*;
pub use plugin::*;
pub use whisper::*;
pub use youtube_search::*;
pub use ytdlp::*;

#[cfg(test)]
mod tests {
    use super::{select_dependency_path_for_source, select_preferred_dependency_path};
    use crate::types::DependencySource;
    use std::path::PathBuf;

    fn path(name: &str) -> Option<PathBuf> {
        Some(PathBuf::from(name))
    }

    #[test]
    fn preferred_dependency_path_uses_app_then_packaged_then_system() {
        assert_eq!(
            select_preferred_dependency_path(path("app"), path("packaged"), path("system")),
            path("app")
        );
        assert_eq!(
            select_preferred_dependency_path(None, path("packaged"), path("system")),
            path("packaged")
        );
        assert_eq!(
            select_preferred_dependency_path(None, None, path("system")),
            path("system")
        );
    }

    #[test]
    fn app_source_keeps_packaged_binary_as_offline_fallback() {
        assert_eq!(
            select_dependency_path_for_source(
                &DependencySource::App,
                None,
                path("packaged"),
                path("system"),
            ),
            path("packaged")
        );
    }

    #[test]
    fn system_source_never_falls_back_to_app_or_packaged_binary() {
        assert_eq!(
            select_dependency_path_for_source(
                &DependencySource::System,
                path("app"),
                path("packaged"),
                None,
            ),
            None
        );
    }
}
