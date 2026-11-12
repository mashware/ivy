//! Startup policy is resolved before any restored process is opened.
use crate::{
    APP_NAME, InjectedDrags, PANES_VARIABLE, TABS_VARIABLE, any_measurement_variable, board,
    chrome, gesture_at_startup, header, injected, ligatures, opening, project_view, restoration,
    session, settings, size_at_startup,
};

mod migration;

pub fn run() -> eframe::Result {
    let measuring = any_measurement_variable(|which| std::env::var_os(which).is_some());
    migration::prepare(measuring).map_err(|error| eframe::Error::AppCreation(Box::new(error)))?;
    // **The file is read HERE and not inside the first frame**, and it is a decision: reading from
    // disk inside the path that is measured would put a disk wait into the frame in exchange for
    // nothing. What is read comes in through the `move` of the closure below, already interpreted
    // and already validated.
    //
    // **And the one that loads is the one that keeps the window**, which was the debt the previous
    // PR left written: the veto of a newer version lives in it, so a second instance would lose it
    // and would clobber the layout of a newer binary.
    let mut persistence = session::Persistence::from_env();

    // **A measurement session does not step on the layout of whoever measures.** The two variables
    // below open what the batch says instead of what the user left, so saving would write **the
    // batch's layout** over it. The measurement script already isolates the configuration
    // directory, but whoever launches the binary by hand with those variables —which is how the
    // documentation says to open a particular layout— does not.
    let tabs = std::env::var(TABS_VARIABLE).ok();
    let panes = std::env::var(PANES_VARIABLE).ok();
    // **Any of the measurement gate's variables vetoes the writing, not only the two that ask for
    // consoles.** With the check put only on those two, **the batch that restores was the only
    // measurement session with saving alive** —it does not set them on purpose, so that the file
    // rules—, so it could write to disk during the run while its control batch had writing vetoed:
    // two different regimes being compared row by row in the annex. And the script only demands
    // that the writes be declared in the batches that drag, so nobody would have caught that one's.
    if measuring {
        persistence.forbid_overwrite("a measurement batch asked for this session");
    }

    // **The settings are read here, and are NOT read under measurement.** The second is what
    // matters: the measurement script isolates the configuration directory, but whoever launches
    // the binary by hand with those variables would read theirs — and their font size would change
    // the grid, which is what all the project's published figures depend on. Colors do not change
    // it; the whole file is vetoed all the same, because the rule is read at a glance and «only
    // these settings» would be a list that forgets to grow.
    let entry = if measuring {
        settings::Entry::Measuring
    } else {
        settings::Entry::Normal
    };
    let (settings, what_it_cost) = settings::to_use(entry, settings::path().as_deref());
    let (with_what, restoration) = restoration::prepare(
        &mut persistence,
        session::path().as_deref(),
        settings.startup_restore,
        measuring,
        tabs.as_deref(),
        panes.as_deref(),
    );

    let integrated = !measuring || std::env::var_os("MEASURE_CHROME").is_some();
    let mut options = chrome::native_options(integrated);
    // **The size is only asked for if somebody asked for it**, and that is why there is no default
    // size here: without the variable, `NativeOptions::default()` carries no `inner_size` and the
    // desktop decides, which is the behavior of always. Restoring **does not touch this**: the
    // window's size is not saved, because leaving it to the desktop is what makes the measurements
    // comparable. See [`crate::SIZE_VARIABLE`].
    let requested = size_at_startup();
    let injected_drags = injected::drags_at_startup();
    // **Read here with the rest of the environment, and not inside the closure**, which is where
    // every other gate of this function is read. It is **applied** to the window after it is built,
    // like the theme and the injected gestures beside it — `Window::opening` is born painting strips
    // and a measured session takes them away, so no test depends on what the session that launched
    // it exported.
    let wants_headers = std::env::var_os(header::HEADER_VARIABLE).is_some();
    let project_probe = project_view::probe_at_startup(measuring);
    if let Some(size) = requested {
        options.viewport = options.viewport.with_inner_size(size);
    }

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            let mut window =
                opening::native_window(cc, with_what, requested, persistence, measuring)?;
            window.restoration = restoration;
            window.board.plans = board::plans::Store::at(
                session::path().and_then(|p| p.parent().map(|p| p.join("plans"))),
                !measuring,
                cc.egui_ctx.clone(),
            );
            // Only a measurement batch asks for them, so they are set here and not in the
            // constructor. The cell gets measured later on: the panel gesture counts its travel in
            // cells and its hook runs before the pass that measures the font.
            window.injected_drags = InjectedDrags::new(injected_drags, gesture_at_startup());
            // **The theme is set here and not in the constructor**, just like the drags above and
            // the restoring span: taking it right inside would force passing it through three
            // constructors with thirty-three calls among tests, and none of them configures
            // anything.
            settings.apply_to(&mut window);
            if integrated {
                window.chrome.start(&cc.egui_ctx, !measuring);
                if std::env::var("MEASURE_CHROME").ok().as_deref() == Some("zoom") {
                    let tab = window.active().name;
                    let focus = window.active().focus;
                    window.pane_view.toggle(tab, focus);
                    window.chrome.set_zoom_probe(true);
                }
            }
            // **The strip is switched off by the same gate that vetoes the settings file**, and a
            // batch opts back into it with a knob of its own: a header takes a row from every
            // console, so a measured run that painted one would publish a grid no published batch
            // was measured with. See [`header::Headers::resolved`].
            window.headers = header::Headers::resolved(entry, wants_headers).integrated(integrated);
            project_view::configure(&mut window, entry, project_probe.clone());
            window.settings_editor = settings::editor::Editor::resolved(
                entry,
                std::env::var("MEASURE_SETTINGS").ok().as_deref(),
            );
            window.settings_editor.remember_family(&settings);
            board::probe::configure(
                &mut window,
                measuring,
                std::env::var("MEASURE_BOARD").ok().as_deref(),
            );
            // **The font goes apart from the rest of the settings, and it is not tidiness.** The
            // others put a value on the window; this one talks to the text engine and **costs
            // between 17.3 and 23.2 ms** (**D24**) because resolving a name forces enumerating the
            // machine. It needs the context, which only exists here.
            ligatures::configure(&mut window, &settings, &cc.egui_ctx, what_it_cost);
            Ok(Box::new(window))
        }),
    )
}
