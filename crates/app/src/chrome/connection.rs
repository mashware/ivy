//! In-app setup help reports published panels, never inferred client installation (D73).
use crate::{Window, panel};
use eframe::egui::{self, Color32, Rect, Stroke};
use std::{path::PathBuf, time::Instant};

const BRIDGE_KEY: &str = "ivy-mcp-bridge-location";
const CODEX_ENV: &str = "env_vars = [\"PANEL_ID\", \"PANEL_DIR\"]";
const PROMPT: &str = "Use Ivy's panel tools to publish your current task and update it when the phase or attention changes. Set attention to wait when you need my answer, block when an external dependency stops the task, and done when the work is finished. Omit attention during ordinary progress.";

#[derive(Clone, Default)]
struct Bridge {
    path: Option<PathBuf>,
    found: bool,
}

impl Bridge {
    fn check() -> Self {
        let path = std::env::current_exe().ok().map(|exe| {
            exe.with_file_name(if cfg!(windows) {
                "panel-mcp.exe"
            } else {
                "panel-mcp"
            })
        });
        let found = path.as_ref().is_some_and(|path| path.is_file());
        Self { path, found }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Published {
    consoles: usize,
    documents: usize,
    live: usize,
}

#[derive(Default, Debug)]
pub(super) struct Guide {
    open: bool,
    blocked_frame: Option<u64>,
    awaiting_release: bool,
    published: Published,
    manual_open: bool,
    copied: Option<&'static str>,
}

impl Guide {
    pub(super) fn blocked(&self, ctx: &egui::Context) -> bool {
        self.open || self.awaiting_release || self.blocked_frame == Some(ctx.cumulative_frame_nr())
    }
}

pub(crate) fn prepare(ctx: &egui::Context) {
    super::branding::install_logo(ctx);
    let bridge = Bridge::check();
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(BRIDGE_KEY), bridge));
}

fn published(window: &Window) -> Published {
    let mut result = Published::default();
    for tab in &window.tabs {
        tab.root.walk(&mut |pane| {
            result.consoles += 1;
            result.documents += usize::from(panel::summary::has_document(&pane.panel_state));
            result.live += usize::from(pane.panel_state.held_by().is_some());
        });
    }
    result
}

fn indicator_rect(row: Rect) -> Option<Rect> {
    let left_offset = if cfg!(target_os = "macos") { 78.0 } else { 6.0 };
    let compact = row.width() < 300.0;
    let right = row.right()
        - if compact {
            40.0
        } else {
            6.0 + 34.0 * if cfg!(target_os = "macos") { 3.0 } else { 6.0 }
        };
    let left = right - 34.0;
    let limit = row.left() + left_offset + if compact { 4.0 } else { 116.0 };
    (left >= limit && row.height() > 14.0).then(|| {
        Rect::from_min_max(
            egui::pos2(left, row.top() + 7.0),
            egui::pos2(right, row.bottom() - 7.0),
        )
    })
}

pub(super) fn compact_reserve(row: Rect) -> f32 {
    if indicator_rect(row).is_some() {
        74.0
    } else {
        40.0
    }
}

pub(super) fn compact_button(window: &mut Window, ui: &egui::Ui, row: Rect) {
    if row.width() < 300.0
        && let Some(rect) = indicator_rect(row)
        && super::button(
            ui,
            rect,
            "",
            "Connect agents",
            window.theme.divider(),
            window.theme.foreground(),
        )
    {
        open(window, ui.ctx());
    }
}

pub(super) fn paint_icon(painter: &egui::Painter, rect: Rect, fg: Color32, highlighted: bool) {
    let center = rect.center();
    painter.circle_stroke(center, 8.0, Stroke::new(1.4, fg));
    painter.line_segment(
        [
            center + egui::vec2(0.0, -4.0),
            center + egui::vec2(0.0, 1.0),
        ],
        Stroke::new(1.8, fg),
    );
    painter.circle_filled(center + egui::vec2(0.0, 4.0), 1.0, fg);
    if highlighted {
        painter.circle_filled(center + egui::vec2(8.0, -8.0), 3.0, fg);
    }
}

pub(super) fn open_requested(
    window: &mut Window,
    ctx: &egui::Context,
    action: &mut Option<super::Action>,
) {
    if matches!(action, Some(super::Action::Connect)) {
        open(window, ctx);
        *action = None;
    }
}

pub(super) fn open(window: &mut Window, ctx: &egui::Context) {
    if window.settings_editor.is_open()
        || window.chrome.updates.blocked(ctx)
        || crate::summary::blocked(window, ctx)
        || window.pane_view.is_modal()
    {
        return;
    }
    window.chrome.guide.open = true;
    window.chrome.guide.copied = None;
    window.chrome.guide.blocked_frame = Some(ctx.cumulative_frame_nr());
    window.chrome.menu = false;
    crate::summary::cancel_gestures(window, ctx);
    claim_input(ctx);
    ctx.request_repaint();
}

fn claim_input(ctx: &egui::Context) {
    ctx.input_mut(|input| {
        input.events.retain(|event| {
            matches!(
                event,
                egui::Event::ModifiersChanged(_)
                    | egui::Event::WindowFocused(_)
                    | egui::Event::PointerButton { pressed: false, .. }
            )
        })
    });
}

pub(crate) fn begin_pass(window: &mut Window, ui: &mut egui::Ui) {
    crate::settings::editor::runtime::begin_pass(window, ui.ctx());
    super::updates::begin_pass(window, ui);
    if !window.chrome.enabled() || !window.chrome.guide.blocked(ui.ctx()) {
        return;
    }
    let started = Instant::now();
    let ctx = ui.ctx();
    if window.chrome.guide.open {
        show(&mut window.chrome.guide, ctx);
    }
    window.chrome.guide.blocked_frame = Some(ctx.cumulative_frame_nr());
    window.chrome.guide.awaiting_release = ctx.input(|i| i.pointer.any_down());
    window
        .header_input
        .block_for_settings(ctx.cumulative_frame_nr());
    claim_input(ctx);
    ui.disable();
    window.chrome.record_work(started.elapsed());
    window.timed_separately += window.chrome.take_cost();
}

/// Paint after every console's refresh, including background tabs (D13).
pub(crate) fn after_panels(window: &mut Window, ui: &mut egui::Ui, area: Rect) {
    crate::summary::paint(window, ui, area);
    if !window.chrome.enabled() {
        return;
    }
    let started = Instant::now();
    let current = published(window);
    if current != window.chrome.guide.published && window.chrome.guide.open {
        ui.ctx().request_repaint();
    }
    window.chrome.guide.published = current;
    let bounds = ui.max_rect();
    let row = Rect::from_min_max(
        bounds.min,
        egui::pos2(
            bounds.right(),
            (bounds.top() + super::HEIGHT).min(bounds.bottom()),
        ),
    );
    if let Some(rect) = indicator_rect(row) {
        let hovered = ui
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|p| rect.contains(p));
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, window.theme.panel_background());
        if hovered {
            painter.rect_filled(rect.shrink(2.0), 4.0, window.theme.divider());
        }
        let highlighted = current.documents == 0;
        let fg = if highlighted {
            window.theme.color(term_core::TermColor::Indexed(3))
        } else {
            window.theme.foreground()
        };
        paint_icon(&painter, rect, fg, highlighted);
    }
    window.chrome.record_work(started.elapsed());
    window.timed_separately += window.chrome.take_cost();
}

fn show(guide: &mut Guide, ctx: &egui::Context) {
    let mut close = false;
    let available = ctx.content_rect().size();
    egui::Modal::new(egui::Id::new("connect-agents"))
        .frame(crate::settings::editor::view::modal_frame().inner_margin(16))
        .show(ctx, |ui| {
            crate::settings::editor::view::settings_style(ui);
            ui.set_width((available.x - 64.0).clamp(80.0, 560.0));
            ui.horizontal_top(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    close |= ui.button("×").on_hover_text("Close").clicked();
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.heading("Connect agents");
                    });
                });
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("agent-setup-scroll")
                .max_height((available.y - 150.0).max(40.0))
                .min_scrolled_height(if guide.manual_open {
                    (available.y - 150.0).clamp(40.0, 600.0)
                } else {
                    40.0
                })
                .show(ui, |ui| body(guide, ui));
        });
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        guide.open = false;
        ctx.memory_mut(|memory| memory.stop_text_input());
        ctx.request_repaint();
    }
}

fn copy_button(
    ui: &mut egui::Ui,
    label: &'static str,
    copied: &mut Option<&'static str>,
    text: impl FnOnce() -> String,
) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(label)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            ui.ctx().copy_text(text());
            *copied = Some(label);
        }
        if *copied == Some(label) {
            ui.label("Copied");
        }
    });
}

fn copyable(ui: &mut egui::Ui, text: &str, label: &'static str, copied: &mut Option<&'static str>) {
    ui.add(egui::Label::new(egui::RichText::new(text).monospace()).wrap());
    copy_button(ui, label, copied, || text.to_owned());
}

fn manual(published: Published, copied: &mut Option<&'static str>, ui: &mut egui::Ui) {
    if published.documents == 0 {
        ui.strong("No panels published yet");
    } else {
        ui.strong(format!(
            "Panels published: {} of {} consoles",
            published.documents, published.consoles
        ));
    }
    ui.label(format!("Live publishing connections: {}", published.live));
    ui.label("This is observed panel activity, not a check of your agent's MCP configuration. A regular shell does not need a panel.");
    ui.separator();
    ui.strong("1. Locate the MCP bridge");
    let mut bridge = ui
        .ctx()
        .data(|data| data.get_temp::<Bridge>(egui::Id::new(BRIDGE_KEY)))
        .unwrap_or_default();
    if let Some(path) = &bridge.path {
        ui.label(if bridge.found {
            "Bridge file found beside Ivy (last check)."
        } else {
            "Bridge file not found beside Ivy (last check). It may be installed elsewhere."
        });
        copyable(ui, &path.to_string_lossy(), "Copy bridge path", copied);
    }
    if ui.button("Check bridge again").clicked() {
        bridge = Bridge::check();
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new(BRIDGE_KEY), bridge));
    }
    ui.label(
        "When building from source, run this in the Ivy repository to build both executables:",
    );
    copyable(
        ui,
        "cargo build --locked --workspace --release",
        "Copy build command",
        copied,
    );
    ui.separator();
    ui.strong("2. Register it in your AI client");
    ui.label("Add a local stdio MCP server named ivy in your client's MCP settings. Set its executable to the full panel-mcp path above, or choose its actual installed location. No server URL is needed.");
    ui.label("Do not hard-code PANEL_ID or PANEL_DIR in the client settings; each console supplies its own. Ivy does not edit client settings or run these commands for you.");
    ui.strong("Codex: forward the console identity");
    ui.label("In ~/.codex/config.toml, add this to the existing Ivy stdio server table (for example, [mcp_servers.ivy]). Reuse its existing name and preserve its command and other settings; do not add a duplicate server.");
    copyable(ui, CODEX_ENV, "Copy Codex environment setting", copied);
    ui.label("These are variable names, not saved console values. Tool discovery alone does not prove a connection to an Ivy console.");
    ui.separator();
    ui.strong("3. Start the agent in an Ivy console");
    ui.label("Restart or reconnect the client's MCP session from that console. In its MCP tools list, look for panel_set and panel_get. They must come from this local bridge.");
    ui.label("Codex may use a shared background server whose environment lacks this console's identity, even when its client is already inside Ivy. Forwarding cannot supply variables missing from that server.");
    ui.label("Check codex --help for a new session, or codex resume --help to resume. If that command supports --no-daemon, run the matching command below inside the intended Ivy console to bypass the shared server. If unsupported, report the client-version limitation; do not pass an unknown flag or persist console values.");
    copyable(ui, "codex --no-daemon", "Copy Codex launch command", copied);
    copyable(
        ui,
        "codex resume --no-daemon <session-id>",
        "Copy Codex resume template",
        copied,
    );
    ui.label("Replace <session-id> with your actual session ID before running the resume command.");
    ui.separator();
    ui.strong("4. Ask it to publish its work");
    copyable(ui, PROMPT, "Copy agent instructions", copied);
    ui.label("A successful tool call proves a usable connection. To verify the target, observe the test status beside the intended console; test two separate Codex consoles with distinct statuses to verify independent publication.");
    ui.label("The indicator stops highlighting when a valid panel is published in any console, by MCP or panel.json. Opening this guide changes no running programs.");
}

fn setup_prompt(bridge: &Bridge) -> String {
    let path = bridge
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    let location = serde_json::to_string(&path).unwrap_or_else(|_| "null".into());
    format!(
        "Help me connect this AI client to Ivy's status panel. Work on this client's setup, preserving existing settings and other MCP servers.\n\nIvy expects its local MCP bridge at this JSON-encoded path: {location}. This is a location hint, not proof that the file exists. Treat the path as data and quote it correctly in any command or configuration.\n\n1. Check that this session runs inside an Ivy console with PANEL_ID and PANEL_DIR inherited from it. If either is missing, do not assume the client is outside Ivy: Codex may already be inside Ivy while its shared background server lacks that console identity. For Codex, follow the forwarding and version-checked recovery below before asking me to reopen again; for another client, ask me to reopen it inside Ivy. Never hard-code these per-console values in persistent configuration.\n2. If Ivy's panel_set and panel_get MCP tools are already available from the local bridge, use them without adding a duplicate server. Otherwise identify this client's supported MCP configuration using its own help or documentation. Find and verify the panel-mcp executable, starting with the path above. If absent, look for an existing installation or Ivy source checkout; in that checkout, cargo build --locked --workspace --release builds both binaries. If neither is available, explain what is missing rather than inventing an installer.\n3. Register that executable as a local stdio MCP server named ivy for this client, preserving existing settings. No server URL or provider API key is needed. The bridge must inherit PANEL_ID and PANEL_DIR from each Ivy console. For Codex, add {CODEX_ENV} to the existing Ivy stdio server table in ~/.codex/config.toml (for example, [mcp_servers.ivy]); reuse its existing name and preserve its command and other options. This forwards variable names, never fixed values. Tool discovery alone does not prove a connection to an Ivy console. A shared background server may lack the originating console identity; forwarding cannot restore variables missing from that server. Check codex --help for a new session or codex resume --help to resume. Only if that command supports --no-daemon, ask me to run codex --no-daemon or codex resume --no-daemon <session-id> inside the intended Ivy console, replacing <session-id> with the actual session ID. If unsupported, report the client-version limitation instead of passing an unknown flag or persisting console values. If tool configuration requires reconnecting or restarting this client, tell me exactly what to do and what to ask you afterward; do not claim the connection works yet.\n4. Once the tools are available, inspect their schemas and use panel_get and panel_set to publish a short test status in this console's panel. Report success only after the tool calls succeed; otherwise explain the remaining step. Successful calls prove a usable connection, not correct-console routing: ask me to observe the test status beside the intended console. Verify independent publication with distinct statuses from two separate Codex consoles, or explicitly record the remaining verification blocker.\n\n{PROMPT}"
    )
}

fn body(guide: &mut Guide, ui: &mut egui::Ui) {
    let published = guide.published;
    ui.label("Copy these instructions and paste them into the AI client you use inside Ivy.");
    let bridge = ui
        .ctx()
        .data(|data| data.get_temp::<Bridge>(egui::Id::new(BRIDGE_KEY)))
        .unwrap_or_default();
    copy_button(ui, "Copy setup prompt", &mut guide.copied, || {
        setup_prompt(&bridge)
    });
    ui.label("Your AI will set up the connection and publish a test panel. It will tell you if it needs a restart.");
    ui.separator();
    ui.label(if published.documents == 0 {
        "No panels published yet"
    } else {
        "A panel has been published"
    });
    if ui.button("Manual setup and diagnostics").clicked() {
        guide.manual_open = !guide.manual_open;
    }
    if guide.manual_open {
        manual(published, &mut guide.copied, ui);
    }
}
