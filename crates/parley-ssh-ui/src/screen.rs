//! The SSH screen (R-SSH-6..9): draws the terminal cell by cell, sends keys, selects and copies, scrolls back,
//! and keeps the terminal's size to the space it has.

pub mod colors;
pub mod input;

use std::collections::HashMap;
use std::sync::Mutex;

use egui::{
    Align2, CornerRadius, EventFilter, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2, WidgetInfo,
};
use parley_ssh_core::keys;
use parley_ssh_core::net::lock;
use parley_ssh_core::term::{CursorShape, Point, Terminal, Underline, Weight, Width};

pub use colors::Palette;
use input::Typed;

/// The text size of the screen at 100 % (R-SSH-8).
pub const FONT_SIZE: f32 = 14.0;
/// Half a blink of the cursor, in seconds.
const HALF_BLINK: f64 = 0.53;

/// What the screen asks of the session around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    Send(Vec<u8>),
    /// The terminal was resized to fit the window.
    Resized {
        columns: usize,
        rows: usize,
    },
    /// Text to paste: the session asks first when it is several lines (R-SSH-9).
    Paste(String),
}

/// A selection, from where the drag began to where it is (R-SSH-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Selection {
    anchor: Point,
    head: Point,
    /// The terminal's dropped lines when it was made: when lines leave the scrollback, it no longer fits.
    dropped: u64,
}

impl Selection {
    fn ordered(&self) -> (Point, Point) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    fn contains(&self, point: Point) -> bool {
        let (from, to) = self.ordered();
        from <= point && point <= to
    }
}

/// What the screen keeps between frames.
#[derive(Debug, Default)]
pub struct View {
    selection: Option<Selection>,
    /// Whether the fonts have a glyph for each character seen, so a missing one is drawn as a stand-in.
    glyphs: HashMap<char, bool>,
    /// Wheel movement not yet a whole line.
    wheel: f32,
    awaiting_paste: bool,
    focused: bool,
    /// A text size other than 100 %, in percent (the settings' text size).
    pub zoom: u16,
}

impl View {
    /// The selected text, if anything is selected.
    pub fn selected_text(&self, terminal: &Terminal) -> Option<String> {
        let selection = self.selection.filter(|s| s.dropped == terminal.dropped())?;
        let (from, to) = selection.ordered();
        Some(terminal.text(from, to))
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Asks the system for the clipboard; its text comes back as a paste, which is then pasted, not taken for
    /// Ctrl+V (R-SSH-9).
    pub fn request_paste(&mut self, ctx: &egui::Context) {
        self.awaiting_paste = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
    }
}

/// Where the cells go.
struct Grid {
    origin: Pos2,
    cell: Vec2,
    font: FontId,
}

impl Grid {
    #[expect(clippy::cast_precision_loss, reason = "rows and columns are small")]
    fn rect(&self, row: usize, column: usize, width: usize) -> Rect {
        let min = self.origin + Vec2::new(column as f32 * self.cell.x, row as f32 * self.cell.y);
        Rect::from_min_size(min, Vec2::new(self.cell.x * width as f32, self.cell.y))
    }

    /// The cell nearest to `pos`, also outside the screen (a drag past its edge).
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to the screen first"
    )]
    fn nearest(&self, pos: Pos2, rows: usize, columns: usize) -> (usize, usize) {
        let offset = pos - self.origin;
        let clamp = |v: f32, size: f32, count: usize| {
            ((v / size).floor().max(0.0) as usize).min(count.saturating_sub(1))
        };
        (
            clamp(offset.y, self.cell.y, rows),
            clamp(offset.x, self.cell.x, columns),
        )
    }
}

/// How many whole cells fit in `space`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a window's size in cells is small and positive"
)]
fn fit(space: Vec2, cell: Vec2) -> (usize, usize) {
    let columns = (space.x / cell.x).floor().max(1.0) as usize;
    let rows = (space.y / cell.y).floor().max(1.0) as usize;
    (columns, rows)
}

/// Draws the terminal in all the space left and handles the keyboard and mouse; `grab_focus` takes the keyboard.
pub fn show(
    ui: &mut Ui,
    terminal: &Mutex<Terminal>,
    view: &mut View,
    palette: &Palette,
    grab_focus: bool,
) -> Vec<Out> {
    let mut out = Vec::new();
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let size = FONT_SIZE * f32::from(if view.zoom == 0 { 100 } else { view.zoom }) / 100.0;
    let font = parley_ui_egui::fonts::terminal_font(ui.ctx(), size);
    let cell = ui
        .ctx()
        .fonts_mut(|f| Vec2::new(f.glyph_width(&font, 'M'), f.row_height(&font)));
    let grid = Grid {
        origin: response.rect.min,
        cell,
        font,
    };
    let (columns, rows) = fit(response.rect.size(), cell);

    let mut t = lock(terminal);
    if (columns, rows) != (t.columns(), t.rows()) {
        t.resize(columns, rows);
        out.push(Out::Resized { columns, rows });
    }
    if grab_focus || response.clicked() {
        response.request_focus();
    }
    let focused = response.has_focus();
    if focused != view.focused {
        view.focused = focused;
        out.push(Out::Send(keys::focus(focused, t.modes())));
    }
    if focused {
        ui.memory_mut(|m| {
            let all = EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: true,
            };
            m.set_focus_lock_filter(response.id, all);
        });
        let events = ui.input(|i| i.events.clone());
        let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
        for typed in input::read(&events, t.modes(), mac, &mut view.awaiting_paste) {
            match typed {
                Typed::Send(bytes) => {
                    t.scroll_to_bottom();
                    view.selection = None;
                    out.push(Out::Send(bytes));
                }
                Typed::Copy => {
                    if let Some(text) = view.selected_text(&t) {
                        ui.ctx().copy_text(text);
                    }
                }
                Typed::RequestPaste => ui
                    .ctx()
                    .send_viewport_cmd(egui::ViewportCommand::RequestPaste),
                Typed::Paste(text) => out.push(Out::Paste(text)),
                Typed::ScrollPage(direction) => {
                    let page = isize::try_from(t.rows().saturating_sub(1).max(1)).unwrap_or(1);
                    t.scroll_display(direction * page);
                }
            }
        }
    } else if view.awaiting_paste {
        // A menu asked for the clipboard and took the keyboard with its click: its answer is still ours (R-SSH-9).
        let pasted = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Paste(text) => Some(text.clone()),
                _ => None,
            })
        });
        if let Some(text) = pasted {
            view.awaiting_paste = false;
            response.request_focus();
            out.push(Out::Paste(text));
        }
    }
    if response.hovered() {
        scroll_with_wheel(ui, &mut t, view, cell.y);
    }
    select(&response, &grid, &t, view);
    if context_menu(&response, &t, view) {
        response.request_focus();
    }

    let now = ui.input(|i| i.time);
    draw(&painter, &grid, &t, view, palette, response.rect);
    draw_cursor(ui, &painter, &grid, &t, palette, focused, now);
    let text: Vec<String> = (0..t.rows())
        .filter_map(|r| t.visible_row(r))
        .map(parley_ssh_core::term::Row::text)
        .collect();
    drop(t);
    response.widget_info(|| {
        WidgetInfo::labeled(egui::accesskit::Role::Terminal, true, text.join("\n"))
    });
    out
}

/// The right-click menu: Copy, Paste and Copy all (R-SSH-9). True when one was chosen, so the screen takes the
/// keyboard back from the menu.
fn context_menu(response: &egui::Response, t: &Terminal, view: &mut View) -> bool {
    let mut chosen = false;
    response.context_menu(|ui| {
        let selected = view.selected_text(t);
        if ui
            .add_enabled(selected.is_some(), egui::Button::new("Copy"))
            .clicked()
        {
            ui.ctx().copy_text(selected.unwrap_or_default());
            chosen = true;
        }
        if ui.button("Paste").clicked() {
            view.request_paste(ui.ctx());
            chosen = true;
        }
        if ui.button("Copy all").clicked() {
            ui.ctx().copy_text(t.all_text());
            chosen = true;
        }
        if chosen {
            ui.close();
        }
    });
    chosen
}

/// The wheel scrolls the scrollback by lines (R-SSH-8).
#[expect(
    clippy::cast_possible_truncation,
    reason = "a handful of lines per frame"
)]
fn scroll_with_wheel(ui: &Ui, t: &mut Terminal, view: &mut View, line_height: f32) {
    view.wheel += ui.input(|i| i.smooth_scroll_delta.y);
    let lines = (view.wheel / line_height).trunc();
    if lines != 0.0 {
        view.wheel -= lines * line_height;
        t.scroll_display(lines as isize);
    }
}

/// Drag selects; a double click a word; a triple click a line (R-SSH-9).
fn select(response: &egui::Response, grid: &Grid, t: &Terminal, view: &mut View) {
    if view.selection.is_some_and(|s| s.dropped != t.dropped()) {
        view.selection = None;
    }
    let Some(pos) = response.interact_pointer_pos() else {
        return;
    };
    let (row, column) = grid.nearest(pos, t.rows(), t.columns());
    let point = t.point_in_view(row, column);
    let dropped = t.dropped();
    if response.triple_clicked() {
        let end = Point {
            line: point.line,
            column: t.columns().saturating_sub(1),
        };
        view.selection = Some(Selection {
            anchor: Point {
                line: point.line,
                column: 0,
            },
            head: end,
            dropped,
        });
    } else if response.double_clicked() {
        let (from, to) = word_at(t, point);
        view.selection = Some(Selection {
            anchor: from,
            head: to,
            dropped,
        });
    } else if response.drag_started() {
        view.selection = Some(Selection {
            anchor: point,
            head: point,
            dropped,
        });
    } else if response.dragged() {
        if let Some(selection) = &mut view.selection {
            selection.head = point;
        }
    } else if response.clicked() {
        view.selection = None;
    }
}

/// The run of word characters around `point`: letters, digits and `_-./~@:`, as for paths and addresses.
fn word_at(t: &Terminal, point: Point) -> (Point, Point) {
    let Some(row) = t.line(point.line) else {
        return (point, point);
    };
    let is_word = |c: usize| {
        row.cells
            .get(c)
            .is_some_and(|cell| cell.ch.is_alphanumeric() || "_-./~@:".contains(cell.ch))
    };
    if !is_word(point.column) {
        return (point, point);
    }
    let mut from = point.column;
    while from > 0 && is_word(from - 1) {
        from -= 1;
    }
    let mut to = point.column;
    while is_word(to + 1) {
        to += 1;
    }
    (
        Point {
            line: point.line,
            column: from,
        },
        Point {
            line: point.line,
            column: to,
        },
    )
}

fn draw(
    painter: &egui::Painter,
    grid: &Grid,
    t: &Terminal,
    view: &mut View,
    palette: &Palette,
    area: Rect,
) {
    painter.rect_filled(area, CornerRadius::ZERO, palette.background);
    let selection = view.selection.filter(|s| s.dropped == t.dropped());
    for r in 0..t.rows() {
        let Some(row) = t.visible_row(r) else {
            continue;
        };
        for (c, cell) in row.cells.iter().enumerate() {
            if cell.width == Width::Spacer {
                continue;
            }
            let span = if cell.width == Width::Wide { 2 } else { 1 };
            let rect = grid.rect(r, c, span);
            let style = cell.style;
            let bold = style.weight == Weight::Bold;
            let mut fg = palette.foreground_of(style.fg, bold);
            let mut bg = palette.background_of(style.bg);
            if style.inverse {
                std::mem::swap(&mut fg, &mut bg);
            }
            if selection.is_some_and(|s| s.contains(t.point_in_view(r, c))) {
                bg = palette.selection;
            }
            if style.weight == Weight::Faint {
                fg = fg.gamma_multiply(0.6);
            }
            if bg != palette.background {
                painter.rect_filled(rect, CornerRadius::ZERO, bg);
            }
            if style.hidden {
                continue;
            }
            if cell.ch != ' ' || !cell.combining.is_empty() {
                let text = shown_text(painter, view, &grid.font, cell);
                painter.text(
                    rect.left_top(),
                    Align2::LEFT_TOP,
                    &text,
                    grid.font.clone(),
                    fg,
                );
                if bold {
                    // No bold face is bundled: drawn again a little to the right.
                    painter.text(
                        rect.left_top() + Vec2::new(0.6, 0.0),
                        Align2::LEFT_TOP,
                        &text,
                        grid.font.clone(),
                        fg,
                    );
                }
            }
            draw_lines(painter, rect, style.underline, style.strike, fg);
        }
    }
}

/// The cell's text, with a stand-in for characters the fonts cannot draw.
fn shown_text(
    painter: &egui::Painter,
    view: &mut View,
    font: &FontId,
    cell: &parley_ssh_core::term::Cell,
) -> String {
    let drawable = *view
        .glyphs
        .entry(cell.ch)
        .or_insert_with(|| painter.ctx().fonts_mut(|f| f.has_glyph(font, cell.ch)));
    if drawable {
        cell.text()
    } else {
        "?".to_owned()
    }
}

fn draw_lines(
    painter: &egui::Painter,
    rect: Rect,
    underline: Underline,
    strike: bool,
    color: egui::Color32,
) {
    let pen = Stroke::new(1.0, color);
    let bottom = rect.bottom() - 1.5;
    match underline {
        Underline::None => {}
        Underline::Single => {
            painter.hline(rect.x_range(), bottom, pen);
        }
        Underline::Double => {
            painter.hline(rect.x_range(), bottom, pen);
            painter.hline(rect.x_range(), bottom - 2.0, pen);
        }
        Underline::Curly => {
            let points: Vec<Pos2> = (0..=8u8)
                .map(|i| {
                    let x = rect.left() + rect.width() * f32::from(i) / 8.0;
                    let y = if i % 2 == 0 { bottom } else { bottom - 2.0 };
                    Pos2::new(x, y)
                })
                .collect();
            painter.add(egui::Shape::line(points, pen));
        }
    }
    if strike {
        painter.hline(rect.x_range(), rect.center().y, pen);
    }
}

/// The cursor, while the screen (not the scrollback) shows and the host has not hidden it: filled while the screen
/// has the keyboard, an outline otherwise, blinking if the host asked for it.
fn draw_cursor(
    ui: &Ui,
    painter: &egui::Painter,
    grid: &Grid,
    t: &Terminal,
    palette: &Palette,
    focused: bool,
    now: f64,
) {
    let modes = t.modes();
    if t.display_offset() != 0 || !modes.cursor_visible {
        return;
    }
    if focused && modes.cursor_blink {
        let left = HALF_BLINK - now.rem_euclid(HALF_BLINK);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(left));
        if now.rem_euclid(2.0 * HALF_BLINK) >= HALF_BLINK {
            return;
        }
    }
    let (row, column) = t.cursor();
    let wide = t
        .visible_row(row)
        .and_then(|r| r.cells.get(column))
        .is_some_and(|c| c.width == Width::Wide);
    let rect = grid.rect(row, column, if wide { 2 } else { 1 });
    let color = palette.foreground;
    match (modes.cursor_shape, focused) {
        (_, false) => {
            painter.rect_stroke(
                rect.shrink(0.5),
                CornerRadius::ZERO,
                Stroke::new(1.0, color),
                egui::StrokeKind::Inside,
            );
        }
        (CursorShape::Block, true) => {
            painter.rect_filled(rect, CornerRadius::ZERO, color);
            if let Some(cell) = t.visible_row(row).and_then(|r| r.cells.get(column))
                && cell.ch != ' '
            {
                painter.text(
                    rect.left_top(),
                    Align2::LEFT_TOP,
                    cell.text(),
                    grid.font.clone(),
                    palette.background,
                );
            }
        }
        (CursorShape::Underline, true) => {
            let line = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 2.0), rect.max);
            painter.rect_filled(line, CornerRadius::ZERO, color);
        }
        (CursorShape::Bar, true) => {
            let bar = Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height()));
            painter.rect_filled(bar, CornerRadius::ZERO, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    struct Shown {
        terminal: Mutex<Terminal>,
        view: View,
        out: Vec<Out>,
        /// Take the keyboard on the first frame only, as the session does.
        grab: bool,
    }

    fn shown(text: &[u8]) -> Harness<'static, Shown> {
        let mut terminal = Terminal::new(80, 24);
        terminal.feed(text);
        let state = Shown {
            terminal: Mutex::new(terminal),
            view: View::default(),
            out: Vec::new(),
            grab: true,
        };
        Harness::builder()
            .with_size(egui::vec2(640.0, 400.0))
            // Frames as fast as a screen's, so two clicks can make a double click.
            .with_step_dt(1.0 / 60.0)
            .build_ui_state(
                |ui, s: &mut Shown| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    let grab = std::mem::take(&mut s.grab);
                    let out = show(ui, &s.terminal, &mut s.view, &Palette::dark(), grab);
                    s.out.extend(out);
                },
                state,
            )
    }

    #[test]
    fn the_screen_fits_the_space_tells_the_host_and_is_read_by_assistive_technology_r_ssh_8() {
        let mut h = shown("Velkommen æøå".as_bytes());
        h.run_steps(2);
        let resized = h.state().out.iter().find_map(|o| match o {
            Out::Resized { columns, rows } => Some((*columns, *rows)),
            _ => None,
        });
        let (columns, rows) = resized.expect("resized to the space");
        assert!(columns > 40 && rows > 10, "{columns}x{rows}");
        h.get_by_label_contains("Velkommen æøå");
    }

    #[test]
    fn typing_goes_to_the_host_and_scrolls_back_to_the_screen_r_ssh_7_r_ssh_8() {
        let mut h = shown(&b"line\r\n".repeat(60));
        h.run_steps(2);
        net_scroll(&h.state().terminal, 5);
        h.state_mut().out.clear();
        h.event(egui::Event::Text("ls".into()));
        h.key_press(egui::Key::Enter);
        h.run_steps(2);
        let sent: Vec<u8> = h
            .state()
            .out
            .iter()
            .filter_map(|o| {
                if let Out::Send(b) = o {
                    Some(b.clone())
                } else {
                    None
                }
            })
            .flatten()
            .collect();
        assert_eq!(sent, b"ls\r");
        assert_eq!(lock(&h.state().terminal).display_offset(), 0);
    }

    #[test]
    fn a_paste_asked_for_from_a_menu_is_pasted_not_taken_for_ctrl_v_r_ssh_9() {
        let mut h = shown(b"");
        h.run_steps(2);
        let ctx = h.ctx.clone();
        h.state_mut().view.request_paste(&ctx);
        h.state_mut().out.clear();
        h.event(egui::Event::Paste("ls\nwho".into()));
        h.run_steps(2);
        assert!(
            h.state().out.contains(&Out::Paste("ls\nwho".into())),
            "{:?}",
            h.state().out
        );
    }

    fn copied<S>(h: &Harness<'static, S>) -> Option<String> {
        h.output()
            .platform_output
            .commands
            .iter()
            .find_map(|c| match c {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
    }

    /// The middle of the cell at `row`, `column` of a screen at the harness's top left.
    fn cell_centre(h: &Harness<'static, Shown>, row: usize, column: usize) -> egui::Pos2 {
        let font = parley_ui_egui::fonts::terminal_font(&h.ctx, FONT_SIZE);
        let cell = h
            .ctx
            .fonts_mut(|f| Vec2::new(f.glyph_width(&font, 'M'), f.row_height(&font)));
        let origin = h.get_by_role(egui::accesskit::Role::Terminal).rect().min;
        #[expect(clippy::cast_precision_loss, reason = "small cell numbers")]
        let at = origin + Vec2::new((column as f32 + 0.5) * cell.x, (row as f32 + 0.5) * cell.y);
        at
    }

    /// `times` clicks in a row.
    fn click(
        h: &mut Harness<'static, Shown>,
        at: egui::Pos2,
        button: egui::PointerButton,
        times: usize,
    ) {
        h.event(egui::Event::PointerMoved(at));
        for _ in 0..times {
            for pressed in [true, false] {
                h.event(egui::Event::PointerButton {
                    pos: at,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        h.step();
    }

    #[test]
    fn a_word_double_clicked_is_copied_and_pasted_from_the_right_click_menu_r_ssh_9() {
        let mut h = shown(b"echo hello");
        h.run_steps(2);
        let word = cell_centre(&h, 0, 7);
        click(&mut h, word, egui::PointerButton::Primary, 2);
        h.run_steps(2);
        assert_eq!(
            h.state()
                .view
                .selected_text(&lock(&h.state().terminal))
                .as_deref(),
            Some("hello")
        );

        click(&mut h, word, egui::PointerButton::Secondary, 1);
        h.run_steps(2);
        let copy = h.get_by_label("Copy").rect().center();
        click(&mut h, copy, egui::PointerButton::Primary, 1);
        assert_eq!(copied(&h).as_deref(), Some("hello"));
        h.run_steps(2);

        click(&mut h, word, egui::PointerButton::Secondary, 1);
        h.run_steps(2);
        let paste = h.get_by_label("Paste").rect().center();
        click(&mut h, paste, egui::PointerButton::Primary, 1);
        h.run_steps(2);
        // The system answers RequestPaste with the clipboard's text, a frame later.
        h.state_mut().out.clear();
        h.event(egui::Event::Paste("hello".into()));
        h.run_steps(2);
        assert!(
            h.state().out.contains(&Out::Paste("hello".into())),
            "{:?}",
            h.state().out
        );
    }

    fn net_scroll(terminal: &Mutex<Terminal>, lines: isize) {
        lock(terminal).scroll_display(lines);
        assert!(lock(terminal).display_offset() > 0);
    }

    #[test]
    fn the_terminal_gets_as_many_whole_cells_as_fit_r_ssh_8() {
        assert_eq!(
            fit(Vec2::new(805.0, 410.0), Vec2::new(8.0, 20.0)),
            (100, 20)
        );
        assert_eq!(fit(Vec2::new(3.0, 3.0), Vec2::new(8.0, 20.0)), (1, 1));
    }

    #[test]
    fn a_double_click_selects_a_path_or_address_r_ssh_9() {
        let mut t = Terminal::new(40, 2);
        t.feed(b"cd /var/log && ssh me@host.lan");
        let (from, to) = word_at(&t, Point { line: 0, column: 5 });
        assert_eq!(t.text(from, to), "/var/log");
        let (from, to) = word_at(
            &t,
            Point {
                line: 0,
                column: 20,
            },
        );
        assert_eq!(t.text(from, to), "me@host.lan");
        let (from, to) = word_at(&t, Point { line: 0, column: 2 });
        assert_eq!(from, to, "a blank selects nothing more than itself");
    }

    #[test]
    fn a_selection_is_dropped_once_its_lines_leave_the_scrollback() {
        let mut t = Terminal::new(5, 1);
        t.set_history_limit(1);
        t.feed(b"abc");
        let mut view = View {
            selection: Some(Selection {
                anchor: Point { line: 0, column: 0 },
                head: Point { line: 0, column: 2 },
                dropped: t.dropped(),
            }),
            ..View::default()
        };
        assert_eq!(view.selected_text(&t).as_deref(), Some("abc"));
        t.feed(b"\r\n1\r\n2");
        assert_eq!(view.selected_text(&t), None);
        view.clear_selection();
    }
}
