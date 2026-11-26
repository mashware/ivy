//! Local path targets are revealed internally, never sent to a desktop opener.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalTarget {
    pub(crate) path: PathBuf,
    pub(crate) line: Option<u32>,
}
impl LocalTarget {
    pub(crate) fn parse(target: &str, cwd: Option<&Path>) -> Option<Self> {
        // std reads %USERPROFILE% on Windows, not HOME (D108).
        Self::parse_with_home(target, cwd, std::env::home_dir)
    }
    fn parse_with_home(
        target: &str,
        cwd: Option<&Path>,
        home: impl FnOnce() -> Option<PathBuf>,
    ) -> Option<Self> {
        if target.len() > 8192
            || target
                .chars()
                .any(|c| !panel_contract::is_shown_as_itself(c))
        {
            return None;
        }
        let target = target.trim_matches(['`', '\'', '"']);
        let (target, line) = suffix(target);
        let path = if target.starts_with("file://") {
            let url = url::Url::parse(target).ok()?;
            if !term_core::Screen::is_local_file_host(url.host_str().unwrap_or(""))
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return None;
            }
            url::Url::parse(&format!("file://{}", url.path()))
                .ok()?
                .to_file_path()
                .ok()?
        } else {
            let drive = target
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
                && target.as_bytes().get(1) == Some(&b':')
                && matches!(target.as_bytes().get(2), Some(b'/' | b'\\'));
            if target.contains("://") || (!drive && target.contains(':')) {
                return None;
            }
            let home_relative = target
                .strip_prefix("~/")
                .or_else(|| target.strip_prefix("~\\").filter(|_| cfg!(windows)));
            if let Some(relative) = home_relative {
                home()?.join(relative)
            } else {
                if !target.contains(['/', '\\'])
                    && Path::new(target)
                        .extension()
                        .is_none_or(|ext| ext.is_empty())
                {
                    return None;
                }
                PathBuf::from(target)
            }
        };
        if path
            .to_str()
            .is_some_and(|text| text.chars().any(|c| !panel_contract::is_shown_as_itself(c)))
        {
            return None;
        }
        let path = if path.is_absolute() {
            path
        } else {
            cwd.map_or(path.clone(), |cwd| cwd.join(path))
        };
        Some(Self { path, line })
    }
}
fn suffix(target: &str) -> (&str, Option<u32>) {
    let Some((head, last)) = target.rsplit_once(':') else {
        return (target, None);
    };
    let Ok(n) = last.parse::<u32>() else {
        return (target, None);
    };
    if n == 0 {
        return (target, None);
    }
    if let Some((path, row)) = head.rsplit_once(':')
        && let Ok(row) = row.parse::<u32>()
        && row > 0
    {
        return (path, Some(row));
    }
    (head, Some(n))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_local_paths_and_refuses_web_or_remote_targets() {
        assert_eq!(
            LocalTarget::parse("`docs/a.md:12:3`", Some(Path::new("/repo")))
                .unwrap()
                .line,
            Some(12)
        );
        let file = std::env::temp_dir().join("a b.md");
        let url = url::Url::from_file_path(&file).unwrap();
        assert!(url.as_str().contains("a%20b.md"));
        assert_eq!(LocalTarget::parse(url.as_str(), None).unwrap().path, file);
        assert_eq!(
            LocalTarget::parse("docs/a.md", None).unwrap().path,
            Path::new("docs/a.md")
        );
        assert!(LocalTarget::parse("file://remote/tmp/a.md", None).is_none());
        assert!(LocalTarget::parse("https://example/a.md", None).is_none());
    }
    #[test]
    fn windows_paths_tilde_expands_from_the_given_home() {
        let home = if cfg!(windows) {
            Path::new(r"C:\Users\someone")
        } else {
            Path::new("/home/someone")
        };
        let cwd = Some(Path::new("/elsewhere"));
        let given = || Some(home.to_path_buf());
        assert_eq!(
            LocalTarget::parse_with_home("~/README.md", cwd, given)
                .unwrap()
                .path,
            home.join("README.md")
        );
        assert!(LocalTarget::parse_with_home("~/README.md", cwd, || None).is_none());
        // The native separator is Windows' alone; elsewhere a backslash is part of a name.
        let native = LocalTarget::parse_with_home(r"~\README.md", cwd, given).unwrap();
        if cfg!(windows) {
            assert_eq!(native.path, home.join("README.md"));
        } else {
            assert_eq!(native.path, Path::new("/elsewhere").join(r"~\README.md"));
        }
    }
    // On the Windows runner HOME is unset or differs from %USERPROFILE%, so reading HOME again in
    // `parse` sends this link elsewhere.
    #[cfg(windows)]
    #[test]
    fn windows_paths_tilde_follows_home_dir() {
        let home = std::env::home_dir().expect("Windows always has a profile folder");
        assert_eq!(
            LocalTarget::parse("~/README.md", None).unwrap().path,
            home.join("README.md")
        );
    }
    #[test]
    #[cfg(unix)]
    fn accepts_current_hostname_and_refuses_remote_hostname() {
        let mut name = [0u8; 256];
        assert_eq!(
            unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) },
            0
        );
        let end = name.iter().position(|b| *b == 0).unwrap();
        let host = std::str::from_utf8(&name[..end]).unwrap();
        assert_eq!(
            LocalTarget::parse(&format!("file://{host}/tmp/local.md"), None)
                .unwrap()
                .path,
            Path::new("/tmp/local.md")
        );
        assert!(LocalTarget::parse("file://different-host.invalid/tmp/local.md", None).is_none());
    }
    #[test]
    fn local_files_and_directories_keep_literal_paths_without_executing_them() {
        for path in [
            "/repo/src",
            "src/main.rs",
            "./assets",
            "Cargo.toml",
            "/tmp/a;echo.txt",
        ] {
            let target = LocalTarget::parse(path, Some(Path::new("/repo"))).unwrap();
            assert_eq!(
                target.path,
                if Path::new(path).is_absolute() {
                    PathBuf::from(path)
                } else {
                    Path::new("/repo").join(path)
                }
            );
        }
        let folder = std::env::temp_dir().join("folder");
        let url = url::Url::from_directory_path(&folder).unwrap();
        assert_eq!(LocalTarget::parse(url.as_str(), None).unwrap().path, folder);
        let hidden = std::env::temp_dir().join("\u{1b}hidden.rs");
        let hidden_url = url::Url::from_file_path(hidden).unwrap();
        assert!(LocalTarget::parse(hidden_url.as_str(), None).is_none());
        for path in [
            "ordinary",
            "file://remote.invalid/tmp/folder",
            "https://example.test/a.rs",
            "file:///tmp/%1Bhidden.rs",
            "/tmp/\u{202e}hidden.rs",
        ] {
            assert!(
                LocalTarget::parse(path, None).is_none(),
                "accepted {path:?}"
            );
        }
    }
}
