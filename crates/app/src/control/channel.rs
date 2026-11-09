//! Optional off-frame services share the pane endpoint without changing panel ownership.
use super::*;

pub(crate) fn open_with_sessions(
    dir: &PanelDir,
    reader: crate::panel::DocumentReader,
    reads: ReadActivity,
    sessions: Option<panel_control::session::SessionHandler>,
    wake: impl Fn() + Send + Sync + 'static,
) -> Option<PaneControl> {
    let (sender, inbox) = sync_channel(MAX_PENDING);
    #[cfg(test)]
    let test_sender = sender.clone();
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
    let read_document = measured_reader(reader, reads.clone());
    match Channel::open_with_services(
        dir.path(),
        dir.id(),
        sender,
        Some(read_document),
        sessions,
        Arc::clone(&wake),
    ) {
        Ok(channel) => Some(PaneControl {
            _channel: channel,
            inbox,
            #[cfg(test)]
            test_sender,
            wake,
            waiting_on_the_window: Vec::new(),
            did: Control::default(),
            reads,
        }),
        // **Binding nothing on a platform that cannot assert who may connect is policy, not a
        // fault, so it is not reported.** Said once per pane at every start it reads as a broken
        // terminal, and the person reading it has nothing to do about it: the platform gets its
        // channel when F2.7 can put a permission on the endpoint.
        Err(failure) if failure.kind() == std::io::ErrorKind::Unsupported => None,
        Err(failure) => {
            eprintln!(
                "console {}: it goes without a control channel, its endpoint could not be bound ({failure})",
                dir.id()
            );
            None
        }
    }
}
