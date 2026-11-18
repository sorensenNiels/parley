//! The 5250 screen (R-UI-1): drawn position by position in a monospace grid that grows with the
//! window, with the cursor; it takes the keyboard while a session is open. Hotspots (R-UI-20) are
//! underlined under the pointer and a click does what they say. Dragging selects a rectangle to
//! copy (R-UI-22), and the find bar's matches are highlighted (R-UI-21).

mod colors;
mod cursor;
mod find;
mod windows;

pub use colors::{COLORS, CustomColors, Palette, Scheme};
pub use cursor::CursorShape;

pub use find::{
    FIELD as FIND_FIELD, FindAction, FindBar, bar as find_bar,
    bar_with_match_case as find_bar_with_match_case,
};

use egui::{
    Align2, Color32, CornerRadius, CursorIcon, EventFilter, FontId, Pos2, Rect, Sense, Stroke, Ui,
    Vec2, WidgetInfo,
};
use parley_core::screen::{Hotspot, Rectangle, Span, Target, hotspots, rectangle_text, table_text};
use parley_core::{Color, Key, Session};

use crate::keymap::{Function, KeyInput, KeyMap, PasteBreaks, Pasted, Toward, pasted};

/// What the operator has on the screen beside the session: the find bar and the selection.
#[derive(Debug, Default)]
pub struct View {
    pub find: Option<FindBar>,
    /// The colours to draw in (R-UI-23): the session's scheme, or the settings'.
    pub palette: Palette,
    /// Blinking text blinks; else it shows steady in reverse image (R-UI-24).
    pub blinking: bool,
    /// Windows are drawn with frames (R-UI-26).
    pub frames: bool,
    /// What a line break does when pasting (R-KEY-12).
    pub paste_breaks: PasteBreaks,
    /// The cursor's shape, and whether it blinks (R-UI-28).
    pub cursor: CursorShape,
    pub cursor_blink: bool,
    /// When the operator last pressed a key (egui's clock): a blinking cursor stays on while
    /// typing (R-UI-28).
    last_key: f64,
    /// The screen's border flashes until this time (egui's clock, seconds) for the alarm
    /// (R-UI-25).
    flash_until: f64,
    selection: Option<Selection>,
}

/// How a position with the blink attribute shows in this frame (R-UI-24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Blink {
    Shown,
    Hidden,
    /// Blinking is turned off: reverse image instead.
    Steady,
}

/// How long the alarm's flash lasts, in seconds (R-UI-25).
const FLASH: f64 = 0.3;
/// Half a blink: shown this long, then hidden as long (R-UI-24).
const HALF_BLINK: f64 = 0.5;

/// A rectangle dragged over the screen (R-UI-22) or grown with Shift and the arrow keys
/// (R-UI-36), with the screen it was made on: it goes when the host changes the screen.
#[derive(Debug)]
struct Selection {
    /// The corner it started at.
    anchor: (usize, usize),
    /// The corner the pointer or the arrow keys move.
    head: (usize, usize),
    screen: Vec<u8>,
}

impl Selection {
    fn new(anchor: (usize, usize), session: &Session) -> Self {
        Self {
            anchor,
            head: anchor,
            screen: session.screen().buffer().to_vec(),
        }
    }

    fn area(&self) -> Rectangle {
        Rectangle::spanning(self.anchor, self.head)
    }
}

impl View {
    /// The host sounded the alarm at `now` (egui's clock): flash the border (R-UI-25).
    pub fn flash(&mut self, now: f64) {
        self.flash_until = now + FLASH;
    }

    /// The alarm has flashed since the view was made, or `otherwise` (tests).
    #[cfg(test)]
    pub(crate) fn flashing_or(&self, otherwise: bool) -> bool {
        self.flash_until > 0.0 || otherwise
    }

    /// Opens the find bar, or gives it the keyboard when it is open (R-UI-21).
    pub fn open_find(&mut self) {
        match &mut self.find {
            Some(bar) => bar.focus(),
            None => self.find = Some(FindBar::opened()),
        }
    }

    /// The part of the screen to copy: the selection, or all of it.
    fn area(&self, session: &Session) -> Rectangle {
        self.selection
            .as_ref()
            .map_or_else(|| Rectangle::all(session.screen()), Selection::area)
    }

    /// The selection as text, or the whole screen without one (R-UI-1, R-UI-22).
    pub fn copy_text(&self, session: &Session) -> String {
        rectangle_text(session.screen(), session.code_page(), self.area(session))
    }

    /// The selection, or the whole screen, as tab-separated columns (R-UI-22).
    pub fn table_text(&self, session: &Session) -> String {
        table_text(session.screen(), session.code_page(), self.area(session))
    }

    /// Drops a selection made on a screen the host has changed since.
    fn forget_old_selection(&mut self, session: &Session) {
        if self
            .selection
            .as_ref()
            .is_some_and(|s| s.screen != session.screen().buffer())
        {
            self.selection = None;
        }
    }

    /// Grows the selection one position `toward`, within the screen; without one, it starts
    /// at the cursor (R-UI-36).
    fn grow(&mut self, toward: Toward, session: &Session) {
        let screen = session.screen();
        let size = screen.size();
        let (last_row, last_column) = (
            usize::from(size.rows()).saturating_sub(1),
            usize::from(size.columns()).saturating_sub(1),
        );
        let selection = self
            .selection
            .get_or_insert_with(|| Selection::new(screen.row_column(screen.cursor), session));
        let (row, column) = &mut selection.head;
        match toward {
            Toward::Up => *row = row.saturating_sub(1),
            Toward::Down => *row = (*row + 1).min(last_row),
            Toward::Left => *column = column.saturating_sub(1),
            Toward::Right => *column = (*column + 1).min(last_column),
        }
    }
}

/// How the cursor shows now (R-UI-28): an outline without the keyboard, else its shape for
/// insert mode, and while it blinks a frame asked for at the next change.
fn cursor_look(ui: &Ui, view: &View, focused: bool, insert: bool, now: f64) -> cursor::Look {
    if !focused {
        return cursor::Look::Outline;
    }
    if view.cursor_blink {
        let left = cursor::until_blink_change(now, view.last_key);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(left));
        if !cursor::blink_visible(now, view.last_key) {
            return cursor::Look::Hidden;
        }
    }
    cursor::Look::Shape(view.cursor.shown(insert))
}

/// Draws the session's screen in all the space there is and passes the keyboard to it, read
/// with `map` (R-KEY-2). `grab_focus` takes the keyboard focus (after a toolbar button, for
/// example). Returns what the keys asked of Parley beside the session: macros.
pub fn show(
    ui: &mut Ui,
    session: &mut Session,
    input: &mut KeyInput,
    map: &KeyMap,
    grab_focus: bool,
    view: &mut View,
) -> Vec<Function> {
    let mut asked = Vec::new();
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let size = session.screen().size();
    let (rows, columns) = (usize::from(size.rows()), usize::from(size.columns()));
    let grid = Grid::fit(ui.ctx(), response.rect, rows, columns);

    if grab_focus || ui.memory(|m| m.focused().is_none()) {
        response.request_focus();
    }
    view.forget_old_selection(session);
    select(ui, &response, &grid, session, view);
    let spots = hotspots(session.screen(), session.code_page());
    let spot_at = |pos: Option<Pos2>| {
        let (row, column) = grid.cell_at(pos?)?;
        spots.iter().find(|h| h.contains(row, column))
    };
    let hovered = spot_at(response.hover_pos());
    if hovered.is_some() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.clicked() {
        response.request_focus();
        view.selection = None;
        let pos = response.interact_pointer_pos();
        if let Some(spot) = spot_at(pos) {
            press(ui, session, spot);
        } else if let Some((row, column)) = pos.and_then(|p| grid.cell_at(p)) {
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
        for function in input.read(&events, modifiers, map) {
            match function {
                Function::Key(key) => {
                    view.selection = None;
                    view.last_key = ui.input(|i| i.time);
                    session.key(key);
                }
                Function::CopyScreen => ui.ctx().copy_text(view.copy_text(session)),
                Function::PasteText(text) => {
                    view.selection = None;
                    paste(session, &text, view.paste_breaks);
                }
                Function::CopyTable => ui.ctx().copy_text(view.table_text(session)),
                Function::Find => view.open_find(),
                Function::Select(toward) => view.grow(toward, session),
                // The text comes back as a paste event, read above next frame.
                Function::Paste => ui
                    .ctx()
                    .send_viewport_cmd(egui::ViewportCommand::RequestPaste),
                other @ (Function::RecordMacro
                | Function::RunMacro(_)
                | Function::Shortcuts
                | Function::Sql
                | Function::Ssh) => {
                    asked.push(other);
                }
            }
        }
    }

    let cells = session.screen().cells(session.code_page());
    let palette = &view.palette;
    let now = ui.input(|i| i.time);
    let blink = blink_now(ui, view, &cells, now);
    draw_cells(
        &painter,
        response.rect,
        &grid,
        &cells,
        &Marks::of(view, session, blink),
    );
    if view.frames {
        windows::draw(
            &painter,
            &grid,
            session.screen(),
            session.code_page(),
            &view.palette,
        );
    }
    flash_border(ui, &painter, response.rect, view, now);
    if let Some(spot) = hovered {
        underline_hotspot(&painter, &grid, &cells, spot, palette);
    }
    let look = cursor_look(ui, view, response.has_focus(), session.insert_mode(), now);
    cursor::draw(&painter, session, &grid, &cells, look, palette);

    let text = session.screen().text(session.code_page());
    response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Terminal, true, &text));
    asked
}

/// How blinking text shows now, and a frame asked for when it changes, only while there is
/// blinking text on the screen (R-UI-24).
fn blink_now(ui: &Ui, view: &View, cells: &[parley_core::screen::Cell], now: f64) -> Blink {
    if !view.blinking {
        return Blink::Steady;
    }
    if cells.iter().any(|c| c.style.blink) {
        let left = HALF_BLINK - now.rem_euclid(HALF_BLINK);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(left));
    }
    blink_phase(now)
}

/// Shown for the first half of each second, hidden for the second (R-UI-24).
fn blink_phase(now: f64) -> Blink {
    if now.rem_euclid(2.0 * HALF_BLINK) < HALF_BLINK {
        Blink::Shown
    } else {
        Blink::Hidden
    }
}

/// The alarm's flash: a thick border in the scheme's yellow until it is over (R-UI-25).
fn flash_border(ui: &Ui, painter: &egui::Painter, area: Rect, view: &View, now: f64) {
    if now >= view.flash_until {
        return;
    }
    painter.rect_stroke(
        area,
        CornerRadius::ZERO,
        Stroke::new(4.0, view.palette.color(Color::Yellow)),
        egui::StrokeKind::Inside,
    );
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs_f64(view.flash_until - now));
}

/// The positions with their characters and attributes, over the marks (R-UI-1).
fn draw_cells(
    painter: &egui::Painter,
    area: Rect,
    grid: &Grid,
    cells: &[parley_core::screen::Cell],
    marks: &Marks,
) {
    let palette = &marks.palette;
    painter.rect_filled(area, CornerRadius::ZERO, palette.background);
    for (i, cell) in cells.iter().enumerate() {
        let (row, column) = (i / grid.columns, i % grid.columns);
        let rect = grid.cell(row, column);
        if let Some(fill) = marks.fill(row, column) {
            painter.rect_filled(rect, CornerRadius::ZERO, fill);
        }
        let style = cell.style;
        let fg = palette.color(style.color);
        let blink = if style.blink {
            marks.blink
        } else {
            Blink::Shown
        };
        let text_color = if style.reverse || blink == Blink::Steady {
            painter.rect_filled(rect, CornerRadius::ZERO, fg);
            palette.background
        } else {
            fg
        };
        if cell.ch != ' ' && blink != Blink::Hidden {
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
}

/// Types pasted text (R-UI-1, R-KEY-12): a line break is Field Exit, or a move one row down
/// to the column the paste began in.
fn paste(session: &mut Session, text: &str, breaks: PasteBreaks) {
    let (_, start_column) = session.screen().row_column(session.screen().cursor);
    for piece in pasted(text) {
        match (piece, breaks) {
            (Pasted::Key(key), _) => session.key(key),
            (Pasted::LineBreak, PasteBreaks::NextField) => session.key(Key::FieldExit),
            (Pasted::LineBreak, PasteBreaks::NextRow) => {
                let (row, _) = session.screen().row_column(session.screen().cursor);
                session.key(Key::MoveTo {
                    row: row + 1,
                    column: start_column,
                });
            }
        }
    }
}

/// Starts, grows or ends a selection as the pointer is dragged over the screen (R-UI-22).
fn select(ui: &Ui, response: &egui::Response, grid: &Grid, session: &Session, view: &mut View) {
    if response.drag_started()
        && let Some(anchor) = ui
            .input(|i| i.pointer.press_origin())
            .and_then(|p| grid.cell_at(p))
    {
        view.selection = Some(Selection::new(anchor, session));
    }
    if response.dragged()
        && let (Some(selection), Some(pos)) = (&mut view.selection, response.interact_pointer_pos())
    {
        selection.head = grid.nearest_cell(pos);
    }
}

/// What is marked on the screen: the find bar's matches, its current one, the selection; in
/// the palette's colours.
struct Marks {
    palette: Palette,
    blink: Blink,
    matches: Vec<Span>,
    current: Option<Span>,
    selection: Option<Rectangle>,
}

impl Marks {
    fn of(view: &View, session: &Session, blink: Blink) -> Self {
        let matches = view
            .find
            .as_ref()
            .map(|f| f.matches(session))
            .unwrap_or_default();
        let current = view
            .find
            .as_ref()
            .and_then(|f| f.current(matches.len()))
            .and_then(|i| matches.get(i).copied());
        Self {
            palette: view.palette,
            blink,
            matches,
            current,
            selection: view.selection.as_ref().map(Selection::area),
        }
    }

    /// The colour under a position, drawn before its character so the text stays readable.
    fn fill(&self, row: usize, column: usize) -> Option<Color32> {
        let find = self.palette.color(Color::Yellow);
        if self.current.is_some_and(|m| m.contains(row, column)) {
            Some(find.gamma_multiply(0.6))
        } else if self.matches.iter().any(|m| m.contains(row, column)) {
            Some(find.gamma_multiply(0.3))
        } else if self.selection.is_some_and(|s| s.contains(row, column)) {
            Some(self.palette.color(Color::Blue).gamma_multiply(0.45))
        } else {
            None
        }
    }
}

/// Does what a clicked hotspot says (R-UI-20): its keys go to the session, a URL to the browser.
fn press(ui: &Ui, session: &mut Session, spot: &Hotspot) {
    if let Target::Url(url) = &spot.target {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
        return;
    }
    let keys = spot.keys(session.screen());
    session.hotspot_keys(&keys);
}

fn underline_hotspot(
    painter: &egui::Painter,
    grid: &Grid,
    cells: &[parley_core::screen::Cell],
    spot: &Hotspot,
    palette: &Palette,
) {
    let Some(cell) = cells.get(spot.row * grid.columns + spot.start) else {
        return;
    };
    let first = grid.cell(spot.row, spot.start);
    let last = grid.cell(spot.row, spot.end.saturating_sub(1));
    painter.hline(
        first.left()..=last.right(),
        first.bottom() - 1.0,
        Stroke::new(1.0, palette.color(cell.style.color)),
    );
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
    fn fit(ctx: &egui::Context, space: Rect, rows: usize, columns: usize) -> Self {
        let measure = |font: &FontId| {
            ctx.fonts_mut(|f| Vec2::new(f.glyph_width(font, 'M'), f.row_height(font)))
        };
        let reference = measure(&crate::fonts::terminal_font(ctx, 16.0));
        let scale = (space.width() / (reference.x * columns as f32))
            .min(space.height() / (reference.y * rows as f32));
        let size = ((16.0 * scale * 2.0).floor() / 2.0).clamp(6.0, 64.0);
        let font = crate::fonts::terminal_font(ctx, size);
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

    /// The position nearest to `pos`, also when it is outside the grid (a drag past its edge).
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to the grid first"
    )]
    fn nearest_cell(&self, pos: Pos2) -> (usize, usize) {
        let offset = pos - self.origin;
        let clamp = |v: f32, size: f32, count: usize| {
            let last = count.saturating_sub(1);
            ((v / size).floor().max(0.0) as usize).min(last)
        };
        (
            clamp(offset.y, self.cell_size.y, self.rows),
            clamp(offset.x, self.cell_size.x, self.columns),
        )
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
    use egui::Event;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use parley_core::{CodePage, KeyboardState};

    type State = (Session, KeyInput, View);

    fn harness() -> Harness<'static, State> {
        harness_with(crate::test_support::waiting_session())
    }

    fn harness_with(session: Session) -> Harness<'static, State> {
        Harness::builder()
            .with_size(egui::vec2(900.0, 500.0))
            .build_ui_state(
                |ui, (session, input, view)| {
                    show(ui, session, input, KeyMap::built_in(), false, view);
                },
                (session, KeyInput::default(), View::default()),
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

    /// The centre of a position on the screen, from the grid the widget fits.
    fn cell_centre(harness: &Harness<'static, State>, row: usize, column: usize) -> Pos2 {
        let rect = harness.get_by_role(egui::accesskit::Role::Terminal).rect();
        Grid::fit(&harness.ctx, rect, 24, 80)
            .cell(row, column)
            .center()
    }

    fn click_cell(harness: &Harness<'static, State>, row: usize, column: usize) {
        let target = cell_centre(harness, row, column);
        let button = |pressed| Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        harness.event(Event::PointerMoved(target));
        harness.event(button(true));
        harness.event(button(false));
    }

    #[test]
    fn a_click_on_a_function_key_legend_presses_the_key_r_ui_20() {
        let mut harness = harness_with(crate::test_support::session_showing("F3=Exit"));
        harness.run();
        harness.state_mut().0.take_outbound();
        click_cell(&harness, 0, 1);
        harness.run();
        let session = &mut harness.state_mut().0;
        assert_eq!(session.keyboard(), KeyboardState::Locked);
        let sent = session.take_outbound();
        assert!(sent.contains(&0x33), "F3's AID in {sent:02X?}");
    }

    #[test]
    fn a_click_on_an_option_types_it_into_the_field_r_ui_20() {
        let mut harness = harness_with(crate::test_support::session_showing("4=Delete"));
        harness.run();
        click_cell(&harness, 0, 0);
        harness.run();
        let session = &harness.state().0;
        assert_eq!(
            session.keyboard(),
            KeyboardState::Unlocked,
            "Enter is not pressed"
        );
        let screen = session.screen().text(CodePage::Ccsid37);
        assert!(
            screen.lines().nth(1).is_some_and(|l| l.starts_with(" 4")),
            "{screen}"
        );
    }

    #[test]
    fn a_click_on_a_url_opens_it_in_the_browser_r_ui_20() {
        let mut harness = harness_with(crate::test_support::session_showing("https://pub400.com"));
        harness.run();
        click_cell(&harness, 0, 3);
        // One frame: the output is that of the frame the click was handled in.
        harness.step();
        let opened =
            harness.output().platform_output.commands.iter().any(
                |c| matches!(c, egui::OutputCommand::OpenUrl(u) if u.url == "https://pub400.com"),
            );
        assert!(opened);
    }

    #[test]
    fn a_hotspot_under_the_pointer_shows_a_hand_r_ui_20() {
        let mut harness = harness_with(crate::test_support::session_showing("F3=Exit"));
        harness.run();
        let over = cell_centre(&harness, 0, 2);
        harness.event(Event::PointerMoved(over));
        harness.run();
        assert_eq!(
            harness.output().platform_output.cursor_icon,
            CursorIcon::PointingHand
        );
        let away = cell_centre(&harness, 10, 10);
        harness.event(Event::PointerMoved(away));
        harness.run();
        assert_ne!(
            harness.output().platform_output.cursor_icon,
            CursorIcon::PointingHand
        );
    }

    #[test]
    fn a_click_beside_a_hotspot_still_moves_the_cursor_r_ui_20() {
        let mut harness = harness_with(crate::test_support::session_showing("F3=Exit"));
        harness.run();
        click_cell(&harness, 0, 9);
        harness.run();
        let session = &harness.state().0;
        assert_eq!(session.screen().row_column(session.screen().cursor), (0, 9));
        assert_eq!(session.keyboard(), KeyboardState::Unlocked);
    }

    #[test]
    fn a_click_moves_the_cursor_to_the_position_under_the_pointer() {
        let mut harness = harness();
        harness.run();
        click_cell(&harness, 12, 40);
        harness.run();
        let session = &harness.state().0;
        assert_eq!(
            session.screen().row_column(session.screen().cursor),
            (12, 40)
        );
    }

    fn drag(harness: &mut Harness<'static, State>, from: (usize, usize), to: (usize, usize)) {
        let start = cell_centre(harness, from.0, from.1);
        let end = cell_centre(harness, to.0, to.1);
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        harness.event(Event::PointerMoved(start));
        harness.event(button(start, true));
        harness.run();
        harness.event(Event::PointerMoved(start + egui::vec2(20.0, 0.0)));
        harness.run();
        harness.event(Event::PointerMoved(end));
        harness.run();
        harness.event(button(end, false));
        harness.run();
    }

    fn copied(harness: &Harness<'static, State>) -> Option<String> {
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .find_map(|c| match c {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
    }

    #[test]
    fn dragging_selects_a_rectangle_that_copy_copies_r_ui_22() {
        let mut harness = harness();
        harness.run();
        drag(&mut harness, (0, 1), (1, 3));
        let (session, _, view) = harness.state();
        assert_eq!(view.copy_text(session), "ell\n");
        let cursor = session.screen().row_column(session.screen().cursor);
        assert_eq!(cursor, (1, 1), "a drag does not move the cursor");
        harness.event(Event::Copy);
        harness.step();
        assert_eq!(copied(&harness).as_deref(), Some("ell\n"));
    }

    #[test]
    fn without_a_selection_copy_takes_the_whole_screen_r_ui_22() {
        let harness = harness();
        let (session, _, view) = harness.state();
        let text = view.copy_text(session);
        assert!(text.starts_with("Hello\n"));
        assert_eq!(text.split('\n').count(), 24);
    }

    #[test]
    fn a_key_or_a_click_drops_the_selection_r_ui_22() {
        let mut harness = harness();
        harness.run();
        drag(&mut harness, (0, 0), (0, 4));
        assert!(harness.state().2.selection.is_some());
        harness.event(Event::Text("x".into()));
        harness.run();
        assert!(harness.state().2.selection.is_none(), "typing drops it");

        drag(&mut harness, (0, 0), (0, 4));
        click_cell(&harness, 10, 10);
        harness.run();
        assert!(harness.state().2.selection.is_none(), "a click drops it");
    }

    #[test]
    fn a_new_screen_from_the_host_drops_the_selection_r_ui_22() {
        let mut harness = harness();
        harness.run();
        drag(&mut harness, (0, 0), (0, 4));
        harness
            .state_mut()
            .0
            .receive(&crate::test_support::screen_record_showing("Other"));
        harness.run();
        assert!(harness.state().2.selection.is_none());
    }

    fn shift_arrow(harness: &mut Harness<'static, State>, key: egui::Key) {
        harness.event(Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::SHIFT,
        });
        harness.run();
    }

    #[test]
    fn shift_and_the_arrow_keys_select_from_the_cursor_r_ui_36() {
        let mut harness = harness();
        harness.state_mut().0.key(Key::MoveTo { row: 0, column: 1 });
        harness.run();
        shift_arrow(&mut harness, egui::Key::ArrowRight);
        shift_arrow(&mut harness, egui::Key::ArrowRight);
        shift_arrow(&mut harness, egui::Key::ArrowDown);
        let (session, _, view) = harness.state();
        assert_eq!(
            view.copy_text(session),
            "ell
"
        );
        let cursor = session.screen().row_column(session.screen().cursor);
        assert_eq!(cursor, (0, 1), "the cursor stays where it was");
        harness.event(Event::Copy);
        harness.step();
        assert_eq!(
            copied(&harness).as_deref(),
            Some(
                "ell
"
            )
        );
    }

    #[test]
    fn shift_and_the_arrow_keys_shrink_and_stop_at_the_screen_edge_r_ui_36() {
        let mut harness = harness();
        harness.state_mut().0.key(Key::MoveTo { row: 0, column: 0 });
        harness.run();
        shift_arrow(&mut harness, egui::Key::ArrowUp);
        shift_arrow(&mut harness, egui::Key::ArrowLeft);
        shift_arrow(&mut harness, egui::Key::ArrowRight);
        shift_arrow(&mut harness, egui::Key::ArrowRight);
        shift_arrow(&mut harness, egui::Key::ArrowLeft);
        let (session, _, view) = harness.state();
        assert_eq!(view.copy_text(session), "He");
    }

    #[test]
    fn shift_and_an_arrow_key_grow_a_dragged_selection_r_ui_36() {
        let mut harness = harness();
        harness.run();
        drag(&mut harness, (0, 0), (0, 2));
        shift_arrow(&mut harness, egui::Key::ArrowRight);
        let (session, _, view) = harness.state();
        assert_eq!(view.copy_text(session), "Hell");
    }

    #[test]
    fn an_arrow_key_without_shift_drops_the_selection_r_ui_36() {
        let mut harness = harness();
        harness.run();
        shift_arrow(&mut harness, egui::Key::ArrowRight);
        assert!(harness.state().2.selection.is_some());
        harness.event(Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
        assert!(harness.state().2.selection.is_none());
    }

    #[test]
    fn copy_as_table_splits_the_columns_with_tabs_r_ui_22() {
        let mut harness = harness_with(crate::test_support::session_showing("Name   Size"));
        harness.run();
        harness.event(Event::Key {
            key: egui::Key::T,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        harness.step();
        let table = copied(&harness).expect("copied");
        assert_eq!(table, "Name\tSize", "blank rows are left out");
    }

    #[test]
    fn ctrl_f_opens_the_find_bar_and_its_matches_are_found_r_ui_21() {
        let mut harness = harness_with(crate::test_support::session_showing("Hello hello"));
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::F);
        harness.run();
        let (session, _, view) = harness.state_mut();
        let bar = view.find.as_mut().expect("the find bar opened");
        bar.text = "HELLO".into();
        assert_eq!(bar.matches(session).len(), 2);
    }

    #[test]
    fn blinking_text_shows_half_a_second_and_hides_half_a_second_r_ui_24() {
        assert_eq!(blink_phase(0.0), Blink::Shown);
        assert_eq!(blink_phase(0.49), Blink::Shown);
        assert_eq!(blink_phase(0.5), Blink::Hidden);
        assert_eq!(blink_phase(0.99), Blink::Hidden);
        assert_eq!(blink_phase(1.2), Blink::Shown);
    }

    #[test]
    fn with_blinking_off_blinking_text_shows_steady_r_ui_24() {
        let mut harness = harness();
        harness.state_mut().2.blinking = false;
        harness.run();
        let ctx = harness.ctx.clone();
        let cells = harness.state().0.screen().cells(CodePage::Ccsid37);
        let blink = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert_eq!(
                blink_now(ui, &harness.state().2, &cells, 0.7),
                Blink::Steady
            );
        });
        drop(blink);
    }

    #[test]
    fn the_alarm_flash_lasts_a_moment_r_ui_25() {
        let mut view = View::default();
        view.flash(10.0);
        assert!(view.flash_until > 10.0 && view.flash_until <= 10.0 + FLASH);
    }

    #[test]
    fn a_pasted_line_break_moves_to_the_next_field_or_the_next_row_r_key_12() {
        for (breaks, cursor) in [
            (PasteBreaks::NextField, (1, 1)),
            (PasteBreaks::NextRow, (2, 1)),
        ] {
            let mut harness = harness();
            harness.state_mut().2.paste_breaks = breaks;
            harness.run();
            harness.event(Event::Paste("ab\r\n".into()));
            harness.run();
            let session = &harness.state().0;
            let screen = session.screen().text(CodePage::Ccsid37);
            assert!(
                screen.lines().nth(1).is_some_and(|l| l.starts_with(" ab")),
                "{screen}"
            );
            assert_eq!(
                session.screen().row_column(session.screen().cursor),
                cursor,
                "{breaks:?}"
            );
        }
    }
}
