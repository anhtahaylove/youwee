use std::path::Path;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct UpdaterCleanupReport {
    pub removed: usize,
    pub failed: usize,
    pub current_version_detected: bool,
}

fn is_youwee_updater_temp_dir(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("Youwee-") else {
        return false;
    };
    let Some((version, suffix)) = rest.rsplit_once("-updater-") else {
        return false;
    };

    !version.is_empty()
        && suffix.len() == 6
        && suffix
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

fn cleanup_updater_temp_dirs_in(temp_root: &Path) -> UpdaterCleanupReport {
    let mut report = UpdaterCleanupReport::default();
    let Ok(entries) = std::fs::read_dir(temp_root) else {
        report.failed = 1;
        return report;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let is_directory = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        let name = name.to_str();
        let matches = name.is_some_and(is_youwee_updater_temp_dir);

        if is_directory && matches {
            if name.is_some_and(|value| {
                value.starts_with(&format!("Youwee-{}-updater-", env!("CARGO_PKG_VERSION")))
            }) {
                report.current_version_detected = true;
            }
            if std::fs::remove_dir_all(path).is_ok() {
                report.removed += 1;
            } else {
                report.failed += 1;
            }
        }
    }

    report
}

pub fn cleanup_stale_updater_temp_dirs() -> UpdaterCleanupReport {
    cleanup_updater_temp_dirs_in(&std::env::temp_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn removes_only_youwee_updater_directories() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("youwee-updater-cleanup-test-{unique}"));
        let stale = root.join(format!(
            "Youwee-{}-updater-Ab12Cd",
            env!("CARGO_PKG_VERSION")
        ));
        let unrelated = root.join("OtherApp-1.0.0-updater-Ab12Cd");
        let malformed = root.join("Youwee-0.19.1-custom.33-updater-too-long");

        std::fs::create_dir_all(&stale).expect("create stale updater directory");
        std::fs::create_dir_all(&unrelated).expect("create unrelated directory");
        std::fs::create_dir_all(&malformed).expect("create malformed directory");
        std::fs::write(stale.join("installer.exe"), b"fixture").expect("write fixture");

        let report = cleanup_updater_temp_dirs_in(&root);

        assert_eq!(report.removed, 1);
        assert_eq!(report.failed, 0);
        assert!(report.current_version_detected);
        assert!(!stale.exists());
        assert!(unrelated.exists());
        assert!(malformed.exists());

        std::fs::remove_dir_all(root).expect("remove test directory");
    }
}
