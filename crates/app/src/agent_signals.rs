//! Declared client metadata; it never writes the producer-owned panel.
use panel_control::{Activity, AgentSnapshot};
pub(crate) mod logos;
use std::time::{Duration, Instant, SystemTime};
use term_core::{ProgramClient, ProgramState, Screen};

pub(crate) const STALE_AFTER: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Client {
    Claude,
    Codex,
    OpenCode,
}
impl Client {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::OpenCode => "OpenCode",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Enabled {
    pub claude: bool,
    pub codex: bool,
    pub opencode: bool,
}
impl Enabled {
    fn allows(self, client: Client) -> bool {
        match client {
            Client::Claude => self.claude,
            Client::Codex => self.codex,
            Client::OpenCode => self.opencode,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct View {
    pub client: Client,
    pub busy: bool,
    pub waiting: bool,
    pub backed: bool,
}
struct Declaration {
    view: View,
    at: Instant,
}
struct Reporter {
    connection: u64,
    connected: bool,
    declaration: Declaration,
}
#[derive(Default)]
pub(crate) struct Signals {
    enabled: Enabled,
    generation: u64,
    capability_pending: bool,
    native: Option<Declaration>,
    reporter: Option<Reporter>,
}
impl Signals {
    pub(crate) fn configure(&mut self, enabled: Enabled, screen: &mut Screen) {
        self.enabled = enabled;
        if !enabled.claude {
            self.capability_pending = false;
        }
        screen.set_program_clients(enabled.claude, enabled.codex);
        if self
            .native
            .as_ref()
            .is_some_and(|s| !enabled.allows(s.view.client))
        {
            self.native = None;
        }
        if !enabled.opencode {
            self.reporter = None;
        }
    }
    pub(crate) fn sync_native(&mut self, screen: &mut Screen, now: Instant) -> bool {
        let ipc_live = self.reporter.as_ref().is_some_and(|r| r.connected);
        let reply = screen.take_program_query() && self.enabled.claude && !ipc_live;
        let generation = screen.program_generation();
        if generation != self.generation {
            self.generation = generation;
            if screen.program_snapshot().is_none() {
                self.capability_pending = false;
            }
            if !ipc_live {
                self.native = screen.program_snapshot().and_then(|snapshot| {
                    let client = match snapshot.client {
                        ProgramClient::Claude => Client::Claude,
                        ProgramClient::Codex => Client::Codex,
                    };
                    self.enabled.allows(client).then_some(Declaration {
                        view: View {
                            client,
                            busy: snapshot.state == ProgramState::Working,
                            waiting: snapshot.state == ProgramState::Blocked
                                && snapshot.kind.is_some(),
                            backed: true,
                        },
                        at: now,
                    })
                });
                if self.native.is_some() {
                    self.reporter = None;
                }
            }
        }
        reply
    }
    pub(crate) fn take_capability_reply(&mut self, queried: bool, writable: bool) -> bool {
        if !self.enabled.claude || self.reporter.as_ref().is_some_and(|r| r.connected) {
            self.capability_pending = false;
            return false;
        }
        self.capability_pending |= queried;
        writable && std::mem::take(&mut self.capability_pending)
    }
    pub(crate) fn receive(
        &mut self,
        connection: u64,
        snapshot: Option<AgentSnapshot>,
        now: Instant,
    ) -> Result<(), String> {
        if !self.enabled.opencode {
            return Err("OpenCode integration is disabled; enable it in Connect agents.".into());
        }
        if self
            .reporter
            .as_ref()
            .is_some_and(|r| r.connected && r.connection != connection)
        {
            return Err("Another TUI connection is reporting this console's agent signals.".into());
        }
        match snapshot {
            Some(snapshot) => {
                if snapshot.stale {
                    let Some(reporter) = &mut self.reporter else {
                        return Err(
                            "A complete current snapshot is required before reporting stale state."
                                .into(),
                        );
                    };
                    if reporter.connection != connection {
                        return Err("A complete current snapshot is required to replace the previous reporter.".into());
                    }
                    reporter.declaration.view.backed = false;
                    return Ok(());
                }
                self.native = None;
                self.reporter = Some(Reporter {
                    connection,
                    connected: true,
                    declaration: Declaration {
                        view: View {
                            client: Client::OpenCode,
                            busy: snapshot.activity == Activity::Busy,
                            waiting: snapshot.permission || snapshot.question,
                            backed: true,
                        },
                        at: now,
                    },
                });
            }
            None if self
                .reporter
                .as_ref()
                .is_some_and(|r| r.connection == connection) =>
            {
                self.reporter = None;
            }
            None => {}
        }
        Ok(())
    }
    pub(crate) fn closed(&mut self, connection: u64, _now: Instant) {
        if let Some(reporter) = &mut self.reporter
            && reporter.connection == connection
        {
            reporter.connected = false;
        }
    }
    pub(crate) fn view(&self, now: Instant) -> Option<View> {
        let (declaration, connected) = self.reporter.as_ref().map_or_else(
            || (self.native.as_ref(), true),
            |r| (Some(&r.declaration), r.connected),
        );
        declaration.map(|s| {
            let backed =
                connected && s.view.backed && now.saturating_duration_since(s.at) < STALE_AFTER;
            View {
                busy: backed && s.view.busy,
                backed,
                ..s.view
            }
        })
    }
    pub(crate) fn stale_in(&self, now: Instant) -> Option<Duration> {
        let declaration = self.reporter.as_ref().map_or_else(
            || self.native.as_ref(),
            |r| r.connected.then_some(&r.declaration),
        )?;
        if !declaration.view.backed {
            return None;
        }
        STALE_AFTER
            .checked_sub(now.saturating_duration_since(declaration.at))
            .filter(|d| !d.is_zero())
    }
    pub(crate) fn attention(&self, now: Instant) -> Option<crate::panel::AttentionState> {
        let view = self.view(now)?;
        view.waiting.then_some(crate::panel::AttentionState {
            class: panel_contract::Attention::Wait,
            backed: view.backed,
        })
    }
}
impl crate::Pane {
    pub(crate) fn effective_attention(
        &self,
        now: SystemTime,
    ) -> Option<crate::panel::AttentionState> {
        self.panel_state
            .attention(now)
            .max(self.signals.attention(Instant::now()))
    }
}
