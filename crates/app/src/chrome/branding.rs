//! The approved Ivy identity, embedded at startup (D71).
use eframe::egui::{self, Color32, Rect, Vec2};

// D71: decode the embedded artwork before the first frame and share it with the title bar.
fn icon() -> &'static egui::IconData {
    static ICON: std::sync::OnceLock<egui::IconData> = std::sync::OnceLock::new();
    ICON.get_or_init(|| {
        eframe::icon_data::from_png_bytes(include_bytes!("../../../../resources/branding/ivy.png"))
            .expect("the embedded Ivy icon must be a valid PNG")
    })
}

pub(crate) fn native_options(integrated: bool) -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: super::viewport(egui::ViewportBuilder::default(), integrated)
            .with_title(crate::APP_NAME)
            .with_icon(icon().clone()),
        ..Default::default()
    }
}

pub(crate) fn install_logo(ctx: &egui::Context) {
    let icon = icon();
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    );
    let texture = ctx.load_texture("ivy-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("ivy-logo"), texture));
}

/// The mark alone in its slot after the split buttons, as the kit draws it: the window's name is
/// the native title's job, and the tabs need the width the word took.
pub(super) fn paint_logo(ui: &egui::Ui, slot: Rect) {
    let texture = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(egui::Id::new("ivy-logo")));
    if let Some(texture) = texture {
        ui.painter_at(slot).image(
            texture.id(),
            Rect::from_center_size(slot.center(), Vec2::splat(20.0)),
            Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_window_receives_the_ivy_name_and_transparent_green_icon() {
        for integrated in [false, true] {
            let options = native_options(integrated);
            assert_eq!(options.viewport.title.as_deref(), Some("Ivy"));
            let icon = options
                .viewport
                .icon
                .expect("Ivy must replace the framework icon");
            assert_eq!((icon.width, icon.height), (512, 512));
            assert_eq!(icon.rgba.len(), 512 * 512 * 4);
            assert_eq!(&icon.rgba[..4], &[0, 0, 0, 0]);
            assert!(
                icon.rgba
                    .chunks_exact(4)
                    .any(|p| p[1] > p[0] && p[1] > p[2] && p[3] == 255)
            );
        }
    }
}
