pub(super) use crate::client_attestation::Root;

pub(super) fn root(
    scope: &term_core::ForegroundScope,
    peer: &panel_control::session::Peer,
    programs: &[crate::client_attestation::Program],
) -> std::io::Result<Root> {
    crate::client_attestation::root(scope, peer, programs)
}
