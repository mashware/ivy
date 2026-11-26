//! D90: pathname resolution may wait on a filesystem after transport cancellation.
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    mpsc::{RecvTimeoutError, sync_channel},
};
use std::time::{Duration, Instant};

const LIMIT: usize = 8;
pub(super) const DEADLINE: Duration = Duration::from_millis(750);
const STOP_POLL: Duration = Duration::from_millis(10);
static UNFINISHED: AtomicUsize = AtomicUsize::new(0);
static TIMED_OUT: AtomicU64 = AtomicU64::new(0);
static BUSY: AtomicU64 = AtomicU64::new(0);
static CANCELLED: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
pub(super) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Process-wide inspection jobs, including jobs whose caller has already timed out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowsInspectionStats {
    pub unfinished: usize,
    pub timed_out: u64,
    pub busy: u64,
    pub cancelled: u64,
}

pub fn windows_inspection_stats() -> WindowsInspectionStats {
    WindowsInspectionStats {
        unfinished: UNFINISHED.load(Ordering::Acquire),
        timed_out: TIMED_OUT.load(Ordering::Relaxed),
        busy: BUSY.load(Ordering::Relaxed),
        cancelled: CANCELLED.load(Ordering::Relaxed),
    }
}

#[derive(Default)]
pub(super) struct Lane(Arc<AtomicBool>);

struct Permit(Arc<AtomicBool>);
impl Permit {
    fn acquire(lane: &Lane) -> io::Result<Self> {
        if lane.0.swap(true, Ordering::AcqRel) {
            BUSY.fetch_add(1, Ordering::Relaxed);
            return Err(io::ErrorKind::WouldBlock.into());
        }
        if UNFINISHED
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < LIMIT).then_some(n + 1)
            })
            .is_err()
        {
            lane.0.store(false, Ordering::Release);
            BUSY.fetch_add(1, Ordering::Relaxed);
            return Err(io::ErrorKind::WouldBlock.into());
        }
        Ok(Self(Arc::clone(&lane.0)))
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        UNFINISHED.fetch_sub(1, Ordering::AcqRel);
        self.0.store(false, Ordering::Release);
    }
}

// Only the native invocation module supplies production operations. No publication, window,
// callback or document capability belongs in an inspection job (D90).
pub(super) fn run<T: Send + 'static>(
    lane: &Lane,
    stop: &AtomicBool,
    live: impl Fn() -> bool,
    budget: Duration,
    operation: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
    if stop.load(Ordering::Acquire) || !live() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let permit = Permit::acquire(lane)?;
    let (send, receive) = sync_channel(1);
    let deadline = Instant::now() + budget;
    std::thread::Builder::new()
        .name("native-agent-inspection".into())
        .spawn(move || {
            // A timeout must not release this permit while filesystem I/O is still running.
            let result = operation();
            drop(permit);
            let _ = send.send(result);
        })?;
    loop {
        if stop.load(Ordering::Acquire) || !live() {
            CANCELLED.fetch_add(1, Ordering::Relaxed);
            return Err(io::ErrorKind::Interrupted.into());
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            TIMED_OUT.fetch_add(1, Ordering::Relaxed);
            return Err(io::ErrorKind::TimedOut.into());
        };
        match receive.recv_timeout(remaining.min(STOP_POLL)) {
            Ok(result) => {
                if stop.load(Ordering::Acquire) || !live() {
                    CANCELLED.fetch_add(1, Ordering::Relaxed);
                    return Err(io::ErrorKind::Interrupted.into());
                }
                if Instant::now() >= deadline {
                    TIMED_OUT.fetch_add(1, Ordering::Relaxed);
                    return Err(io::ErrorKind::TimedOut.into());
                }
                return result;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(io::Error::other(
                    "native inspection did not return a result",
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{Receiver, Sender, channel};

    struct Blocked {
        result: Receiver<Result<u8, io::ErrorKind>>,
        release: Option<Sender<()>>,
        caller: Option<std::thread::JoinHandle<()>>,
        baseline: usize,
    }
    impl Drop for Blocked {
        fn drop(&mut self) {
            self.release.take();
            if let Some(caller) = self.caller.take() {
                caller.join().unwrap();
            }
            wait_for_unfinished(self.baseline);
        }
    }

    fn wait_for_unfinished(expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while UNFINISHED.load(Ordering::Acquire) != expected {
            assert!(
                Instant::now() < deadline,
                "inspection permit did not return"
            );
            std::thread::yield_now();
        }
    }

    fn blocked(lane: Arc<Lane>, stop: Arc<AtomicBool>, budget: Duration) -> Blocked {
        let baseline = UNFINISHED.load(Ordering::Acquire);
        let (release, released) = channel();
        let (started, entered) = channel();
        let (send, result) = channel();
        let caller = std::thread::spawn(move || {
            let result = run(
                &lane,
                &stop,
                || true,
                budget,
                move || {
                    started.send(()).unwrap();
                    released.recv().map_err(|_| io::ErrorKind::Interrupted)?;
                    Ok(9)
                },
            );
            let _ = send.send(result.map_err(|e| e.kind()));
        });
        let job = Blocked {
            result,
            release: Some(release),
            caller: Some(caller),
            baseline,
        };
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        job
    }

    #[test]
    fn timeout_does_not_free_a_still_running_native_operation() {
        let _guard = TEST_LOCK.lock().unwrap();
        let lane = Arc::new(Lane::default());
        let stop = Arc::new(AtomicBool::new(false));
        let job = blocked(Arc::clone(&lane), stop, Duration::from_millis(20));
        assert_eq!(
            job.result.recv_timeout(Duration::from_secs(1)).unwrap(),
            Err(io::ErrorKind::TimedOut)
        );
        assert_eq!(UNFINISHED.load(Ordering::Acquire), job.baseline + 1);
        let ran = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&ran);
        let result = run(
            &lane,
            &AtomicBool::new(false),
            || true,
            DEADLINE,
            move || {
                observed.store(true, Ordering::Release);
                Ok(())
            },
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
        assert!(!ran.load(Ordering::Acquire));
    }

    #[test]
    fn stop_returns_without_waiting_for_the_native_operation() {
        let _guard = TEST_LOCK.lock().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let job = blocked(
            Arc::new(Lane::default()),
            Arc::clone(&stop),
            Duration::from_secs(5),
        );
        stop.store(true, Ordering::Release);
        assert_eq!(
            job.result.recv_timeout(Duration::from_secs(1)).unwrap(),
            Err(io::ErrorKind::Interrupted)
        );
        assert_eq!(UNFINISHED.load(Ordering::Acquire), job.baseline + 1);
    }

    #[test]
    fn unfinished_jobs_are_bounded_across_distinct_invocations() {
        let _guard = TEST_LOCK.lock().unwrap();
        let baseline = UNFINISHED.load(Ordering::Acquire);
        assert_eq!(baseline, 0);
        let lanes: Vec<_> = (0..=LIMIT).map(|_| Lane::default()).collect();
        let permits: Vec<_> = lanes[..LIMIT]
            .iter()
            .map(|lane| Permit::acquire(lane).unwrap())
            .collect();
        assert!(
            matches!(Permit::acquire(&lanes[LIMIT]), Err(e) if e.kind() == io::ErrorKind::WouldBlock)
        );
        assert_eq!(windows_inspection_stats().unfinished, LIMIT);
        drop(permits);
        assert_eq!(windows_inspection_stats().unfinished, baseline);
    }

    #[test]
    fn a_completed_inspection_returns_its_result_and_permit() {
        let _guard = TEST_LOCK.lock().unwrap();
        let baseline = UNFINISHED.load(Ordering::Acquire);
        assert_eq!(
            run(
                &Lane::default(),
                &AtomicBool::new(false),
                || true,
                DEADLINE,
                || Ok(7)
            )
            .unwrap(),
            7
        );
        wait_for_unfinished(baseline);
    }
}
