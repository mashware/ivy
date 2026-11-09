use std::io::Read;
use std::time::Duration;

use semver::Version;
use serde::Deserialize;

const ENDPOINT: &str = "https://api.github.com/repos/mashware/ivy/releases?per_page=20";
const MAX_BODY: u64 = 1024 * 1024;
const MAX_NOTES: usize = 32 * 1024;

#[derive(Debug, Clone)]
pub(super) struct Release {
    pub(super) version: Version,
    pub(super) name: String,
    pub(super) notes: String,
    pub(super) url: String,
}

#[derive(Deserialize)]
struct Entry {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

pub(super) fn fetch() -> Result<Vec<Release>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .max_redirects(0)
        .build()
        .into();
    let mut response = agent
        .get(ENDPOINT)
        .header("User-Agent", format!("Ivy/{}", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Could not check releases: {e}"))?;
    read(response.body_mut().as_reader())
}

fn read(reader: impl Read) -> Result<Vec<Release>, String> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_BODY + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Could not read releases: {e}"))?;
    if bytes.len() as u64 > MAX_BODY {
        return Err("Release response exceeds 1 MiB".into());
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|e| format!("Release response is not UTF-8: {e}"))?;
    let entries: Vec<Entry> =
        serde_json::from_str(text).map_err(|e| format!("Invalid release response: {e}"))?;
    let mut releases: Vec<_> = entries
        .into_iter()
        .filter_map(|entry| {
            if entry.draft || entry.prerelease || entry.tag_name.len() > 256 {
                return None;
            }
            let version =
                Version::parse(entry.tag_name.strip_prefix('v').unwrap_or(&entry.tag_name)).ok()?;
            if !version.pre.is_empty() {
                return None;
            }
            let mut url = url::Url::parse("https://github.com/mashware/ivy/releases/tag/").ok()?;
            url.path_segments_mut()
                .ok()?
                .pop_if_empty()
                .push(&entry.tag_name);
            let mut notes = entry
                .body
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "No release notes provided.".into());
            if notes.len() > MAX_NOTES {
                let mut end = MAX_NOTES - "\n\n[Release notes truncated.]".len();
                while !notes.is_char_boundary(end) {
                    end -= 1;
                }
                notes.truncate(end);
                notes.push_str("\n\n[Release notes truncated.]");
            }
            Some(Release {
                name: {
                    let mut name = entry
                        .name
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| entry.tag_name.clone());
                    if name.len() > 512 {
                        let mut end = 509;
                        while !name.is_char_boundary(end) {
                            end -= 1;
                        }
                        name.truncate(end);
                        name.push('…');
                    }
                    name
                },
                version,
                notes,
                url: url.into(),
            })
        })
        .collect();
    releases.sort_by(|a, b| b.version.cmp_precedence(&a.version));
    Ok(releases)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_versions_are_sorted_and_untrusted_tags_filtered() {
        let releases = read(r#"[{"tag_name":"v1.9.0"},{"tag_name":"1.10.0"},{"tag_name":"v2.0.0-rc.1"},{"tag_name":"v9.0.0","draft":true},{"tag_name":"v8.0.0","prerelease":true},{"tag_name":"../bad"}]"#.as_bytes()).unwrap();
        assert_eq!(
            releases
                .iter()
                .map(|r| r.version.to_string())
                .collect::<Vec<_>>(),
            ["1.10.0", "1.9.0"]
        );
        assert_eq!(
            releases[0].url,
            "https://github.com/mashware/ivy/releases/tag/1.10.0"
        );
    }
    #[test]
    fn rejects_oversized_and_invalid_response() {
        assert!(
            read(&vec![b' '; MAX_BODY as usize + 1][..])
                .unwrap_err()
                .contains("1 MiB")
        );
        assert!(read(&b"\xff"[..]).unwrap_err().contains("UTF-8"));
        assert!(read(&b"{}"[..]).is_err());
        assert!(read(&b"[]"[..]).unwrap().is_empty());
    }
    #[test]
    fn names_and_tags_are_bounded_and_missing_notes_are_explained() {
        let input = serde_json::json!([
            { "tag_name": "1.0.0", "name": "é".repeat(400), "body": "" },
            { "tag_name": format!("1.0.0+{}", "a".repeat(256)) }
        ])
        .to_string();
        let releases = read(input.as_bytes()).unwrap();
        assert_eq!(releases.len(), 1);
        assert!(releases[0].name.len() <= 512);
        assert_eq!(releases[0].notes, "No release notes provided.");
    }
    #[test]
    fn notes_are_bounded_at_utf8_boundary() {
        let input =
            serde_json::json!([{ "tag_name": "1.0.0", "body": "é".repeat(MAX_NOTES) }]).to_string();
        let releases = read(input.as_bytes()).unwrap();
        assert!(releases[0].notes.len() <= MAX_NOTES);
        assert!(releases[0].notes.ends_with("[Release notes truncated.]"));
    }
}
