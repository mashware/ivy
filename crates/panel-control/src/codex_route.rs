//! Private, conversation-scoped routes for Codex's panel bridge.
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const ENV: &str = "IVY_CODEX_ROUTE";

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn random() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn safe_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
#[cfg(unix)]
fn check(meta: &fs::Metadata, dir: bool) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    if meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o777 != if dir { 0o700 } else { 0o600 }
        || if dir { !meta.is_dir() } else { !meta.is_file() }
    {
        return Err(invalid(
            "route storage is not private and owned by this user",
        ));
    }
    Ok(())
}
#[cfg(not(unix))]
fn check(meta: &fs::Metadata, dir: bool) -> io::Result<()> {
    if if dir { !meta.is_dir() } else { !meta.is_file() } {
        return Err(invalid("invalid route storage"));
    }
    Ok(())
}
fn validate_dir(path: &Path) -> io::Result<()> {
    check(&fs::symlink_metadata(path)?, true)
}
fn options(create: bool) -> OpenOptions {
    let mut o = OpenOptions::new();
    o.read(true).write(true);
    if create {
        o.create_new(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    o
}
fn open_file(path: &Path, create: bool) -> io::Result<File> {
    let file = options(create).open(path)?;
    check(&file.metadata()?, false)?;
    Ok(file)
}
fn lock(file: &File) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) => return Err(e),
        }
    }
}
fn rewrite(file: &mut File, value: &impl Serialize) -> io::Result<()> {
    let data = serde_json::to_vec(value)?;
    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    file.write_all(&data)?;
    file.sync_data()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub v: u8,
    pub pane_id: String,
    pub pane_dir: PathBuf,
    pub epoch: String,
    pub active: bool,
}
impl Identity {
    fn validate(&self) -> io::Result<()> {
        if self.v != 1
            || (self.active
                && (self.pane_id.is_empty()
                    || !self.pane_dir.is_absolute()
                    || self.epoch.is_empty()))
        {
            return Err(invalid("invalid route record"));
        }
        Ok(())
    }
}
pub struct RouteGuard {
    file: File,
    pub identity: Identity,
}
impl RouteGuard {
    fn write(&mut self) -> io::Result<()> {
        rewrite(&mut self.file, &self.identity)
    }
}
pub fn lock_route(path: &Path) -> io::Result<RouteGuard> {
    if !path.is_absolute() {
        return Err(invalid("route path must be absolute"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| invalid("route has no parent"))?;
    validate_dir(parent)?;
    let mut file = open_file(path, false)?;
    lock(&file)?;
    let mut bytes = Vec::new();
    (&mut file).take(16385).read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(invalid("route record too large"));
    }
    let identity: Identity = serde_json::from_slice(&bytes)?;
    identity.validate()?;
    Ok(RouteGuard { file, identity })
}
pub struct Registry {
    root: PathBuf,
}
impl Registry {
    pub fn for_home(home: &Path) -> io::Result<Self> {
        if !home.is_absolute() {
            return Err(invalid("home must be absolute"));
        }
        let root = home.join("ivy-panel-routes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let mut b = fs::DirBuilder::new();
            b.mode(0o700);
            match b.create(&root) {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(e),
            }
        }
        #[cfg(not(unix))]
        {
            match fs::create_dir(&root) {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(e),
            }
        }
        validate_dir(&root)?;
        Ok(Self { root })
    }
    fn registry_lock(&self) -> io::Result<File> {
        validate_dir(&self.root)?;
        let p = self.root.join("registry.lock");
        let f = match open_file(&p, true) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => open_file(&p, false)?,
            Err(e) => return Err(e),
        };
        lock(&f)?;
        Ok(f)
    }
    pub fn pending(&self) -> io::Result<PathBuf> {
        let _lock = self.registry_lock()?;
        loop {
            let p = self.root.join(format!("route-{}", random()?));
            match open_file(&p, true) {
                Ok(mut f) => {
                    rewrite(
                        &mut f,
                        &Identity {
                            v: 1,
                            pane_id: String::new(),
                            pane_dir: PathBuf::new(),
                            epoch: String::new(),
                            active: false,
                        },
                    )?;
                    return Ok(p);
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }
    fn validate_route(&self, p: &Path) -> io::Result<()> {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if p.parent() != Some(self.root.as_path())
            || !name
                .strip_prefix("route-")
                .is_some_and(|x| x.len() == 32 && x.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(invalid("route outside registry"));
        }
        Ok(())
    }
    pub fn route_for(&self, thread: &str) -> io::Result<Option<PathBuf>> {
        if !safe_uuid(thread) {
            return Err(invalid("invalid thread UUID"));
        }
        let _lock = self.registry_lock()?;
        let path = self
            .root
            .join(format!("thread-{}", thread.to_ascii_lowercase()));
        let mut f = match open_file(&path, false) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut s = String::new();
        (&mut f).take(256).read_to_string(&mut s)?;
        let route = self.root.join(s);
        self.validate_route(&route)?;
        let _guard = lock_route(&route)?;
        Ok(Some(route))
    }
    pub fn bind(
        &self,
        thread: &str,
        route: &Path,
        pane_id: &str,
        pane_dir: &Path,
        epoch: &str,
    ) -> io::Result<()> {
        if !safe_uuid(thread) {
            return Err(invalid("invalid thread UUID"));
        }
        self.validate_route(route)?;
        let _lock = self.registry_lock()?;
        let mut guard = lock_route(route)?;
        let identity = Identity {
            v: 1,
            pane_id: pane_id.into(),
            pane_dir: pane_dir.into(),
            epoch: epoch.into(),
            active: true,
        };
        identity.validate()?;
        // The registry lock serializes route creation and binding. Inspect the actual records
        // instead of trusting a stale pane index: a killed launcher leaves its route behind.
        // Hold every matching record locked until the entire cutover has committed or rolled back.
        let mut displaced = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let candidate = entry.path();
            if candidate == route
                || !entry.file_name().to_str().is_some_and(|name| {
                    name.strip_prefix("route-").is_some_and(|suffix| {
                        suffix.len() == 32 && suffix.bytes().all(|b| b.is_ascii_hexdigit())
                    })
                })
            {
                continue;
            }
            let other = lock_route(&candidate)?;
            if other.identity.active
                && other.identity.pane_id == pane_id
                && other.identity.pane_dir == pane_dir
            {
                let previous = other.identity.clone();
                displaced.push((other, previous));
            }
        }
        let mapping = self
            .root
            .join(format!("thread-{}", thread.to_ascii_lowercase()));
        let temp = self.root.join(format!("mapping-{}", random()?));
        let mut f = open_file(&temp, true)?;
        let previous = guard.identity.clone();
        let mut route_changed = false;
        let result = (|| {
            f.write_all(
                route
                    .file_name()
                    .ok_or_else(|| invalid("missing route name"))?
                    .as_encoded_bytes(),
            )?;
            f.sync_data()?;
            guard.identity = identity;
            route_changed = true;
            guard.write()?;
            for (other, _) in &mut displaced {
                other.identity.active = false;
                other.write()?;
            }
            fs::rename(&temp, &mapping)?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temp);
            guard.identity = previous;
            let mut restore_error = if route_changed {
                guard.write().err()
            } else {
                None
            };
            for (other, previous) in &mut displaced {
                other.identity = previous.clone();
                if let Err(failure) = other.write() {
                    restore_error = Some(failure);
                }
            }
            if let Some(restore) = restore_error {
                return Err(io::Error::other(format!(
                    "binding failed ({error}) and restoring the previous route failed ({restore})"
                )));
            }
            return Err(error);
        }
        Ok(())
    }
    pub fn revoke(&self, route: &Path, epoch: &str) -> io::Result<()> {
        self.validate_route(route)?;
        let mut guard = lock_route(route)?;
        if guard.identity.epoch == epoch {
            guard.identity.active = false;
            guard.write()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn registry() -> (PathBuf, Registry) {
        let home = std::env::temp_dir().join(format!("ivy-route-test-{}", random().unwrap()));
        fs::create_dir(&home).unwrap();
        let registry = Registry::for_home(&home).unwrap();
        (home, registry)
    }
    #[test]
    fn binding_and_old_revocation() {
        let (home, r) = registry();
        let route = r.pending().unwrap();
        assert!(!lock_route(&route).unwrap().identity.active);
        let thread = "01234567-89ab-cdef-0123-456789abcdef";
        r.bind(thread, &route, "one", Path::new("/tmp/one"), "old")
            .unwrap();
        assert_eq!(r.route_for(thread).unwrap(), Some(route.clone()));
        r.bind(thread, &route, "two", Path::new("/tmp/two"), "new")
            .unwrap();
        r.revoke(&route, "old").unwrap();
        let guard = lock_route(&route).unwrap();
        assert!(guard.identity.active);
        assert_eq!(guard.identity.pane_id, "two");
        drop(guard);
        r.revoke(&route, "new").unwrap();
        assert!(!lock_route(&route).unwrap().identity.active);
        assert!(r.route_for("../../outside").is_err());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn failed_mapping_commit_restores_the_previous_record_and_removes_temporary_file() {
        let (home, r) = registry();
        let route = r.pending().unwrap();
        let thread = "01234567-89ab-cdef-0123-456789abcdef";
        r.bind(thread, &route, "old", Path::new("/tmp/old"), "old")
            .unwrap();
        let other_thread = "11234567-89ab-cdef-0123-456789abcdef";
        fs::create_dir(r.root.join(format!("thread-{other_thread}"))).unwrap();
        assert!(
            r.bind(other_thread, &route, "new", Path::new("/tmp/new"), "new")
                .is_err()
        );
        let guard = lock_route(&route).unwrap();
        assert_eq!(guard.identity.pane_id, "old");
        assert_eq!(guard.identity.epoch, "old");
        assert!(guard.identity.active);
        drop(guard);
        assert_eq!(r.route_for(thread).unwrap(), Some(route));
        assert!(!fs::read_dir(&r.root).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("mapping-")
        }));
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn binding_another_conversation_revokes_only_the_same_pane() {
        let (home, r) = registry();
        let old = r.pending().unwrap();
        let new = r.pending().unwrap();
        let other = r.pending().unwrap();
        let a = "01234567-89ab-cdef-0123-456789abcdef";
        let b = "11234567-89ab-cdef-0123-456789abcdef";
        let c = "21234567-89ab-cdef-0123-456789abcdef";
        r.bind(a, &old, "pane-a", Path::new("/tmp/pane-a"), "old")
            .unwrap();
        r.bind(c, &other, "pane-b", Path::new("/tmp/pane-b"), "other")
            .unwrap();
        r.bind(b, &new, "pane-a", Path::new("/tmp/pane-a"), "new")
            .unwrap();
        assert!(!lock_route(&old).unwrap().identity.active);
        assert!(lock_route(&new).unwrap().identity.active);
        assert!(lock_route(&other).unwrap().identity.active);
        r.revoke(&old, "old").unwrap();
        assert!(lock_route(&new).unwrap().identity.active);
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn failed_replacement_restores_the_displaced_pane() {
        let (home, r) = registry();
        let old = r.pending().unwrap();
        let new = r.pending().unwrap();
        let a = "01234567-89ab-cdef-0123-456789abcdef";
        let b = "11234567-89ab-cdef-0123-456789abcdef";
        r.bind(a, &old, "pane", Path::new("/tmp/pane"), "old")
            .unwrap();
        fs::create_dir(r.root.join(format!("thread-{b}"))).unwrap();
        assert!(
            r.bind(b, &new, "pane", Path::new("/tmp/pane"), "new")
                .is_err()
        );
        assert!(lock_route(&old).unwrap().identity.active);
        assert!(!lock_route(&new).unwrap().identity.active);
        assert_eq!(r.route_for(a).unwrap(), Some(old));
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn binding_waits_until_inflight_route_guard_is_released() {
        use std::sync::mpsc::sync_channel;
        let (home, r) = registry();
        let route = r.pending().unwrap();
        let guard = lock_route(&route).unwrap();
        let other_file = open_file(&route, false).unwrap();
        assert_eq!(
            other_file.try_lock_exclusive().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        let (started_tx, started_rx) = sync_channel(0);
        let (done_tx, done_rx) = sync_channel(0);
        let other_route = route.clone();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result = r.bind(
                "01234567-89ab-cdef-0123-456789abcdef",
                &other_route,
                "new",
                Path::new("/tmp/new"),
                "new",
            );
            done_tx.send(result).unwrap();
        });
        started_rx.recv().unwrap();
        assert!(done_rx.try_recv().is_err());
        drop(guard);
        done_rx.recv().unwrap().unwrap();
        worker.join().unwrap();
        assert_eq!(lock_route(&route).unwrap().identity.epoch, "new");
        fs::remove_dir_all(home).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlink_and_missing_routes_are_rejected() {
        use std::os::unix::fs::symlink;
        let (home, r) = registry();
        let route = r.pending().unwrap();
        let link = route.with_file_name(format!("route-{}", random().unwrap()));
        symlink(&route, &link).unwrap();
        assert!(lock_route(&link).is_err());
        fs::remove_file(&route).unwrap();
        assert!(lock_route(&route).is_err());
        fs::remove_dir_all(home).unwrap();
    }
}
