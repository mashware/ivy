//! The interface's shared look: the egui style installed from the theme's roles, and the controls
//! every surface paints with (D94). A surface that needs a control the kit does not have adds it here
//! first, so two screens never grow two versions of the same button.

pub(crate) mod kit;
pub(crate) mod style;
