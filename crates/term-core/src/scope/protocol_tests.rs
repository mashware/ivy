//! A private message bus exercises the production manager transaction and observer.
use super::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, atomic::AtomicUsize};
use zbus::{fdo, object_server::SignalEmitter, zvariant::OwnedValue};

const UNIT: &str = "/org/freedesktop/systemd1/unit/fixture";
#[derive(Default)]
struct Calls {
    starts: Vec<(String, HashMap<String, OwnedValue>)>,
    retained: Vec<String>,
    released: Vec<String>,
}
struct Manager(Arc<Mutex<Calls>>);
#[zbus::interface(name = "org.freedesktop.systemd1.Manager")]
impl Manager {
    fn subscribe(&self) {}
    async fn start_transient_unit(
        &self,
        name: String,
        mode: String,
        properties: Vec<(String, OwnedValue)>,
        auxiliary: Vec<(String, Vec<(String, OwnedValue)>)>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<OwnedObjectPath> {
        assert_eq!(mode, "fail");
        assert!(auxiliary.is_empty());
        let properties: HashMap<_, _> = properties.into_iter().collect();
        let optional = properties.contains_key("OOMPolicy");
        self.0
            .lock()
            .unwrap()
            .starts
            .push((name.clone(), properties));
        if optional {
            return Err(fdo::Error::UnknownProperty("OOMPolicy".into()));
        }
        let job = OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/2").unwrap();
        Self::job_removed(&emitter, 2, &job, &name, "done")
            .await
            .unwrap();
        Ok(job)
    }
    fn get_unit(&self, _name: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(UNIT).unwrap()
    }
    fn ref_unit(&self, name: String) {
        self.0.lock().unwrap().retained.push(name);
    }
    fn unref_unit(&self, name: String) {
        self.0.lock().unwrap().released.push(name);
    }
    fn reset_failed_unit(&self, _name: &str) {}
    #[zbus(signal)]
    async fn job_removed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        job: &OwnedObjectPath,
        unit: &str,
        result: &str,
    ) -> zbus::Result<()>;
}
struct Unit {
    group: String,
    result: Arc<Mutex<String>>,
    queries: Arc<AtomicUsize>,
}
#[zbus::interface(name = "org.freedesktop.systemd1.Scope")]
impl Unit {
    #[zbus(property)]
    fn control_group(&self) -> &str {
        &self.group
    }
    #[zbus(property)]
    fn result(&self) -> String {
        self.queries.fetch_add(1, Ordering::SeqCst);
        self.result.lock().unwrap().clone()
    }
}
struct Bus {
    daemon: Child,
    _server: Connection,
    client: Connection,
    calls: Arc<Mutex<Calls>>,
    result: Arc<Mutex<String>>,
    queries: Arc<AtomicUsize>,
}
impl Bus {
    fn new() -> Self {
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let calls = Arc::new(Mutex::new(Calls::default()));
        let result = Arc::new(Mutex::new("success".to_owned()));
        let queries = Arc::new(AtomicUsize::new(0));
        let group = std::fs::read_to_string("/proc/self/cgroup")
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .unwrap()
            .to_owned();
        let (server, client) = future::block_on(async {
            let server = zbus::connection::Builder::address(address.trim())
                .unwrap()
                .name(DESTINATION)
                .unwrap()
                .serve_at(MANAGER, Manager(calls.clone()))
                .unwrap()
                .serve_at(
                    UNIT,
                    Unit {
                        group,
                        result: result.clone(),
                        queries: queries.clone(),
                    },
                )
                .unwrap()
                .build()
                .await
                .unwrap();
            let client = zbus::connection::Builder::address(address.trim())
                .unwrap()
                .build()
                .await
                .unwrap();
            (server, client)
        });
        Self {
            daemon,
            _server: server,
            client,
            calls,
            result,
            queries,
        }
    }
    fn scope(&self) -> Scope {
        Scope {
            connection: self.client.clone(),
            name: "fixture.scope".into(),
            path: UNIT.try_into().unwrap(),
        }
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < deadline, "fixture did not settle");
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
#[ignore = "private dbus-daemon requires Unix sockets; run explicitly outside the sandbox"]
fn unknown_oom_policy_retries_the_actual_protocol_and_retains_the_basic_scope() {
    let bus = Bus::new();
    let scope = future::block_on(bounded(
        Instant::now() + Duration::from_secs(2),
        place_connected(
            bus.client.clone(),
            std::process::id(),
            "fixture.scope",
            "literal identity",
        ),
    ))
    .unwrap();
    assert_eq!(scope.name, "fixture-basic.scope");
    {
        let calls = bus.calls.lock().unwrap();
        assert_eq!(calls.starts.len(), 2);
        assert_eq!(calls.starts[0].0, "fixture.scope");
        assert_eq!(calls.starts[1].0, "fixture-basic.scope");
        for (_, properties) in &calls.starts {
            assert_eq!(
                <&str>::try_from(&properties["Description"]).unwrap(),
                "Ivy console literal identity"
            );
            assert_eq!(<&str>::try_from(&properties["Slice"]).unwrap(), "app.slice");
            assert_eq!(
                Vec::<u32>::try_from(properties["PIDs"].try_clone().unwrap()).unwrap(),
                vec![std::process::id()]
            );
        }
        assert_eq!(
            <&str>::try_from(&calls.starts[0].1["OOMPolicy"]).unwrap(),
            "kill"
        );
        assert!(!calls.starts[1].1.contains_key("OOMPolicy"));
        assert_eq!(calls.retained, ["fixture-basic.scope"]);
        assert!(
            calls.released.is_empty(),
            "basic scope lost before observation"
        );
    }
    drop(scope);
    wait_until(|| !bus.calls.lock().unwrap().released.is_empty());
    assert_eq!(bus.calls.lock().unwrap().released, ["fixture-basic.scope"]);
}
#[test]
#[ignore = "private dbus-daemon requires Unix sockets; run explicitly outside the sandbox"]
fn observer_stops_at_the_real_500ms_cutoff_and_ignores_later_oom_evidence() {
    let bus = Bus::new();
    let (pty, _output) =
        crate::Pty::spawn("/bin/sh", &["-c", "exit 0"], crate::Size::default()).unwrap();
    let cached = pty.termination.clone();
    let started = Instant::now();
    let _stop = bus.scope().observe(cached.clone());
    assert_eq!(cached.snapshot(), crate::Termination::Pending);
    wait_until(|| cached.snapshot() != crate::Termination::Pending);
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(480),
        "observer ended early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(900),
        "observer exceeded cutoff: {elapsed:?}"
    );
    assert_eq!(cached.snapshot(), crate::Termination::Ordinary);
    wait_until(|| !bus.calls.lock().unwrap().released.is_empty());
    let queries = bus.queries.load(Ordering::SeqCst);
    assert!(queries >= 2, "observer did not query manager");
    *bus.result.lock().unwrap() = "oom-kill".into();
    std::thread::sleep(Duration::from_millis(120));
    assert_eq!(bus.queries.load(Ordering::SeqCst), queries);
    assert_eq!(cached.snapshot(), crate::Termination::Ordinary);
}
#[test]
#[ignore = "private dbus-daemon requires Unix sockets; run explicitly outside the sandbox"]
fn observer_delayed_evidence_and_bus_disconnect_use_actual_manager_results() {
    for disconnected in [false, true] {
        let mut bus = Bus::new();
        let (pty, _output) =
            crate::Pty::spawn("/bin/sh", &["-c", "kill -KILL $$"], crate::Size::default()).unwrap();
        let cached = pty.termination.clone();
        let _stop = bus.scope().observe(cached.clone());
        wait_until(|| bus.queries.load(Ordering::SeqCst) >= 2);
        assert_eq!(cached.snapshot(), crate::Termination::Pending);
        let changed = Instant::now();
        if disconnected {
            bus.daemon.kill().unwrap();
        } else {
            *bus.result.lock().unwrap() = "oom-kill".into();
        }
        wait_until(|| cached.snapshot() != crate::Termination::Pending);
        assert!(
            changed.elapsed() < Duration::from_millis(250),
            "observer did not settle promptly"
        );
        assert_eq!(
            cached.snapshot(),
            if disconnected {
                crate::Termination::Signal
            } else {
                crate::Termination::Oom
            }
        );
    }
}
