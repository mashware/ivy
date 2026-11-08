//! Local Markdown targets are read internally, never sent to a desktop opener.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalTarget {
    pub(crate) path: PathBuf,
    pub(crate) line: Option<u32>,
}
impl LocalTarget {
    pub(crate) fn parse(target: &str, cwd: Option<&Path>) -> Option<Self> {
        if target.len() > 8192 || target.chars().any(char::is_control) {
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
            if target.contains("://") || target.contains(':') {
                return None;
            }
            PathBuf::from(target)
        };
        if !path.extension()?.eq_ignore_ascii_case("md") {
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
    fn accepts_only_local_markdown() {
        assert_eq!(
            LocalTarget::parse("`docs/a.md:12:3`", Some(Path::new("/repo")))
                .unwrap()
                .line,
            Some(12)
        );
        assert_eq!(
            LocalTarget::parse("file:///tmp/a%20b.md", None)
                .unwrap()
                .path,
            Path::new("/tmp/a b.md")
        );
        assert_eq!(
            LocalTarget::parse("docs/a.md", None).unwrap().path,
            Path::new("docs/a.md")
        );
        assert!(LocalTarget::parse("file://remote/tmp/a.md", None).is_none());
        assert!(LocalTarget::parse("https://example/a.md", None).is_none());
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
}
