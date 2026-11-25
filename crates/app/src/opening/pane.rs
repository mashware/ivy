//! Shared pane construction for a launched program and a visible startup failure.
use super::*;

pub(super) fn assemble_pane(
    ctx: &egui::Context,
    name: usize,
    reads: control::ReadActivity,
    panel: Option<PanelDir>,
    integration: Option<shell_integration::Prepared>,
    mut pty: crate::launch::Process,
    output: std::sync::mpsc::Receiver<Vec<u8>>,
) -> Pane {
    pty.context = Some(ctx.clone());

    #[cfg(test)]
    crate::tests::completed_operation(crate::tests::OperationWork::Launch);

    let waker = ctx.clone();
    let wake: std::sync::Arc<dyn Fn() + Send + Sync> =
        std::sync::Arc::new(move || waker.request_repaint());
    let output_wake = std::sync::Arc::clone(&wake);
    let pty_output = PtyOutput::new(output, move || output_wake());
    let mut restoration = crate::restoration::PaneState {
        wake: Some(wake),
        ..Default::default()
    };
    let callback = restoration.route.callback();
    // Diagnostic attribution has its own lifetime and is installed regardless of whether session
    // restoration is enabled. The listener starts below, after the PTY root already exists.
    let provenance_inspector = pty
        .foreground_scope()
        .map(panel::provenance_route::inspector)
        .unwrap_or_else(|_| panel::provenance_route::unavailable());
    let (panel_state, control) = if pty.pending() || pty.failure.is_some() {
        (panel::PanelState::default(), None)
    } else {
        panel::pane::state_and_control_with_authority(
            ctx,
            panel.as_ref(),
            reads,
            Some(callback),
            pty.foreground_scope()
                .ok()
                .map(crate::panel::overlay::Authority::new),
            provenance_inspector,
        )
    };

    Pane {
        signals: Default::default(),
        name,
        label: None,
        project: None,
        program: None,
        restoration,
        pty,
        pty_output,
        _shell_integration: integration,
        image: Default::default(),
        panel,
        control,
        panel_state,
        to_reread: Flag::raised(),
        screen: Screen::new(INITIAL_GRID),
        painted_console: None,
        received: Vec::new(),
        keystrokes: Vec::new(),
        held: None,
        search: crate::search::OnThisConsole::default(),
        measured: INITIAL_GRID,
        timings: Timings::default(),
        pending_measurement: None,
        panel_reread_unpainted: std::time::Duration::ZERO,
        control_unpainted: std::time::Duration::ZERO,
        reflowed: std::time::Duration::ZERO,
        pending_wheel: WheelInput::default(),
        panel_width: panel::DEFAULT_WIDTH,
        folded: false,
        taken_over: None,
    }
}
