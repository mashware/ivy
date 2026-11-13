//! Optional off-frame services share the pane endpoint without changing panel ownership.
use super::*;

pub(crate) fn open_with_sessions(
    dir: &PanelDir,
    reader: crate::panel::DocumentReader,
    reads: ReadActivity,
    sessions: Option<panel_control::session::SessionHandler>,
    wake: impl Fn() + Send + Sync + 'static,
) -> Option<PaneControl> {
    open_services(dir, reader, reads, sessions, None, wake)
}

pub(crate) fn open_with_authority(
    dir: &PanelDir,
    state: &PanelState,
    reads: ReadActivity,
    sessions: Option<panel_control::session::SessionHandler>,
    authority: Arc<crate::panel::overlay::Authority>,
    wake: impl Fn() + Send + Sync + 'static,
) -> Option<PaneControl> {
    open_services(
        dir,
        state.reader(),
        reads,
        sessions,
        Some((state, authority)),
        wake,
    )
}

fn open_services(
    dir: &PanelDir,
    reader: crate::panel::DocumentReader,
    reads: ReadActivity,
    sessions: Option<panel_control::session::SessionHandler>,
    authorization: Option<(&PanelState, Arc<crate::panel::overlay::Authority>)>,
    wake: impl Fn() + Send + Sync + 'static,
) -> Option<PaneControl> {
    let (sender, inbox) = sync_channel(MAX_PENDING);
    #[cfg(test)]
    let test_sender = sender.clone();
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
    let read_document = measured_reader(reader, reads.clone());
    let (publisher, authority) = authorization.map_or((None, None), |(state, authority)| {
        let handler = crate::panel::overlay::service(
            state,
            Arc::clone(&authority),
            sender.clone(),
            Arc::clone(&wake),
        );
        (Some(handler), Some(authority))
    });
    match Channel::open_with_authorization_services(
        dir.path(),
        dir.id(),
        sender,
        Some(read_document),
        sessions,
        publisher,
        Arc::clone(&wake),
    ) {
        Ok(channel) => {
            channel.set_plan_reader(Arc::new(|| {
                panel_control::Reply::Failed("this console's project is not known yet".into())
            }));
            Some(PaneControl {
                authority,
                plan_identity: None,
                _channel: channel,
                inbox,
                #[cfg(test)]
                test_sender,
                wake,
                waiting_on_the_window: Vec::new(),
                did: Control::default(),
                reads,
            })
        }
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

impl PaneControl {
    pub(crate) fn install_plan_reader(&self, reader: panel_control::listener::ReadPlan) {
        self._channel.set_plan_reader(reader);
    }
}
