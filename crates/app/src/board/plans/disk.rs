//! One versioned, atomically replaced file per common Git directory.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::Path,
    time::{Duration, UNIX_EPOCH},
};

#[derive(Serialize, Deserialize)]
struct Saved {
    v: u32,
    project: PathBuf,
    name: String,
    published_at: String,
    document: String,
}

pub(super) struct Disk {
    directory: Option<PathBuf>,
    writable: bool,
    vetoed: HashSet<PathBuf>,
}
impl Disk {
    pub(super) fn new(directory: Option<PathBuf>, writable: bool) -> Self {
        Self {
            directory,
            writable,
            vetoed: HashSet::new(),
        }
    }
    pub(super) fn restore(&mut self) -> HashMap<Key, Arc<Published>> {
        let mut bank = HashMap::new();
        let Some(directory) = self.directory.clone() else {
            return bank;
        };
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return bank,
            Err(error) => {
                eprintln!("Project plans not restored: {error}");
                self.writable = false;
                return bank;
            }
        };
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            match load(&path) {
                Ok(saved) => {
                    let key = Key::Repository(saved.project.clone());
                    if path.file_name() != Some(filename(&saved.project).as_ref()) {
                        self.vetoed.insert(path);
                        continue;
                    }
                    let Ok((plan, _)) =
                        panel_contract::plan::parse_document_noting(&saved.document)
                    else {
                        self.vetoed.insert(path);
                        continue;
                    };
                    let Some(published_at) = timestamp(&saved.published_at) else {
                        self.vetoed.insert(path);
                        continue;
                    };
                    bank.insert(
                        key,
                        Arc::new(Published {
                            plan,
                            document: saved.document,
                            published_at,
                        }),
                    );
                }
                Err(error) => {
                    eprintln!("Project plan {} not restored: {error}", path.display());
                    self.vetoed.insert(path);
                }
            }
        }
        bank
    }
    fn target(&self, identity: &Identity) -> Result<PathBuf, String> {
        if !self.writable {
            return Err("saving is disabled for this session".into());
        }
        let directory = self
            .directory
            .as_ref()
            .ok_or("the configuration directory is unavailable")?;
        let path = directory.join(filename(&identity.path));
        if self.vetoed.contains(&path) {
            return Err("the stored plan could not be understood; it has been preserved".into());
        }
        Ok(path)
    }
    pub(super) fn save(
        &mut self,
        identity: &Identity,
        document: &str,
        published_at: SystemTime,
    ) -> Result<(), String> {
        let path = self.target(identity)?;
        if path.exists() {
            let saved = load(&path).inspect_err(|_| {
                self.vetoed.insert(path.clone());
            })?;
            if saved.project != identity.path {
                self.vetoed.insert(path);
                return Err("a different project holds this storage name".into());
            }
        }
        let saved = Saved {
            v: 1,
            project: identity.path.clone(),
            name: identity.name.clone(),
            published_at: date(published_at),
            document: document.into(),
        };
        let bytes = serde_json::to_vec(&saved).map_err(|e| e.to_string())?;
        let directory = path.parent().unwrap();
        fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let temp = path.with_extension(format!("tmp-{}", std::process::id()));
        let result = (|| {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result.map_err(|e: std::io::Error| e.to_string())
    }
    pub(super) fn retire(&self, identity: &Identity) -> Result<(), String> {
        // An in-memory publication in a measurement session may be retired without touching disk.
        if !self.writable {
            return Ok(());
        }
        let path = self.target(identity)?;
        if path.exists() {
            let saved = load(&path)?;
            if saved.project != identity.path {
                return Err("a different project holds this storage name".into());
            }
        }
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("the stored plan could not be retired: {e}")),
        }
    }
}
fn load(path: &Path) -> Result<Saved, String> {
    // JSON escaping can expand the bounded document by six; the wrapper has its own fixed allowance.
    let ceiling = panel_contract::MAX_BYTES * 6 + 64 * 1024;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(ceiling + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > ceiling {
        return Err("stored plan is over the ceiling".into());
    }
    let saved: Saved = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if saved.v != 1 {
        return Err(format!("unknown plan version {}", saved.v));
    }
    Ok(saved)
}
fn filename(project: &Path) -> String {
    // A stable hash, independent of Rust's per-process HashMap seed.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in project.as_os_str().as_encoded_bytes() {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}.json")
}

// Gregorian civil-date conversion keeps the stored UTC timestamp portable without a locale or subprocess.
fn date(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}
fn timestamp(text: &str) -> Option<SystemTime> {
    if text.len() != 20 || !text.is_ascii() {
        return None;
    }
    let n = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day, hour, minute, second) = (
        n(0..4)?,
        n(5..7)?,
        n(8..10)?,
        n(11..13)?,
        n(14..16)?,
        n(17..19)?,
    );
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || !(0..24).contains(&hour)
        || !(0..60).contains(&minute)
        || !(0..60).contains(&second)
    {
        return None;
    }
    let year = year - i64::from(month <= 2);
    let era = year / 400;
    let yoe = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month + 2) / 5 + day - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    let seconds = days
        .checked_mul(86400)?
        .checked_add(hour * 3600 + minute * 60 + second)?;
    let time = UNIX_EPOCH.checked_add(Duration::from_secs(u64::try_from(seconds).ok()?))?;
    (date(time) == text).then_some(time)
}
