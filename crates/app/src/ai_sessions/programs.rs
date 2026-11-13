pub(super) use crate::client_attestation::Program;
pub(crate) use crate::client_attestation::ResolvedLaunch;

pub(crate) fn resolve_launch(
    client: panel_control::session::Client,
) -> std::io::Result<ResolvedLaunch> {
    crate::client_attestation::resolve_launch(client)
}

pub(super) fn discover() -> Vec<Program> {
    crate::client_attestation::discover()
}

pub(super) fn supplied(
    programs: Vec<(panel_control::session::Client, std::path::PathBuf)>,
) -> Vec<Program> {
    crate::client_attestation::supplied(programs)
}

pub(super) fn reap_children() {
    crate::client_attestation::reap_children();
}

pub(super) fn pending_children() -> (Vec<std::sync::Arc<std::os::fd::OwnedFd>>, bool) {
    crate::client_attestation::pending_children()
}
