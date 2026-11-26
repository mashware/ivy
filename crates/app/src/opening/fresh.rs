use super::*;

pub(crate) fn create(
    window: &mut Window,
    ctx: &egui::Context,
    selection: crate::launch::fresh::Selection,
) -> Result<(), String> {
    let before = Instant::now();
    let reads = window.control.read_activity();
    let panel = match PanelDir::create() {
        Ok(panel) => Some(panel),
        Err(error) => {
            window.timed_separately += before.elapsed();
            return Err(format!(
                "The agent console's panel could not be created: {error}"
            ));
        }
    };
    let mut identity = crate::panel::pane::launch_identity(panel.as_ref());
    let helper = crate::resume::helper_path();
    identity.push((
        "IVY_SESSION_HELPER",
        helper.as_deref().map(std::path::Path::as_os_str),
    ));
    let env = identity
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.map(std::ffi::OsStr::to_owned)))
        .collect();
    let wake = ctx.clone();
    let opened = crate::launch::fresh::start(
        std::sync::Arc::new(move || wake.request_repaint()),
        reads.clone(),
        selection,
        INITIAL_GRID,
        env,
    )
    .map(|(pty, output)| {
        let pane = assemble_pane(ctx, window.next_pane_name, reads, panel, None, pty, output);
        let layout = OpeningLayout {
            columns: 1,
            rows: 1,
        };
        Tab::with_panes(vec![pane], &layout, window.next_name)
    });
    window.next_name += 1;
    window.next_pane_name += 1;
    let result = match opened {
        Ok(tab) => {
            window.bring_to_front(Ok(tab), ctx);
            Ok(())
        }
        Err(error) => Err(format!("The agent console could not be opened: {error}")),
    };
    window.timed_separately += before.elapsed();
    result
}
