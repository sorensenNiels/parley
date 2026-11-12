//! The 5250 screen (R-UI-1): drawn position by position in a monospace grid that grows with the
//! window, with the cursor; it takes the keyboard while a session is open.

use egui::{
    Align2, Color32, CornerRadius, Event, EventFilter, FontId, Pos2, Rect, Response, Sense, Stroke,
    Ui, Vec2, WidgetInfo,
};
use parley_core::{Color, Key, Session};

use crate::keymap::Keymap;

pub const BACKGROUND: Color32 = Color32::BLACK;

/// The 5250 colours, a little softened for a modern screen.
pub fn color(c: Color) -> Color32 {
    match c {
        Color::Green => Color32::from_rgb(0x4A, 0xE2, 0x4A),
        Color::White => Color32::from_rgb(0xF2, 0xF2, 0xF2),
        Color::Red => Color32::from_rgb(0xFF, 0x55, 0x55),
        Color::Turquoise => Color32::from_rgb(0x40, 0xE0, 0xD0),
        Color::Yellow => Color32::from_rgb(0xFF, 0xF0, 0x5A),
        Color::Pink => Color32::from_rgb(0xFF, 0x80, 0xFF),
        Color::Blue => Color32::from_rgb(0x6C, 0x9C, 0xFF),
    }
}

/// Draws the session's screen in all the space there is and passes the keyboard to it.
/// `grab_focus` takes the keyboard focus (after a toolbar button, for example).
pub fn show(ui: &mut Ui, session: &mut Session, keymap: &mut Keymap, grab_focus: bool) -> Response {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
    let size = session.screen().size();
    let (rows, columns) = (usize::from(size.rows()), usize::from(size.columns()));
    let grid = Grid::fit(ui, response.rect, rows, columns);

    if grab_focus || ui.memory(|m| m.focused().is_none()) {
        response.request_focus();
    }
    if response.clicked() {
        response.request_focus();
        if let Some((row, column)) = response
            .interact_pointer_pos()
            .and_then(|p| grid.cell_at(p))
        {
            session.key(Key::MoveTo { row, column });
        }
    }
    if response.has_focus() {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                response.id,
                EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
        let (events, modifiers) = ui.input(|i| (i.events.clone(), i.modifiers));
        if events.iter().any(|e| matches!(e, Event::Copy)) {
            ui.ctx()
                .copy_text(session.screen().text(session.code_page()));
        }
        for key in keymap.keys(&events, modifiers) {
            session.key(key);
        }
    }

    painter.rect_filled(response.rect, CornerRadius::ZERO, BACKGROUND);
    let cells = session.screen().cells(session.code_page());
    for (i, cell) in cells.iter().enumerate() {
        let rect = grid.cell(i / columns, i % columns);
        let style = cell.style;
        let fg = color(style.color);
        let text_color = if style.reverse {
            painter.rect_filled(rect, CornerRadius::ZERO, fg);
            BACKGROUND
        } else {
            fg
        };
        if cell.ch != ' ' {
            painter.text(
                rect.left_top(),
                Align2::LEFT_TOP,
                cell.ch,
                grid.font.clone(),
                text_color,
            );
        }
        if style.underline {
            painter.hline(
                rect.x_range(),
                rect.bottom() - 1.0,
                Stroke::new(1.0, text_color),
            );
        }
        if style.column_separators {
            let tick = (rect.bottom() - grid.cell_size.y * 0.2)..=rect.bottom();
            painter.vline(rect.left(), tick, Stroke::new(1.0, fg.gamma_multiply(0.6)));
        }
    }
    draw_cursor(&painter, session, &grid, &cells, response.has_focus());

    let text = session.screen().text(session.code_page());
    response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Terminal, true, &text));
    response
}

fn draw_cursor(
    painter: &egui::Painter,
    session: &Session,
    grid: &Grid,
    cells: &[parley_core::screen::Cell],
    focused: bool,
) {
    let screen = session.screen();
    let (row, column) = screen.row_column(screen.cursor);
    let rect = grid.cell(row, column);
    let Some(cell) = cells.get(screen.cursor) else {
        return;
    };
    let color = color(cell.style.color);
    if !focused {
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, color),
            egui::StrokeKind::Inside,
        );
    } else if session.insert_mode() {
        let bar = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 3.0), rect.max);
        painter.rect_filled(bar, CornerRadius::ZERO, color);
    } else {
        painter.rect_filled(rect, CornerRadius::ZERO, color);
        if cell.ch != ' ' {
            painter.text(
                rect.left_top(),
                Align2::LEFT_TOP,
                cell.ch,
                grid.font.clone(),
                BACKGROUND,
            );
        }
    }
}

/// Where the positions go: the largest monospace font that fits, centred in the space.
struct Grid {
    origin: Pos2,
    cell_size: Vec2,
    rows: usize,
    columns: usize,
    font: FontId,
}

impl Grid {
    #[expect(
        clippy::cast_precision_loss,
        reason = "rows and columns are at most 132"
    )]
    fn fit(ui: &Ui, space: Rect, rows: usize, columns: usize) -> Self {
        let measure = |font: &FontId| {
            ui.ctx()
                .fonts_mut(|f| Vec2::new(f.glyph_width(font, 'M'), f.row_height(font)))
        };
        let reference = measure(&crate::fonts::terminal_font(ui.ctx(), 16.0));
        let scale = (space.width() / (reference.x * columns as f32))
            .min(space.height() / (reference.y * rows as f32));
        let size = ((16.0 * scale * 2.0).floor() / 2.0).clamp(6.0, 64.0);
        let font = crate::fonts::terminal_font(ui.ctx(), size);
        let cell_size = measure(&font);
        let grid_size = Vec2::new(cell_size.x * columns as f32, cell_size.y * rows as f32);
        let origin = space.center() - grid_size / 2.0;
        Self {
            origin: origin.max(space.min),
            cell_size,
            rows,
            columns,
            font,
        }
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "rows and columns are at most 132"
    )]
    fn cell(&self, row: usize, column: usize) -> Rect {
        let min = self.origin
            + Vec2::new(
                column as f32 * self.cell_size.x,
                row as f32 * self.cell_size.y,
            );
        Rect::from_min_size(min, self.cell_size)
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked to be inside the grid first"
    )]
    fn cell_at(&self, pos: Pos2) -> Option<(usize, usize)> {
        let offset = pos - self.origin;
        if offset.x < 0.0 || offset.y < 0.0 {
            return None;
        }
        let (row, column) = (
            (offset.y / self.cell_size.y) as usize,
            (offset.x / self.cell_size.x) as usize,
        );
        (row < self.rows && column < self.columns).then_some((row, column))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use parley_core::{CodePage, KeyboardState};

    fn harness() -> Harness<'static, (Session, Keymap)> {
        Harness::builder()
            .with_size(egui::vec2(900.0, 500.0))
            .build_ui_state(
                |ui, (session, keymap)| {
                    show(ui, session, keymap, false);
                },
                (crate::test_support::waiting_session(), Keymap::default()),
            )
    }

    #[test]
    fn the_screen_is_readable_by_assistive_technology() {
        let harness = harness();
        harness.get_by_label_contains("Hello");
    }

    #[test]
    fn typing_goes_into_the_field_and_enter_sends_it() {
        // R-UI-1, R-KEY-1
        let mut harness = harness();
        harness.run();
        harness.event(Event::Text("abc".into()));
        harness.run();
        let screen = harness.state().0.screen().text(CodePage::Ccsid37);
        assert!(
            screen.lines().nth(1).is_some_and(|l| l.starts_with(" abc")),
            "{screen}"
        );

        harness.key_press(egui::Key::Enter);
        harness.run();
        assert_eq!(harness.state().0.keyboard(), KeyboardState::Locked);
        assert_ne!(harness.state_mut().0.take_outbound(), Vec::<u8>::new());
    }

    #[test]
    fn a_click_moves_the_cursor_to_the_position_under_the_pointer() {
        let mut harness = harness();
        harness.run();
        // The grid is centred: just right of and below the centre is row 13, column 41.
        let target = harness.get_by_label_contains("Hello").rect().center() + egui::vec2(3.0, 3.0);
        let button = |pressed| Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        harness.event(Event::PointerMoved(target));
        harness.event(button(true));
        harness.event(button(false));
        harness.run();
        let session = &harness.state().0;
        assert_eq!(
            session.screen().row_column(session.screen().cursor),
            (12, 40)
        );
    }
}
