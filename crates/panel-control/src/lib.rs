#![deny(rustdoc::broken_intra_doc_links)]
//! **A second transport for the panel document, and not a second contract.**
//!
//! Since August 2026 `panel.json` is *a* transport and not *the* file: the contract is the document
//! and the source of truth is the state the application holds per pane (**D17**). This crate is what
//! makes that sentence true instead of aspirational — a producer that would rather speak than write
//! connects to its pane's endpoint and publishes, and the document it sends goes through the same
//! parser, the same ceiling and the same tolerance as the one on disk.
//!
//! **The file stays the default, and nothing here changes that.** Its barrier to entry is zero: a
//! shell script with `printf` publishes a panel. What this adds is for the producer that already has
//! a process running inside the pane and would rather not own a file — an agent, mainly — and for
//! whom the file's one real weakness bites: **the file does not know who writes it**, so two
//! producers are settled by the operating system's scheduling. A connection does know, which is what
//! lets a source *hold* a pane and be the only writer while it does.
//!
//! # What it deliberately does not carry
//!
//! **No pane identifier, on any message.** The pane a call acts on is the endpoint the connection
//! opened. `PANEL_ID` is in every descendant's environment and any sibling of this user reads it out
//! of `/proc/<pid>/environ`, so it is a name and not a credential; a field carrying it would be a
//! field somebody starts trusting, and the check that validates it is what a later refactor drops.
//!
//! OpenCode may also send a strict `agent_signal` v1 snapshot on its pane endpoint.
//! This independent connection does not claim the panel and is not an MCP tool; its
//! connection identity and disconnect are delivered to the application for arbitration.
//!
//! # What this build serves
//!
//! The six verbs of the frozen wire format: `panel_set`, `panel_get`, `panel_patch`, and the three
//! `split_*` that let a connection **open a console of its own, read it and close it**. The three
//! panel verbs need nobody else — their work stays with the pane — and the three split verbs travel
//! to the window, which is the only party with a tree: a console is named by an opaque
//! handle scoped to the connection that opened it, and the person's first byte into it revokes the
//! handle (**D44**). All six are advertised, because all six are answered.

pub mod client;
pub mod endpoint;
pub mod listener;
pub mod session;
pub mod wire;

pub use client::{Answered, Producer};
pub use listener::{Answer, Channel, Connection, Delivery, ReadDocument, What};
pub use wire::{Activity, AgentSnapshot, MAX_REQUEST, Reply, Request, Side};
