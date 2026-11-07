//! Transient mixed-workload probe. It never opens or writes a configuration file.
use std::collections::BTreeMap;
use term_core::Size;

#[derive(Default)]
pub(super) struct Probe {
    pub baseline: Option<BTreeMap<usize, Size>>,
    pub stage: u8,
    pub changed: usize,
    pub observed: usize,
    pub color: bool,
    pub font_active: bool,
    pub unchanged: bool,
    pub reset: bool,
    pub warm: bool,
    pub failure: bool,
    pub family: Option<String>,
    pub preparation: [Option<std::time::Duration>; 4],
}
impl Probe {
    pub fn publish(&self, to: &mut dyn std::io::Write) -> std::io::Result<()> {
        for (index, label) in [
            "missing family",
            "first selection",
            "clear selection",
            "repeated selection",
        ]
        .iter()
        .enumerate()
        {
            writeln!(
                to,
                "settings probe {label}: attempts={}, total={:?}",
                usize::from(self.preparation[index].is_some()),
                self.preparation[index]
            )?;
        }
        writeln!(to, "settings persistence: vetoed")?;
        writeln!(to, "settings probe family: {:?}", self.family.as_deref())?;
        writeln!(
            to,
            "settings probe: baseline={}, changed={}, observed={}",
            self.baseline.as_ref().map_or(0, BTreeMap::len),
            self.changed,
            self.observed
        )?;
        writeln!(
            to,
            "settings probe effects: color={}, font={}, unchanged={}, reset={}, warm={}, failure={}",
            self.color, self.font_active, self.unchanged, self.reset, self.warm, self.failure
        )
    }
}
