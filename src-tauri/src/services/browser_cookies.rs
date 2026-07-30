use crate::services::should_skip_cookies_for_url;
use crate::utils::{firefox_profiles_ini_path, resolve_firefox_profile_for_cookies};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

fn cookie_domain_matches(host: &str, cookie_domain: &str) -> bool {
    let host = host.trim_start_matches('.').to_ascii_lowercase();
    let domain = cookie_domain.trim_start_matches('.').to_ascii_lowercase();
    host == domain || host.ends_with(&format!(".{domain}")) || domain.ends_with(&format!(".{host}"))
}

fn cookie_header_from_netscape_file(path: &str, target_host: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let now = chrono::Utc::now().timestamp();
    let cookies: Vec<String> = content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let line = line.strip_prefix("#HttpOnly_").unwrap_or(line);
            if line.starts_with('#') {
                return None;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 7 || !cookie_domain_matches(target_host, parts[0]) {
                return None;
            }
            let expires = parts[4].parse::<i64>().unwrap_or(0);
            if expires != 0 && expires <= now {
                return None;
            }
            Some(format!("{}={}", parts[5], parts[6]))
        })
        .collect();

    (!cookies.is_empty()).then(|| cookies.join("; "))
}

fn firefox_cookie_db_path(selected_profile: &str) -> Option<PathBuf> {
    let profile = resolve_firefox_profile_for_cookies(selected_profile);
    let profile_path = PathBuf::from(&profile);
    let path = if profile_path.is_absolute() {
        profile_path
    } else {
        firefox_profiles_ini_path()?
            .parent()?
            .join("Profiles")
            .join(profile)
    };
    Some(path.join("cookies.sqlite"))
}

fn sqlite_sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}-{suffix}", path.to_string_lossy()))
}

fn copy_sqlite_with_sidecars(source: &Path, dest: &Path) -> bool {
    if fs::copy(source, dest).is_err() {
        return false;
    }

    for suffix in ["wal", "shm"] {
        let source_sidecar = sqlite_sidecar_path(source, suffix);
        if source_sidecar.exists() {
            fs::copy(source_sidecar, sqlite_sidecar_path(dest, suffix)).ok();
        }
    }

    true
}

fn remove_sqlite_copy(path: &Path) {
    fs::remove_file(path).ok();
    for suffix in ["wal", "shm"] {
        fs::remove_file(sqlite_sidecar_path(path, suffix)).ok();
    }
}

fn read_firefox_cookie_header(db_path: &Path, target_host: &str) -> Option<String> {
    let temp_path =
        std::env::temp_dir().join(format!("youwee-cookies-{}.sqlite", uuid::Uuid::new_v4()));
    let db_to_read = if copy_sqlite_with_sidecars(db_path, &temp_path) {
        temp_path.as_path()
    } else {
        db_path
    };

    let result = (|| {
        let conn = Connection::open(db_to_read).ok()?;
        let now = chrono::Utc::now().timestamp();
        let mut stmt = conn
            .prepare("SELECT host, name, value FROM moz_cookies WHERE (expiry = 0 OR expiry > ?1)")
            .ok()?;
        let rows = stmt
            .query_map([now], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .ok()?;
        let cookies: Vec<String> = rows
            .filter_map(Result::ok)
            .filter(|(domain, _, _)| cookie_domain_matches(target_host, domain))
            .map(|(_, name, value)| format!("{name}={value}"))
            .collect();
        (!cookies.is_empty()).then(|| cookies.join("; "))
    })();

    remove_sqlite_copy(&temp_path);
    result
}

pub(crate) fn build_browser_cookie_header(
    target_url: &str,
    cookie_mode: Option<&str>,
    cookie_browser: Option<&str>,
    cookie_browser_profile: Option<&str>,
    cookie_file_path: Option<&str>,
    cookie_skip_patterns: Option<&[String]>,
) -> Option<String> {
    if cookie_skip_patterns
        .map(|patterns| should_skip_cookies_for_url(target_url, patterns))
        .unwrap_or(false)
    {
        return None;
    }

    let host = reqwest::Url::parse(target_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))?;

    match cookie_mode.unwrap_or("off") {
        "file" => cookie_file_path
            .filter(|path| !path.trim().is_empty())
            .and_then(|path| cookie_header_from_netscape_file(path, &host)),
        "browser" => match (cookie_browser, cookie_browser_profile) {
            (Some(browser), Some(profile)) if browser.eq_ignore_ascii_case("firefox") => {
                firefox_cookie_db_path(profile)
                    .and_then(|path| read_firefox_cookie_header(&path, &host))
            }
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cookie_header_from_netscape_file, copy_sqlite_with_sidecars, remove_sqlite_copy,
        sqlite_sidecar_path,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn parses_cookie_header_from_netscape_file() {
        let path =
            std::env::temp_dir().join(format!("youwee-cookie-test-{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(
            &path,
            "# Netscape HTTP Cookie File\n#HttpOnly_.tiktok.com\tTRUE\t/\tTRUE\t0\tsessionid\tabc\n.example.com\tTRUE\t/\tTRUE\t0\tother\tnope\n",
        )
        .expect("write cookie file");

        let header =
            cookie_header_from_netscape_file(path.to_str().expect("path"), "www.tiktok.com")
                .expect("cookie header");
        std::fs::remove_file(path).ok();

        assert_eq!(header, "sessionid=abc");
    }

    #[test]
    fn sqlite_sidecar_path_appends_sqlite_wal_suffix() {
        assert_eq!(
            sqlite_sidecar_path(Path::new("cookies.sqlite"), "wal"),
            PathBuf::from("cookies.sqlite-wal")
        );
    }

    #[test]
    fn copies_and_removes_sqlite_sidecars() {
        let dir =
            std::env::temp_dir().join(format!("youwee-sqlite-copy-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let source = dir.join("cookies.sqlite");
        let dest = dir.join("copy.sqlite");
        std::fs::write(&source, "db").expect("write db");
        std::fs::write(sqlite_sidecar_path(&source, "wal"), "wal").expect("write wal");
        std::fs::write(sqlite_sidecar_path(&source, "shm"), "shm").expect("write shm");

        assert!(copy_sqlite_with_sidecars(&source, &dest));
        assert_eq!(std::fs::read_to_string(&dest).ok().as_deref(), Some("db"));
        assert_eq!(
            std::fs::read_to_string(sqlite_sidecar_path(&dest, "wal"))
                .ok()
                .as_deref(),
            Some("wal")
        );
        assert_eq!(
            std::fs::read_to_string(sqlite_sidecar_path(&dest, "shm"))
                .ok()
                .as_deref(),
            Some("shm")
        );

        remove_sqlite_copy(&dest);
        assert!(!dest.exists());
        assert!(!sqlite_sidecar_path(&dest, "wal").exists());
        assert!(!sqlite_sidecar_path(&dest, "shm").exists());
        std::fs::remove_dir_all(dir).ok();
    }
}
