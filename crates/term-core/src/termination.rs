//! Cached process status, separate from the terminal reader's lifetime.
use portable_pty::{Child, ChildKiller, ExitStatus};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Confirmed process termination and, where available, scope evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    /// The root is alive, or its scope's final result is still being observed.
    Pending,
    /// The root exited without a signal; no memory-pressure evidence was found.
    Ordinary,
    /// The collected wait status explicitly records a signal.
    Signal,
    /// This console's retained systemd scope explicitly reported `oom-kill`.
    Oom,
}

#[derive(Debug)]
struct State {
    child: Box<dyn Child + Send + Sync>,
    status: Option<ExitStatus>,
    exited: Option<Instant>,
    observing: bool,
    closing: bool,
    oom: bool,
}

/// The same cached wait status serves existing callers and the scope observer.
#[derive(Clone, Debug)]
pub(crate) struct CachedChild(Arc<Mutex<State>>);
impl CachedChild {
    pub(crate) fn new(child: Box<dyn Child + Send + Sync>) -> Self {
        Self(Arc::new(Mutex::new(State {
            child,
            status: None,
            exited: None,
            observing: false,
            closing: false,
            oom: false,
        })))
    }
    pub(crate) fn snapshot(&self) -> Termination {
        let mut state = self.0.lock().unwrap();
        let _ = collect(&mut state);
        if state.oom {
            Termination::Oom
        } else if state.status.is_none() || state.observing {
            Termination::Pending
        } else if state.status.as_ref().is_some_and(|s| s.signal().is_some()) {
            Termination::Signal
        } else {
            Termination::Ordinary
        }
    }
    /// Retire observer reaping before closing starts checking and signalling the owned pid.
    /// The lock also waits for a collection already in flight; the stop flag alone cannot do so.
    pub(crate) fn begin_close(&self) {
        self.0.lock().unwrap().closing = true;
    }
    #[cfg(target_os = "linux")]
    pub(crate) fn exited(&self) -> Option<Instant> {
        let mut state = self.0.lock().unwrap();
        if !state.closing {
            let _ = collect(&mut state);
        }
        state.exited
    }
    #[cfg(any(test, target_os = "linux"))]
    pub(crate) fn observe(&self, observing: bool, oom: bool) {
        let mut state = self.0.lock().unwrap();
        state.observing = observing;
        state.oom |= oom;
    }
}
fn collect(state: &mut State) -> io::Result<Option<ExitStatus>> {
    if state.status.is_none()
        && let Some(status) = state.child.try_wait()?
    {
        state.status = Some(status);
        state.exited = Some(Instant::now());
    }
    Ok(state.status.clone())
}
impl Child for CachedChild {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        collect(&mut self.0.lock().unwrap())
    }
    fn wait(&mut self) -> io::Result<ExitStatus> {
        let mut state = self.0.lock().unwrap();
        if let Some(status) = &state.status {
            return Ok(status.clone());
        }
        // Only Pty's closing paths call wait. The scope stop flag is already raised, and a
        // program left running hands this handle to its detached reaper before waiting. Holding the
        // cache lock here parks only that reaper (and a retiring scope observer), never a live UI
        // accessor. Preserve the native wait instead of waking once per millisecond indefinitely.
        //
        // Abandoned on purpose: a child left running belongs to the detached reaper until it exits.
        let status = state.child.wait()?;
        state.status = Some(status.clone());
        state.exited = Some(Instant::now());
        Ok(status)
    }
    fn process_id(&self) -> Option<u32> {
        self.0.lock().unwrap().child.process_id()
    }
    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        self.0.lock().unwrap().child.as_raw_handle()
    }
}
impl ChildKiller for CachedChild {
    fn kill(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().child.kill()
    }
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        self.0.lock().unwrap().child.clone_killer()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[cfg(unix)]
    use std::time::Duration;

    #[cfg(target_os = "linux")]
    #[derive(Debug)]
    struct ReapOnExit {
        exited: Arc<std::sync::atomic::AtomicBool>,
        reaped: Arc<AtomicUsize>,
    }
    #[cfg(target_os = "linux")]
    impl ChildKiller for ReapOnExit {
        fn kill(&mut self) -> io::Result<()> {
            Ok(())
        }
        fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
            unreachable!("closing does not clone this fixture's killer")
        }
    }
    #[cfg(target_os = "linux")]
    impl Child for ReapOnExit {
        fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            Ok(self.exited.load(Ordering::SeqCst).then(|| {
                self.reaped.fetch_add(1, Ordering::SeqCst);
                ExitStatus::with_exit_code(0)
            }))
        }
        fn wait(&mut self) -> io::Result<ExitStatus> {
            unreachable!("the fixture collects only through try_wait")
        }
        fn process_id(&self) -> Option<u32> {
            Some(123)
        }
    }
    crate::windows_unix_omission!(
        observer_cannot_reap_between_the_close_status_check_and_signal,
        "unix-only: requires Linux observer reaping behavior"
    );
    #[cfg(target_os = "linux")]
    #[test]
    fn observer_cannot_reap_between_the_close_status_check_and_signal() {
        let exited = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reaped = Arc::new(AtomicUsize::new(0));
        let mut child = CachedChild::new(Box::new(ReapOnExit {
            exited: exited.clone(),
            reaped: reaped.clone(),
        }));
        let observer = child.clone();
        child.begin_close();
        assert!(child.try_wait().unwrap().is_none());
        // Force the child to exit after closing's final check, then run the observer to
        // completion before the signal. This order used to reap the pid before signalling it.
        exited.store(true, Ordering::SeqCst);
        let observed = std::thread::spawn(move || observer.exited())
            .join()
            .unwrap();
        assert_eq!(observed, None);
        assert_eq!(
            reaped.load(Ordering::SeqCst),
            0,
            "pid became reusable before signal"
        );
        // Closing still owns reaping and its cached result remains repeatable.
        assert!(child.try_wait().unwrap().is_some());
        assert!(child.try_wait().unwrap().is_some());
        assert_eq!(reaped.load(Ordering::SeqCst), 1);
    }
    crate::windows_unix_omission!(
        dropping_a_pty_retires_observer_reaping_before_the_close_signal,
        "unix-only: requires Linux observer reaping behavior"
    );
    #[cfg(target_os = "linux")]
    #[test]
    fn dropping_a_pty_retires_observer_reaping_before_the_close_signal() {
        let (pty, _output) =
            crate::Pty::spawn("/bin/sleep", &["30"], crate::Size::default()).unwrap();
        let observer = pty.termination.clone();
        drop(pty);
        assert!(observer.0.lock().unwrap().closing);
    }

    #[derive(Debug)]
    struct NativeWaitOnly(Arc<AtomicUsize>);
    impl ChildKiller for NativeWaitOnly {
        fn kill(&mut self) -> io::Result<()> {
            Ok(())
        }
        fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
            Box::new(Self(self.0.clone()))
        }
    }
    impl Child for NativeWaitOnly {
        fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            panic!("the detached reaper must use native wait, not status polling");
        }
        fn wait(&mut self) -> io::Result<ExitStatus> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ExitStatus::with_exit_code(7))
        }
        fn process_id(&self) -> Option<u32> {
            None
        }
        #[cfg(windows)]
        fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
            None
        }
    }
    #[test]
    fn wait_uses_native_blocking_wait_once_and_caches_its_status() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut child = CachedChild::new(Box::new(NativeWaitOnly(calls.clone())));
        let mut other = child.clone();
        assert_eq!(child.wait().unwrap().exit_code(), 7);
        assert_eq!(other.wait().unwrap().exit_code(), 7);
        assert_eq!(other.try_wait().unwrap().unwrap().exit_code(), 7);
        assert_eq!(other.snapshot(), Termination::Ordinary);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[derive(Debug)]
    struct Finished(ExitStatus);
    impl ChildKiller for Finished {
        fn kill(&mut self) -> io::Result<()> {
            Ok(())
        }
        fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
            Box::new(Finished(self.0.clone()))
        }
    }
    impl Child for Finished {
        fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            Ok(Some(self.0.clone()))
        }
        fn wait(&mut self) -> io::Result<ExitStatus> {
            Ok(self.0.clone())
        }
        fn process_id(&self) -> Option<u32> {
            None
        }
        #[cfg(windows)]
        fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
            None
        }
    }
    crate::windows_unix_omission!(
        actual_child_status_distinguishes_sigkill_from_exit_137,
        "unix-only: requires Unix signal exit-status semantics"
    );
    #[cfg(unix)]
    #[test]
    fn actual_child_status_distinguishes_sigkill_from_exit_137() {
        use crate::{Pty, Size};
        for (script, expected) in [
            ("kill -KILL $$", Termination::Signal),
            ("exit 137", Termination::Ordinary),
        ] {
            let (mut pty, _output) =
                Pty::spawn("/bin/sh", &["-c", script], Size { rows: 4, cols: 20 }).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            while pty.termination() == Termination::Pending && Instant::now() < deadline {
                // Bounded by the owned shell's status deadline.
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(pty.termination(), expected);
            assert_eq!(pty.exit_success(), Some(false));
            assert_eq!(pty.exit_success(), Some(false));
        }
    }
    #[test]
    fn status_137_without_signal_is_ordinary_and_status_is_repeatable() {
        let mut child = CachedChild::new(Box::new(Finished(ExitStatus::with_exit_code(137))));
        assert_eq!(child.snapshot(), Termination::Ordinary);
        assert_eq!(child.try_wait().unwrap().unwrap().exit_code(), 137);
        assert_eq!(child.try_wait().unwrap().unwrap().exit_code(), 137);
    }
    #[test]
    fn explicit_signal_without_scope_evidence_is_never_oom() {
        let child = CachedChild::new(Box::new(Finished(ExitStatus::with_signal("Killed"))));
        assert_eq!(child.snapshot(), Termination::Signal);
    }
    #[test]
    fn delayed_positive_scope_evidence_wins_over_signal_status() {
        let child = CachedChild::new(Box::new(Finished(ExitStatus::with_signal("Killed"))));
        child.observe(true, false);
        assert_eq!(child.snapshot(), Termination::Pending);
        child.observe(false, true);
        assert_eq!(child.snapshot(), Termination::Oom);
        child.observe(false, false);
        assert_eq!(child.snapshot(), Termination::Oom);
    }
    #[test]
    fn unavailable_scope_does_not_hide_collected_signal() {
        let child = CachedChild::new(Box::new(Finished(ExitStatus::with_signal("Terminated"))));
        child.observe(true, false);
        assert_eq!(child.snapshot(), Termination::Pending);
        child.observe(false, false);
        assert_eq!(child.snapshot(), Termination::Signal);
    }
}
