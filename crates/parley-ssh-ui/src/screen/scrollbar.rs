//! The scroll bar at the screen's right edge (R-SSH-8), with the shell's commands marked on its track (R-SSH-45):
//! a failed command's mark is red. Dragging or clicking it scrolls; it shows while there is scrollback.

use egui::{Color32, CornerRadius, Rect, Sense, Ui, pos2, vec2};
use parley_ssh_core::term::{CommandMark, Point, Terminal};

use super::Palette;

/// The strip's width, kept free of cells.
pub const WIDTH: f32 = 10.0;
/// The thumb's least height.
const MIN_THUMB: f32 = 18.0;

/// Draws the bar in `strip` and scrolls `t` when it is dragged or clicked.
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "line counts are far below f32's exact range; the result is clamped to the scrollback"
)]
pub fn show(ui: &Ui, strip: Rect, t: &mut Terminal, palette: &Palette) {
    let history = t.history_len();
    if history == 0 || t.alternate_screen() || strip.height() <= MIN_THUMB {
        return;
    }
    let total = (history + t.rows()) as f32;
    let height = strip.height();
    let response = ui.interact(
        strip,
        ui.scope_id().with("ssh-scrollbar"),
        Sense::click_and_drag(),
    );
    if let Some(pos) = response.interact_pointer_pos()
        && (response.dragged() || response.clicked())
    {
        // The pointer marks the middle of what is shown.
        let middle = ((pos.y - strip.top()) / height * total).max(0.0) as usize;
        let top = middle.saturating_sub(t.rows() / 2);
        t.show_line_at_top(top);
    }
    let painter = ui.painter();
    let track = palette.foreground.gamma_multiply(0.08);
    painter.rect_filled(strip, CornerRadius::same(3), track);
    for command in t.commands() {
        let y = strip.top() + command.prompt as f32 / total * height;
        let colour = mark_colour(&command, palette);
        painter.rect_filled(
            Rect::from_min_size(pos2(strip.left() + 1.0, y), vec2(strip.width() - 2.0, 2.0)),
            CornerRadius::ZERO,
            colour,
        );
    }
    let top_line = (history - t.display_offset()) as f32;
    let thumb_height = (t.rows() as f32 / total * height).max(MIN_THUMB);
    let thumb_top = (strip.top() + top_line / total * height).min(strip.bottom() - thumb_height);
    let thumb = Rect::from_min_size(
        pos2(strip.left() + 2.0, thumb_top),
        vec2(strip.width() - 4.0, thumb_height),
    );
    let strength = if response.hovered() || response.dragged() {
        0.55
    } else {
        0.3
    };
    painter.rect_filled(
        thumb,
        CornerRadius::same(3),
        palette.foreground.gamma_multiply(strength),
    );
}

/// Red for a failed command, else the text colour faded.
fn mark_colour(command: &CommandMark, palette: &Palette) -> Color32 {
    if command.failed() {
        palette.ansi(1)
    } else {
        palette.foreground.gamma_multiply(0.6)
    }
}

/// The command whose prompt or output holds `line`.
pub fn command_at(t: &Terminal, line: usize) -> Option<CommandMark> {
    t.commands().into_iter().rev().find(|c| c.prompt <= line)
}

/// The command before or after the one at the view's top (Ctrl+Shift+Up and Down, R-SSH-45), scrolled to the
/// top; after the last one, the screen again.
pub fn jump(t: &mut Terminal, back: bool) {
    let top = t.point_in_view(0, 0).line;
    let prompts: Vec<usize> = t.commands().iter().map(|c| c.prompt).collect();
    let target = if back {
        prompts.iter().rev().find(|p| **p < top).copied()
    } else {
        prompts.iter().find(|p| **p > top).copied()
    };
    match target {
        Some(line) => t.show_line_at_top(line),
        None if !back => t.scroll_to_bottom(),
        None => {}
    }
}

/// The text of a command's output, lines joined as Copy joins them.
pub fn output_text(t: &Terminal, command: &CommandMark) -> Option<String> {
    let (from, to) = command.output_lines()?;
    Some(t.text(
        Point {
            line: from,
            column: 0,
        },
        Point {
            line: to,
            column: t.columns().saturating_sub(1),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell() -> Terminal {
        let mut t = Terminal::new(20, 3);
        for (command, status) in [("ls", 0), ("false", 1), ("pwd", 0)] {
            t.feed(format!("\x1b]133;A\x07$ {command}\r\n\x1b]133;C\x07out {command}\r\n\x1b]133;D;{status}\x07").as_bytes());
        }
        t.feed(b"\x1b]133;A\x07$ ");
        t
    }

    #[test]
    fn ctrl_shift_up_and_down_go_from_command_to_command_r_ssh_45() {
        let mut t = shell();
        assert_eq!(t.history_len(), 4);
        // Prompts on lines 0, 2, 4 and 6; the view shows lines 4 to 6.
        jump(&mut t, true);
        assert_eq!(
            t.point_in_view(0, 0).line,
            2,
            "the prompt before the view's top"
        );
        jump(&mut t, true);
        assert_eq!(t.point_in_view(0, 0).line, 0);
        jump(&mut t, true);
        assert_eq!(t.point_in_view(0, 0).line, 0, "none before the first");
        jump(&mut t, false);
        assert_eq!(t.point_in_view(0, 0).line, 2);
        jump(&mut t, false);
        jump(&mut t, false);
        assert_eq!(t.display_offset(), 0, "the last ones are on the screen");
    }

    #[test]
    fn a_commands_output_is_found_from_any_of_its_lines_r_ssh_45() {
        let t = shell();
        let failed = command_at(&t, 3).expect("a command");
        assert!(failed.failed());
        assert_eq!(output_text(&t, &failed).as_deref(), Some("out false"));
        assert_eq!(
            mark_colour(&failed, &Palette::dark()),
            Palette::dark().ansi(1)
        );
        assert_eq!(command_at(&t, 0).map(|c| c.prompt), Some(0));
    }
}
