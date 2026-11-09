use eframe::egui;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use super::feed::{self, Release};

#[derive(Debug, Default)]
pub(super) struct Service {
    initialized: bool,
    receiver: Option<Receiver<Result<Vec<Release>, String>>>,
    releases: Vec<Release>,
    status: String,
}

impl Service {
    pub(super) fn initialize(&mut self, ctx: &egui::Context) {
        if !self.initialized {
            self.initialized = true;
            self.check(ctx);
        }
    }

    pub(super) fn check(&mut self, ctx: &egui::Context) {
        self.start(ctx, feed::fetch);
    }

    fn start(
        &mut self,
        ctx: &egui::Context,
        fetch: impl FnOnce() -> Result<Vec<Release>, String> + Send + 'static,
    ) {
        if self.receiver.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        match std::thread::Builder::new()
            .name("ivy-update-check".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(fetch))
                    .unwrap_or_else(|_| Err("Update check stopped unexpectedly.".into()));
                let _ = sender.send(result);
                ctx.request_repaint();
            }) {
            Ok(_) => {
                self.receiver = Some(receiver);
                self.status = "Checking for updates…".into();
            }
            Err(e) => self.status = format!("Could not start update check: {e}"),
        }
    }

    pub(super) fn poll(&mut self) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(releases)) => {
                self.accept(releases);
                self.receiver = None;
            }
            Ok(Err(error)) => {
                self.status = error;
                self.receiver = None;
            }
            Err(TryRecvError::Disconnected) => {
                self.status = "Update check stopped unexpectedly.".into();
                self.receiver = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }
    fn accept(&mut self, releases: Vec<Release>) {
        self.releases = releases;
        self.status = if self.releases.is_empty() {
            "No stable releases found."
        } else if self.available().is_some() {
            "An update is available."
        } else {
            "Ivy is up to date."
        }
        .into();
    }

    #[cfg(test)]
    pub(super) fn fixture(&mut self, releases: Vec<Release>) {
        self.accept(releases);
    }

    pub(super) fn status(&self) -> &str {
        &self.status
    }
    pub(super) fn checking(&self) -> bool {
        self.receiver.is_some()
    }
    pub(super) fn releases(&self) -> &[Release] {
        &self.releases
    }
    pub(super) fn available(&self) -> Option<&Release> {
        let installed = semver::Version::parse(env!("CARGO_PKG_VERSION")).ok()?;
        self.releases
            .iter()
            .find(|release| release.version.cmp_precedence(&installed).is_gt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait(service: &mut Service) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while service.checking() {
            assert!(std::time::Instant::now() < deadline, "worker timed out");
            service.poll();
            std::thread::yield_now();
        }
    }
    #[test]
    fn single_inflight_and_success_then_error() {
        let mut service = Service::default();
        let ctx = egui::Context::default();
        let (send, recv) = mpsc::channel();
        service.start(&ctx, move || {
            recv.recv().unwrap();
            Ok(Vec::new())
        });
        service.start(&ctx, || panic!("second check must not start"));
        assert!(service.checking());
        send.send(()).unwrap();
        wait(&mut service);
        assert_eq!(service.status(), "No stable releases found.");
        service.start(&ctx, || Err("offline".into()));
        wait(&mut service);
        assert_eq!(service.status(), "offline");
    }
    #[test]
    fn available_compares_semver_precedence() {
        let mut service = Service::default();
        let release = |version: &str| Release {
            version: semver::Version::parse(version).unwrap(),
            name: String::new(),
            notes: String::new(),
            url: String::new(),
        };
        service.releases = vec![release("0.0.1")];
        assert!(service.available().is_none());
        let installed = env!("CARGO_PKG_VERSION");
        service.releases = vec![release(installed)];
        assert!(service.available().is_none());
        service.releases = vec![release(&format!("{installed}+build"))];
        assert!(service.available().is_none());
        let mut newer = semver::Version::parse(installed).unwrap();
        newer.major += 1;
        service.releases = vec![release(&newer.to_string())];
        assert!(service.available().is_some());
    }
    #[test]
    fn disconnected_worker_is_reported() {
        let mut service = Service::default();
        service.start(&egui::Context::default(), || panic!("worker failed"));
        wait(&mut service);
        assert_eq!(service.status(), "Update check stopped unexpectedly.");
    }
}
