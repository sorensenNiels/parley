//! What every dialog shares, as in Tortuga (D12): the window frame and margins, a title row
//! with a close button (🗙) on the right (R-UI-7), and the Cancel / confirm buttons.

use egui::{Context, Id, Modal, Role, Ui, WidgetInfo};

use crate::theme;

/// Shows a modal dialog `width` wide (less on a small window) with `title` and a close
/// button. Returns whether the close button was clicked, and what `content` returned.
pub fn dialog<R>(
    ctx: &Context,
    id: &str,
    title: &str,
    width: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> (bool, R) {
    let frame = egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::same(22));
    Modal::new(Id::unique(id))
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(width.min(ctx.content_rect().width() - 48.0));
            let close = title_bar(ui, title);
            ui.add_space(10.0);
            (close, content(ui))
        })
        .inner
}

/// A dialog's content that scrolls when the window is too small for it, leaving `room`
/// for the rest of the dialog. `last_height` is the content's height last frame, kept by
/// the dialog: a modal only offers a little more room each frame, so the scroll area is
/// given that height at once rather than growing over many frames, which would also
/// move the buttons under a click.
pub fn fitting_scroll_area<R>(
    ui: &mut Ui,
    id: &str,
    room: f32,
    last_height: &mut f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let max_height = (ui.ctx().content_rect().height() - room).max(200.0);
    let output = egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, true])
        .max_height(max_height)
        .min_scrolled_height(last_height.min(max_height))
        .show(ui, add);
    let height = output.content_size.y;
    if (height - *last_height).abs() > 0.5 {
        *last_height = height;
        // The dialog is centred on its size: draw again now, not when the pointer moves.
        ui.ctx().request_repaint();
    }
    output.inner
}

/// A title with a close button (🗙) at the right; true when it was clicked. Screen readers
/// call the button "Close".
pub fn title_bar(ui: &mut Ui, title: &str) -> bool {
    ui.horizontal(|ui| {
        ui.heading(title);
        theme::button_row(ui, |ui| {
            let button = ui.add(theme::icon_button(ui, "🗙")).on_hover_text("Close");
            button.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Close"));
            button.clicked()
        })
    })
    .inner
}

/// Cancel and a confirm button, on the right. Returns (cancel clicked, confirm clicked).
pub fn actions(ui: &mut Ui, confirm: egui::Button<'_>, confirm_enabled: bool) -> (bool, bool) {
    ui.add_space(16.0);
    theme::button_row(ui, |ui| {
        let confirm = ui.add_enabled(confirm_enabled, confirm).clicked();
        let cancel = ui.button("Cancel").clicked();
        (cancel, confirm)
    })
}
