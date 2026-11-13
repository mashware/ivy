//! Agent metadata reaches pane/tab attention without acquiring producer authority.
use super::*;
use crate::agent_signals::{Client, Enabled};
use panel_contract::Attention;

mod native_transport;

fn enabled() -> Enabled {
    Enabled {
        claude: true,
        codex: true,
        opencode: true,
    }
}

fn native(pane: &mut Pane, payload: &str, now: Instant) {
    pane.screen
        .advance(format!("\x1b]7501;{payload}\x1b\\").as_bytes());
    pane.signals.sync_native(&mut pane.screen, now);
}

fn configure(pane: &mut Pane) {
    pane.signals.configure(enabled(), &mut pane.screen);
}

fn publish_flow(pane: &mut Pane, attention: &str) -> String {
    let document = format!(
        r#"{{"attention":"{attention}","phase":"build","lines":["Flow task"],"actions":[{{"label":"Inspect","send":"git status"}}]}}"#
    );
    assert_eq!(
        panel_contract::parse_document(&document)
            .unwrap()
            .actions
            .len(),
        1,
        "the authority witness needs a real producer action"
    );
    pane.panel_state
        .deliver(
            71,
            "flow",
            panel_contract::parse_document(&document).unwrap(),
            SystemTime::now(),
        )
        .unwrap();
    pane.panel_state.document().unwrap()
}

fn still_flow_owned(pane: &mut Pane, document: &str) {
    assert_eq!(
        pane.panel_state.document().unwrap(),
        document,
        "agent metadata rewrote the producer document or actions"
    );
    let competing = panel_contract::parse_document(r#"{"lines":["intruder"]}"#).unwrap();
    assert!(
        pane.panel_state
            .deliver(72, "other", competing, SystemTime::now())
            .is_err(),
        "agent metadata released Flow's ownership"
    );
}

fn snapshot(permission: bool, question: bool) -> panel_control::AgentSnapshot {
    panel_control::AgentSnapshot {
        stale: false,
        session: "ses_root".into(),
        activity: panel_control::Activity::Busy,
        permission,
        question,
    }
}

fn delivered(pane: &mut Pane, connection: u64, what: panel_control::What) -> panel_control::Answer {
    let (answer, reply) = std::sync::mpsc::sync_channel(1);
    pane.control
        .as_ref()
        .unwrap()
        .enqueue_for_tests(panel_control::Delivery {
            origin: panel_control::PublicationOrigin::Unknown,
            connection,
            what,
            answer,
        });
    pane.refresh_its_panel(&egui::Context::default());
    reply
        .recv_timeout(Duration::from_secs(2))
        .expect("the pane must answer the signal request")
}

#[test]
fn native_waits_reach_the_background_tab_without_changing_flow() {
    let mut window = window_of_tabs([["sleep 8", "sleep 8"]; 2]);
    let now = Instant::now();
    let document = {
        let pane = window.tabs[1].panes_mut().into_iter().next().unwrap();
        configure(pane);
        publish_flow(pane, "block")
    };
    for payload in [
        "state=working:app=codex",
        "state=blocked:app=codex:kind=permission",
        "state=blocked:app=codex:kind=question",
        "state=idle:app=codex",
        "state=clear",
    ] {
        let expected = if payload.contains("blocked") {
            Attention::Wait
        } else {
            Attention::Block
        };
        let pane = window.tabs[1].panes_mut().into_iter().next().unwrap();
        native(pane, payload, now);
        assert_eq!(
            pane.effective_attention(SystemTime::now()).unwrap().class,
            expected
        );
        still_flow_owned(pane, &document);
        assert_eq!(
            window.tabs[1].attention(SystemTime::now()).unwrap().class,
            expected,
            "background tab lost its console's input requirement"
        );
        assert_eq!(window.active, 0);
        assert!(
            window.tabs[0].attention(SystemTime::now()).is_none(),
            "signal escaped its pane"
        );
    }
}

#[test]
fn native_resolution_and_disable_preserve_an_existing_panel_wait() {
    let mut pane = pane_with("sleep 8", 0);
    configure(&mut pane);
    let document = publish_flow(&mut pane, "wait");
    let now = Instant::now();
    for payload in [
        "state=blocked:app=claude-code:kind=auth",
        "state=done:app=claude-code",
        "state=error:app=claude-code",
        "state=clear",
    ] {
        native(&mut pane, payload, now);
        assert_eq!(
            pane.effective_attention(SystemTime::now()).unwrap().class,
            Attention::Wait
        );
        still_flow_owned(&mut pane, &document);
    }
    native(&mut pane, "state=blocked:app=codex:kind=question", now);
    pane.signals.configure(Enabled::default(), &mut pane.screen);
    assert!(pane.signals.view(now).is_none());
    assert_eq!(
        pane.effective_attention(SystemTime::now()).unwrap().class,
        Attention::Wait
    );
    still_flow_owned(&mut pane, &document);
}

#[test]
fn disabled_malformed_unknown_and_child_signals_cannot_replace_a_root_wait() {
    let mut pane = pane_with("sleep 8", 0);
    let now = Instant::now();
    native(&mut pane, "state=blocked:app=codex:kind=question", now);
    assert!(pane.effective_attention(SystemTime::now()).is_none());
    configure(&mut pane);
    native(
        &mut pane,
        "state=blocked:app=claude-code:kind=permission",
        now,
    );
    for payload in [
        "state=idle:app=claude-code:id=child",
        "state=clear:id=child",
        "state=working:app=unknown",
        "state=blocked:app=codex:kind=made-up",
        "state=working:app=codex:state=idle",
    ] {
        native(&mut pane, payload, now);
        let view = pane.signals.view(now).unwrap();
        assert_eq!(
            view.client,
            Client::Claude,
            "invalid or child frame stole root identity: {payload}"
        );
        assert!(
            view.waiting,
            "invalid or child frame resolved the root wait: {payload}"
        );
    }
}

#[test]
fn opencode_socket_takeover_is_ordered_and_does_not_acquire_flow_authority() {
    let mut window = window_with("sleep 8", "sleep 8");
    attach_a_channel(&mut window, 0);
    let pane = window.active_mut().panes_mut().into_iter().next().unwrap();
    configure(pane);
    let document = publish_flow(pane, "block");
    let now = Instant::now();
    native(pane, "state=blocked:app=codex:kind=question", now);
    assert!(matches!(
        delivered(pane, 81, panel_control::What::AgentSignal(None)),
        panel_control::Answer::Taken
    ));
    assert_eq!(
        pane.signals.view(now).unwrap().client,
        Client::Codex,
        "a connection without a route snapshot took presentation authority"
    );
    assert!(matches!(
        delivered(
            pane,
            81,
            panel_control::What::AgentSignal(Some(snapshot(true, false)))
        ),
        panel_control::Answer::Taken
    ));
    assert_eq!(
        pane.signals.view(Instant::now()).unwrap().client,
        Client::OpenCode
    );
    assert!(
        matches!(
            delivered(
                pane,
                82,
                panel_control::What::AgentSignal(Some(snapshot(false, false)))
            ),
            panel_control::Answer::Refused(_)
        ),
        "a competing socket replaced the live owner"
    );
    assert_eq!(
        pane.effective_attention(SystemTime::now()).unwrap().class,
        Attention::Wait
    );
    still_flow_owned(pane, &document);
    assert!(matches!(
        delivered(
            pane,
            81,
            panel_control::What::AgentSignal(Some(snapshot(false, false)))
        ),
        panel_control::Answer::Taken
    ));
    assert_eq!(
        pane.effective_attention(SystemTime::now()).unwrap().class,
        Attention::Block
    );
    native(
        pane,
        "state=blocked:app=claude-code:kind=question",
        Instant::now(),
    );
    assert!(matches!(
        delivered(pane, 81, panel_control::What::AgentSignal(None)),
        panel_control::Answer::Taken
    ));
    assert!(
        pane.signals.view(Instant::now()).is_none(),
        "withdrawal resurrected a suppressed native declaration"
    );
    still_flow_owned(pane, &document);
    native(
        pane,
        "state=blocked:app=claude-code:kind=question",
        Instant::now(),
    );
    assert_eq!(
        pane.signals.view(Instant::now()).unwrap().client,
        Client::Claude
    );
}

#[test]
fn unexpected_opencode_close_retains_unconfirmed_wait_until_complete_replacement() {
    let mut window = window_with("sleep 8", "sleep 8");
    attach_a_channel(&mut window, 0);
    let pane = window.active_mut().panes_mut().into_iter().next().unwrap();
    configure(pane);
    let document = publish_flow(pane, "block");
    assert!(matches!(
        delivered(
            pane,
            81,
            panel_control::What::AgentSignal(Some(snapshot(false, true)))
        ),
        panel_control::Answer::Taken
    ));
    let (answer, _reply) = std::sync::mpsc::sync_channel(1);
    pane.control
        .as_ref()
        .unwrap()
        .enqueue_for_tests(panel_control::Delivery {
            origin: panel_control::PublicationOrigin::Unknown,
            connection: 81,
            what: panel_control::What::Closed,
            answer,
        });
    pane.refresh_its_panel(&egui::Context::default());
    let view = pane.signals.view(Instant::now()).unwrap();
    assert!(view.waiting);
    assert!(
        !view.backed,
        "socket loss was reported as a confirmed current wait"
    );
    assert!(!view.busy, "disconnected metadata still animates");
    let attention = pane.effective_attention(SystemTime::now()).unwrap();
    assert_eq!(attention.class, Attention::Wait);
    assert!(!attention.backed, "socket loss was treated as an answer");
    delivered(
        pane,
        82,
        panel_control::What::AgentSignal(Some(snapshot(false, false))),
    );
    assert_eq!(
        pane.effective_attention(SystemTime::now()).unwrap().class,
        Attention::Block
    );
    still_flow_owned(pane, &document);
}

#[test]
fn native_reset_and_silence_have_different_attention_effects() {
    let mut pane = pane_with("sleep 8", 0);
    configure(&mut pane);
    let now = Instant::now();
    native(&mut pane, "state=blocked:app=codex:kind=question", now);
    let stale = now + Duration::from_secs(30 * 60 + 1);
    let attention = pane.signals.attention(stale).unwrap();
    assert_eq!(attention.class, Attention::Wait);
    assert!(
        !attention.backed,
        "silence must retain an unconfirmed request"
    );
    pane.screen.advance(b"\x1bc");
    pane.signals.sync_native(&mut pane.screen, stale);
    assert!(
        pane.signals.view(stale).is_none(),
        "RIS failed to withdraw terminal metadata"
    );
}

#[test]
fn claude_negotiation_is_explicit_and_cannot_rewrite_titles_or_panel_documents() {
    let mut pane = pane_with("sleep 8", 0);
    let now = Instant::now();
    pane.screen.advance(b"\x1b]7501;?\x07");
    assert!(
        !pane.signals.sync_native(&mut pane.screen, now),
        "disabled Claude answered capability negotiation"
    );
    configure(&mut pane);
    let document = publish_flow(&mut pane, "wait");
    pane.screen
        .advance(b"\x1b]0;Keep my title\x07\x1b]7501;?\x07\x1b]7501;?\x1b\\");
    assert!(
        pane.signals.sync_native(&mut pane.screen, now),
        "enabled Claude probe was not routed to the PTY reply path"
    );
    assert!(
        !pane.signals.sync_native(&mut pane.screen, now),
        "one parsed batch replayed capability replies"
    );
    native(&mut pane, "state=working:app=claude-code", now);
    assert_eq!(pane.screen.terminal_title(), Some("Keep my title"));
    still_flow_owned(&mut pane, &document);
}

#[test]
fn live_settings_and_window_pump_apply_and_withdraw_hidden_pane_signals() {
    let mut window = window_of_tabs([["sleep 8", "sleep 8"]; 2]);
    let (preferences, warnings) = settings::interpret("agent.codex.enabled=true").unwrap();
    assert!(warnings.kept().is_empty());
    preferences.apply_to(&mut window);
    window.pump_them_all();
    window.tabs[1].panes_mut()[1]
        .screen
        .advance(b"\x1b]7501;state=blocked:app=codex:kind=question\x1b\\");
    window.pump_them_all();
    assert_eq!(
        window.tabs[1].attention(SystemTime::now()).unwrap().class,
        Attention::Wait,
        "the app pump failed to collect native metadata from a hidden pane"
    );
    assert!(window.tabs[0].attention(SystemTime::now()).is_none());
    let (preferences, warnings) = settings::interpret("agent.codex.enabled=false").unwrap();
    assert!(warnings.kept().is_empty());
    preferences.apply_to(&mut window);
    window.pump_them_all();
    assert!(
        window.tabs[1].attention(SystemTime::now()).is_none(),
        "live disable left background attention behind"
    );
}

#[test]
fn actual_headers_paint_only_declared_logos_preserve_manual_names_and_gate_animation() {
    use crate::agent_signals::{View, logos};
    let ctx = egui::Context::default();
    chrome::prepare(&ctx);
    logos::install(&ctx);
    let mut window = window_with("sleep 8", "sleep 8");
    window.chrome.enable();
    window.chrome.agent_preferences = enabled();
    window.headers = window.headers.integrated(true);
    window.pump_them_all();
    window.active_mut().panes_mut()[0].label = Some("My review console".into());
    window.active_mut().panes_mut()[0]
        .screen
        .advance(b"\x1b]0;Arbitrary \xe2\xa0\xb8 title\x07");
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1000.0));
    let textures: Vec<_> = [Client::Claude, Client::Codex, Client::OpenCode]
        .into_iter()
        .map(|client| {
            ctx.data(|data| {
                data.get_temp::<egui::TextureHandle>(egui::Id::new(("agent-logo", client as u8)))
                    .unwrap()
                    .id()
            })
        })
        .collect();
    let painted = |window: &mut Window| {
        let shapes = ui_pass(window, &ctx, area, egui::RawInput::default());
        assert!(
            the_text_queued_up(&shapes, "My review console"),
            "a declared agent replaced the person's console name"
        );
        shapes
            .into_iter()
            .filter_map(|shape| match shape.shape {
                egui::Shape::Mesh(mesh) if textures.contains(&mesh.texture_id) => {
                    Some(mesh.texture_id)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert!(
        painted(&mut window).is_empty(),
        "a title alone was used as agent identity"
    );
    for (index, app) in ["claude-code", "codex"].into_iter().enumerate() {
        native(
            window.active_mut().panes_mut().into_iter().next().unwrap(),
            &format!("state=working:app={app}"),
            Instant::now(),
        );
        assert_eq!(
            painted(&mut window),
            vec![textures[index]],
            "the real pane header omitted or misidentified a native client"
        );
    }
    window.active_mut().panes_mut()[0]
        .signals
        .receive(81, Some(snapshot(false, false)), Instant::now())
        .unwrap();
    assert_eq!(
        painted(&mut window),
        vec![textures[2]],
        "the real pane header omitted the OpenCode reporter"
    );
    window.chrome.agent_preferences = Enabled::default();
    window.pump_them_all();
    assert!(
        painted(&mut window).is_empty(),
        "disabled client identity remained painted"
    );
    let busy = View {
        client: Client::Codex,
        busy: true,
        waiting: false,
        backed: true,
    };
    assert_eq!(
        logos::animation_delay(busy, true, true),
        Some(Duration::from_millis(100))
    );
    for (view, visible, focused) in [
        (busy, false, true),
        (busy, true, false),
        (
            View {
                busy: false,
                ..busy
            },
            true,
            true,
        ),
        (
            View {
                backed: false,
                ..busy
            },
            true,
            true,
        ),
        (
            View {
                waiting: true,
                busy: false,
                ..busy
            },
            true,
            true,
        ),
    ] {
        assert!(
            logos::animation_delay(view, visible, focused).is_none(),
            "hidden, unfocused, idle, waiting or unconfirmed source armed animation"
        );
    }
}

#[test]
fn incomplete_opencode_refresh_cannot_take_over_or_resolve_a_known_wait() {
    let mut window = window_with("sleep 8", "sleep 8");
    attach_a_channel(&mut window, 0);
    let pane = window.active_mut().panes_mut().into_iter().next().unwrap();
    configure(pane);
    let document = publish_flow(pane, "block");
    native(
        pane,
        "state=blocked:app=codex:kind=permission",
        Instant::now(),
    );
    let mut stale = snapshot(false, false);
    stale.stale = true;
    assert!(matches!(
        delivered(
            pane,
            81,
            panel_control::What::AgentSignal(Some(stale.clone()))
        ),
        panel_control::Answer::Refused(_)
    ));
    assert_eq!(
        pane.signals.view(Instant::now()).unwrap().client,
        Client::Codex
    );
    assert!(matches!(
        delivered(
            pane,
            81,
            panel_control::What::AgentSignal(Some(snapshot(false, true)))
        ),
        panel_control::Answer::Taken
    ));
    assert!(matches!(
        delivered(pane, 81, panel_control::What::AgentSignal(Some(stale))),
        panel_control::Answer::Taken
    ));
    let view = pane.signals.view(Instant::now()).unwrap();
    assert!(
        view.waiting,
        "incomplete refresh replaced a known question with false flags"
    );
    assert!(
        !view.backed && !view.busy,
        "incomplete refresh must stop animation and dim the retained wait"
    );
    assert_eq!(
        pane.effective_attention(SystemTime::now()).unwrap().class,
        Attention::Wait
    );
    assert!(!pane.effective_attention(SystemTime::now()).unwrap().backed);
    still_flow_owned(pane, &document);
    assert!(matches!(
        delivered(
            pane,
            81,
            panel_control::What::AgentSignal(Some(snapshot(false, false)))
        ),
        panel_control::Answer::Taken
    ));
    assert_eq!(
        pane.effective_attention(SystemTime::now()).unwrap().class,
        Attention::Block
    );
    still_flow_owned(pane, &document);
}
