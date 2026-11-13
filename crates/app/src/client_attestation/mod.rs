//! Shared, fail-closed recognition of supported native agent invocations.

mod programs;
mod verify;

use panel_control::session::{Client, Peer};
use std::io;
use std::path::PathBuf;
use term_core::ForegroundScope;

pub(crate) use programs::{Program, ResolvedLaunch};
pub(crate) use verify::Root;

pub(crate) fn discover() -> Vec<Program> {
    programs::discover()
}
pub(crate) fn supplied(programs: Vec<(Client, PathBuf)>) -> Vec<Program> {
    programs::supplied(programs)
}
pub(crate) fn resolve_launch(client: Client) -> io::Result<ResolvedLaunch> {
    programs::resolve_launch(client)
}
pub(crate) fn root(scope: &ForegroundScope, peer: &Peer, programs: &[Program]) -> io::Result<Root> {
    verify::root(scope, peer, programs)
}

pub(crate) struct Programs(Vec<Program>);

impl Programs {
    pub(crate) fn discover() -> Self {
        Self(programs::discover())
    }

    pub(crate) fn root(&self, scope: &ForegroundScope, peer: &Peer) -> io::Result<Root> {
        verify::root(scope, peer, &self.0)
    }

    #[cfg(test)]
    pub(crate) fn supplied(programs: Vec<(Client, PathBuf)>) -> Self {
        Self(programs::supplied(programs))
    }
}

#[cfg(target_os = "linux")]
pub(crate) use programs::{pending_children, reap_children};
