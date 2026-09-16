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
//! # What this build serves
//!
//! `panel_set`, and nothing else. The three `split_*` verbs are part of the same frozen wire format
//! — they are quoted whole in the delivery's contract so nobody re-derives them — and they are
//! refused **by name** here, because their scoping does not exist yet and a verb that answered
//! before it could be scoped would be a promise taken back later.

pub mod client;
pub mod endpoint;
pub mod listener;
pub mod wire;

pub use client::{Answered, Producer};
pub use listener::{Answer, Channel, Connection, Delivery, What};
pub use wire::{MAX_REQUEST, Reply, Request};
