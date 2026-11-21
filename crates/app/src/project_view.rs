//! Project presentation is independent of console titles and producer attention.
use crate::theme::project_inks_distinct as separated;
use crate::{
    Window,
    project::{Identity, Key, Projects},
    theme::Theme,
};
use eframe::egui::{self, Color32};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub(crate) struct Badge {
    /// The identity itself, so grouping never depends on how its name was disambiguated for display.
    pub(crate) key: Key,
    pub(crate) name: String,
    pub(crate) details: String,
    pub(crate) ink: Color32,
    pub(crate) painted: Arc<AtomicBool>,
}

#[derive(Default)]
pub(crate) struct State {
    enabled: bool,
    probe: Option<PathBuf>,
    resolver: Projects,
    native: HashMap<usize, (Instant, Option<PathBuf>)>,
    sources: HashMap<usize, Option<PathBuf>>,
    reports: HashMap<usize, u64>,
    assignments: HashMap<Key, Color32>,
    theme: Option<Theme>,
    painted: HashMap<usize, Arc<AtomicBool>>,
    observed: usize,
    resolved: usize,
    pending: Duration,
    total: Duration,
    frames: usize,
}

pub(crate) fn probe_at_startup(measuring: bool) -> Option<PathBuf> {
    measuring
        .then(|| std::env::var_os("MEASURE_PROJECTS").map(PathBuf::from))
        .flatten()
}

pub(crate) fn configure(
    window: &mut Window,
    entry: crate::settings::Entry,
    probe: Option<PathBuf>,
) {
    window.projects.enabled = entry == crate::settings::Entry::Normal || probe.is_some();
    window.projects.probe = probe;
}

impl State {
    pub(crate) fn directory(&self, pane: usize) -> Option<&std::path::Path> {
        self.sources.get(&pane).and_then(Option::as_deref)
    }

    #[cfg(test)]
    pub(crate) fn reference_report() -> String {
        let mut state = Self {
            enabled: true,
            probe: Some(PathBuf::from("/synthetic-projects")),
            observed: 4,
            resolved: 4,
            pending: Duration::from_millis(1),
            ..Default::default()
        };
        for id in 0..4 {
            state.painted.insert(id, Arc::new(AtomicBool::new(true)));
        }
        state.close_frame();
        let mut report = Vec::new();
        state.publish(&mut report).unwrap();
        String::from_utf8(report).unwrap()
    }

    pub(crate) fn close_frame(&mut self) {
        if self.enabled {
            self.total += std::mem::take(&mut self.pending);
            self.frames += 1;
        }
    }
    pub(crate) fn publish(&self, to: &mut dyn std::io::Write) -> std::io::Result<()> {
        writeln!(
            to,
            "project accents: enabled={}, probe={}, observed={}, resolved={}, painted={}",
            self.enabled,
            self.probe.is_some(),
            self.observed,
            self.resolved,
            self.painted
                .values()
                .filter(|painted| painted.load(Ordering::Relaxed))
                .count()
        )?;
        writeln!(
            to,
            "project identity per frame (native metadata + bookkeeping, excludes background Git): mean_us={:.3}, frames={}",
            self.total.as_secs_f64() * 1e6 / self.frames.max(1) as f64,
            self.frames
        )
    }
}

pub(crate) fn refresh(window: &mut Window, ctx: &egui::Context) {
    // Everything derived from the effective theme refreshes here; the style only when it changed.
    crate::ui::style::install(ctx, &window.theme);
    if !window.projects.enabled {
        return;
    }
    let started = Instant::now();
    let mut state = std::mem::take(&mut window.projects);
    let live: Vec<_> = window.all_panes().map(|p| p.name).collect();
    state.native.retain(|id, _| live.contains(id));
    state.painted.retain(|id, _| live.contains(id));
    state.sources.retain(|id, _| live.contains(id));
    state.reports.retain(|id, _| live.contains(id));
    state.resolver.retain(&live);
    let mut changed = state.theme != Some(window.theme);
    changed |= state.resolver.poll();
    for pane in window.all_panes_mut() {
        let directory = if let Some(root) = &state.probe {
            // The probe injects directory sources, but resolves and paints real repositories.
            Some(root.join(if pane.name % 2 == 0 { "a" } else { "b" }))
        } else if pane.screen.working_directory_reported()
            && pane.screen.working_directory().is_none()
        {
            state
                .reports
                .insert(pane.name, pane.screen.working_directory_generation());
            if let Some(cached) = state.native.get_mut(&pane.name) {
                cached.1 = None;
            }
            None
        } else {
            let published = pane
                .screen
                .working_directory()
                .map(std::path::Path::to_path_buf);
            let generation = pane.screen.working_directory_generation();
            if state.reports.get(&pane.name) != Some(&generation) {
                state.reports.insert(pane.name, generation);
                // A newly published directory is newer than the cached native observation.
                if pane.screen.working_directory_reported()
                    && let Some(cached) = state.native.get_mut(&pane.name)
                {
                    cached.1 = None;
                }
            }
            let cached = state
                .native
                .entry(pane.name)
                .or_insert((started - Duration::from_secs(1), None));
            if started.duration_since(cached.0) >= Duration::from_secs(1) {
                *cached = (started, pane.pty.working_directory());
                pane.program = pane.pty.foreground_program().map(|program| {
                    let folder = cached
                        .1
                        .as_deref()
                        .and_then(std::path::Path::file_name)
                        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
                    (folder, program)
                });
                state.observed += 1;
            }
            cached.1.clone().or(published)
        };
        if state.sources.get(&pane.name) != Some(&directory) {
            changed = true;
            state.observed += usize::from(state.probe.is_some());
            state.sources.insert(pane.name, directory.clone());
        }
        state.resolver.observe(pane.name, directory, started, ctx);
    }
    let identities: Vec<_> = live
        .iter()
        .filter_map(|id| state.resolver.identity(*id).map(|p| (*id, p.clone())))
        .collect();
    let keys: HashSet<_> = identities.iter().map(|(_, p)| p.key.clone()).collect();
    if state.assignments.keys().any(|key| !keys.contains(key)) {
        changed = true;
    }
    state.resolved = identities.len();
    if changed {
        let theme_changed = state.theme != Some(window.theme);
        state.assignments.retain(|key, _| keys.contains(key));
        if theme_changed {
            state.assignments.clear();
        }
        let candidates = crate::theme::project_accents(&window.theme);
        for (_, identity) in &identities {
            if !state.assignments.contains_key(&identity.key) {
                let ink = next_ink(
                    &candidates,
                    state.assignments.values().copied(),
                    window.theme.foreground(),
                );
                state.assignments.insert(identity.key.clone(), ink);
            }
        }
        for pane in window.all_panes_mut() {
            pane.project = state.resolver.identity(pane.name).map(|identity| Badge {
                key: identity.key.clone(),
                name: badge_name(identity, &identities),
                details: format!("{}\n{}", identity.name, identity.path.display()),
                ink: state.assignments[&identity.key],
                painted: {
                    let marker = Arc::new(AtomicBool::new(false));
                    state.painted.insert(pane.name, marker.clone());
                    marker
                },
            });
            if pane.project.is_none() {
                state.painted.remove(&pane.name);
            }
        }
        state.theme = Some(window.theme);
    }
    crate::board::plans::refresh_readers(window);
    let cost = started.elapsed();
    state.pending += cost;
    window.timed_separately += cost;
    window.projects = state;
}

/// The ink a newly seen project takes: the first candidate distinct from every ink in use; once
/// every candidate is taken, the list again from its start, so the tenth project repeats the first
/// named colour instead of taking the text colour — which on Ivy Night is one step from the list's
/// white (#141). The text colour only when the theme offers no candidate at all.
pub(crate) fn next_ink(
    candidates: &[Color32],
    used: impl Iterator<Item = Color32> + Clone,
    fallback: Color32,
) -> Color32 {
    if let Some(free) = candidates
        .iter()
        .copied()
        .find(|ink| used.clone().all(|other| separated(*ink, other)))
    {
        return free;
    }
    match candidates.len() {
        0 => fallback,
        n => candidates[used.count() % n],
    }
}

fn badge_name(identity: &Identity, all: &[(usize, Identity)]) -> String {
    let name = display_name(&identity.name);
    if all
        .iter()
        .any(|(_, other)| other.key != identity.key && display_name(&other.name) == name)
    {
        let path = naming_path(identity);
        let parts: Vec<_> = path.components().collect();
        for start in (0..parts.len().saturating_sub(1)).rev() {
            let suffix: PathBuf = parts[start..].iter().map(|part| part.as_os_str()).collect();
            if all.iter().all(|(_, other)| {
                other.key == identity.key
                    || display_name(&other.name) != name
                    || !display_name(&naming_path(other).to_string_lossy())
                        .ends_with(&display_name(&suffix.to_string_lossy()))
            }) {
                return display_name(&suffix.to_string_lossy());
            }
        }
        return path.display().to_string();
    }
    name
}

fn display_name(name: &str) -> String {
    let mut uppercase = true;
    let mut result = String::new();
    for character in name.chars() {
        if character == '-' || character == '_' || character.is_whitespace() {
            uppercase = true;
        } else if character == '/' || character == '\\' {
            result.push(character);
            uppercase = true;
        } else {
            if uppercase {
                result.extend(character.to_uppercase());
            } else {
                result.push(character);
            }
            uppercase = false;
        }
    }
    if result.is_empty() {
        name.into()
    } else {
        result
    }
}

fn naming_path(identity: &Identity) -> &std::path::Path {
    if matches!(identity.key, Key::Repository(_))
        && identity.name != ".git"
        && identity.path.file_name().is_some_and(|name| name == ".git")
    {
        identity.path.parent().unwrap_or(&identity.path)
    } else {
        &identity.path
    }
}

#[cfg(test)]
mod tests;
