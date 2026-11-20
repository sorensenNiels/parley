//! Whole-session tests of the GUI-like 5250 (R-DS-11, R-DS-12, D57): host records in, screen
//! and reply bytes out. Where IBM i's own bytes were published (IBM's TRC5250 traces of DDS
//! menu bars and push buttons, a host's radio buttons), the records are those bytes.

use parley_core::record::{self, Opcode};
use parley_core::session::{MouseAction, MouseButton, MouseEvent};
use parley_core::telnet::Telnet;
use parley_core::{Aid, CodePage, Key, Session, SessionConfig};

const ESC: u8 = 0x04;
const IAC: u8 = 255;
const SBA: u8 = 0x11;
const READ_MDT: [u8; 4] = [ESC, 0x52, 0x00, 0x00];

fn gui_session(on: bool) -> Session {
    Session::new(SessionConfig {
        gui: on,
        ..SessionConfig::default()
    })
}

fn ebcdic(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| CodePage::Ccsid37.to_byte(c).expect("CCSID 37 character"))
        .collect()
}

/// Write To Display with no control actions, then `orders`, then Read MDT Fields.
fn host_screen(orders: &[u8]) -> Vec<u8> {
    let mut data = vec![ESC, 0x40, ESC, 0x11, 0x00, 0x00];
    data.extend(orders);
    data.extend(READ_MDT);
    Telnet::frame(&record::build(0, Opcode::PutGet, &data))
}

/// WDSF with a 5250-class structured field of `kind` and `rest` (SC30-3533-04 §15.6.13).
fn wdsf(kind: u8, rest: &[u8]) -> Vec<u8> {
    let len = u16::try_from(4 + rest.len()).expect("short");
    let mut o = vec![0x15];
    o.extend(len.to_be_bytes());
    o.extend([0xD9, kind]);
    o.extend(rest);
    o
}

/// A minor structure: its length, then `rest`.
fn minor(rest: &[u8]) -> Vec<u8> {
    let mut o = vec![u8::try_from(rest.len() + 1).expect("short")];
    o.extend(rest);
    o
}

/// The data of the only record the session sent.
fn sent_one(session: &mut Session) -> Vec<u8> {
    let bytes = session.take_outbound();
    let mut records = vec![Vec::new()];
    let mut i = 0;
    while i < bytes.len() {
        match (bytes[i], bytes.get(i + 1)) {
            (IAC, Some(&IAC)) => {
                records.last_mut().expect("one").push(IAC);
                i += 2;
            }
            (IAC, Some(&239)) => {
                records.push(Vec::new());
                i += 2;
            }
            (b, _) => {
                records.last_mut().expect("one").push(b);
                i += 1;
            }
        }
    }
    records.pop();
    assert_eq!(records.len(), 1, "expected one record, got {records:?}");
    record::parse(&records[0])
        .expect("a valid record")
        .data
        .to_vec()
}

fn row_text(session: &Session, row: usize) -> String {
    let text = session.screen().text(session.code_page());
    text.lines()
        .nth(row - 1)
        .expect("row exists")
        .trim_end()
        .to_owned()
}

/// A host's three radio buttons at row 20 column 25, as tn5250j recorded them: single choice
/// (X'11'), radio buttons on a GUI display (X'01'), five text positions, three rows.
fn radio_buttons() -> Vec<u8> {
    let mut rest = vec![
        0x00, 0x60, 0x00, 0x11, 0x01, 0x84, 0x84, 0x00, 0x00, 0x05, 0x03, 0x01, 0x01, 0x00, 0x00,
        0x00,
    ];
    rest.extend(minor(&[
        0x01, 0xE0, 0x00, 0x21, 0x00, 0x21, 0x00, 0x3B, 0x22, 0x20, 0x20, 0x20, 0x20, 0x3A, 0x24,
        0x20, 0x20, 0x3A,
    ]));
    for text in ["One  ", "Two  ", "Three"] {
        let mut choice = vec![0x10, 0x08, 0x00, 0xE0, 0x00];
        choice.extend(ebcdic(text));
        rest.extend(minor(&choice));
    }
    let mut o = vec![SBA, 20, 25];
    o.extend(wdsf(0x50, &rest));
    o
}

#[test]
fn a_host_s_radio_buttons_are_written_chosen_with_the_keys_and_sent_r_ds_11() {
    let mut s = gui_session(true);
    s.receive(&host_screen(&radio_buttons()));
    assert_eq!(row_text(&s, 20), format!("{}_ One", " ".repeat(25)));
    assert_eq!(row_text(&s, 22), format!("{}_ Three", " ".repeat(25)));
    s.key(Key::MoveTo {
        row: 19,
        column: 28,
    });
    s.key(Key::Down);
    assert_eq!(s.screen().row_column(s.screen().cursor), (20, 27));
    s.key(Key::Char(' '));
    assert_eq!(row_text(&s, 21), format!("{}/ Two", " ".repeat(25)));
    s.key(Key::Aid(Aid::ENTER));
    // Cursor, Enter, then the field: its address and choice 2 as X'0021'.
    assert_eq!(sent_one(&mut s), [21, 28, 0xF1, SBA, 20, 25, 0x00, 0x21]);
}

#[test]
fn without_the_switch_a_selection_field_is_skipped_r_ds_11() {
    let mut s = gui_session(false);
    s.receive(&host_screen(&radio_buttons()));
    assert_eq!(row_text(&s, 20), "");
    assert_eq!(s.screen().selection_fields(), []);
}

/// IBM i's menu bar from a DDS MNUBAR record (TRC5250): at row 1 column 2, auto-enter,
/// no indicators, nine text positions, four choices, "Drug" (flags X'88') selected, and a
/// separator line under it.
fn menu_bar() -> Vec<u8> {
    let mut rest = vec![
        0x04, 0x08, 0x00, 0x01, 0xF1, 0xF1, 0xF7, 0x00, 0x00, 0x09, 0x01, 0x04, 0x01, 0x00, 0x00,
        0x00,
    ];
    rest.extend(minor(&[
        0x01, 0xE0, 0x00, 0x21, 0x00, 0x23, 0x00, 0x23, 0x22, 0x20, 0x20, 0x22, 0x20, 0x22, 0x00,
        0x20, 0x00, 0x22,
    ]));
    for (flags, text) in [
        (0x08, "Nurse"),
        (0x08, "Procedure"),
        (0x08, "ICD9"),
        (0x88, "Drug"),
    ] {
        // A mnemonic offset of 0: the first letter.
        let mut choice = vec![0x10, flags, 0x00, 0xE0, 0x00];
        choice.extend(ebcdic(text));
        rest.extend(minor(&choice));
    }
    rest.extend([0x08, 0x09, 0x80, 0x01, 0x50, 0x3A, 0x3A, 0x00]);
    let mut o = vec![SBA, 1, 2];
    o.extend(wdsf(0x50, &rest));
    o
}

#[test]
fn ibm_i_s_menu_bar_lies_in_a_row_and_a_click_sends_its_choice_r_ds_11() {
    let mut s = gui_session(true);
    s.receive(&host_screen(&menu_bar()));
    assert_eq!(
        row_text(&s, 1),
        "  Nurse       Procedure   ICD9        Drug"
    );
    let field = &s.screen().selection_fields()[0];
    assert_eq!(
        field.selected().collect::<Vec<_>>(),
        [3],
        "X'88' is selected"
    );
    // "Drug" starts at column 39; IBM i read X'0023' after it was chosen.
    s.click(0, 40);
    assert_eq!(sent_one(&mut s), [1, 39, 0xF1, SBA, 1, 2, 0x00, 0x23]);
}

#[test]
fn a_mnemonic_chooses_a_menu_bar_choice_r_ds_11() {
    let mut s = gui_session(true);
    s.receive(&host_screen(&menu_bar()));
    s.key(Key::MoveTo { row: 0, column: 3 });
    s.key(Key::Char('p'));
    // "Procedure" is the second choice; its text starts at column 15.
    assert_eq!(sent_one(&mut s), [1, 15, 0xF1, SBA, 1, 2, 0x00, 0x21]);
}

/// IBM i's push buttons from a DDS PSHBTNFLD in a pull-down (TRC5250): at row 3 column 31,
/// auto-enter and auto-select, push buttons (X'51'), ten text positions, three rows; each
/// choice with its mnemonic, Enter as its AID, and Roll Left and Right on the cursor keys.
fn push_buttons() -> Vec<u8> {
    let mut rest = vec![
        0x06, 0x06, 0x80, 0x51, 0x21, 0x31, 0x32, 0x00, 0x00, 0x0A, 0x03, 0x01, 0x01, 0x00, 0x00,
        0x00,
    ];
    for (mnemonic, text) in [(0, "Generic "), (1, " Brand  "), (0, "National")] {
        let mut choice = vec![0x10, 0x0C, 0x18, 0xE0, mnemonic, 0xF1];
        choice.extend(ebcdic(text));
        rest.extend(minor(&choice));
    }
    let mut o = vec![SBA, 3, 31];
    o.extend(wdsf(0x50, &rest));
    o.extend([0x13, 3, 32]);
    o
}

#[test]
fn ibm_i_s_push_buttons_send_their_aid_and_choice_from_a_mnemonic_r_ds_11() {
    let mut s = gui_session(true);
    s.receive(&host_screen(&push_buttons()));
    assert_eq!(row_text(&s, 4), format!("{} Brand", " ".repeat(32)));
    s.key(Key::Char('n'));
    // IBM i read X'0022' after "National".
    assert_eq!(sent_one(&mut s), [5, 33, 0xF1, SBA, 3, 31, 0x00, 0x22]);
}

#[test]
fn the_cursor_keys_send_a_push_button_s_roll_aids_r_ds_11() {
    let mut s = gui_session(true);
    s.receive(&host_screen(&push_buttons()));
    s.key(Key::Right);
    assert_eq!(
        sent_one(&mut s),
        [3, 32, 0xDA],
        "Roll Right, nothing modified"
    );
}

#[test]
fn enter_on_a_push_button_presses_it_r_ds_11() {
    let mut s = gui_session(true);
    s.receive(&host_screen(&push_buttons()));
    s.key(Key::Aid(Aid::ENTER));
    assert_eq!(sent_one(&mut s), [3, 32, 0xF1, SBA, 3, 31, 0x00, 0x20]);
}

#[test]
fn remove_gui_selection_field_blanks_it_r_ds_11() {
    let mut s = gui_session(true);
    let mut orders = push_buttons();
    orders.extend([SBA, 3, 31]);
    orders.extend(wdsf(0x58, &[0x00, 0x00]));
    s.receive(&host_screen(&orders));
    assert_eq!(row_text(&s, 4), "");
    assert_eq!(s.screen().selection_fields(), []);
}

#[test]
fn a_selection_field_for_a_display_without_a_mouse_is_skipped_r_ds_11() {
    let mut s = gui_session(true);
    let mut orders = radio_buttons();
    orders[8] = 0x40; // flag byte 1: only without a mouse
    s.receive(&host_screen(&orders));
    assert_eq!(s.screen().selection_fields(), []);
}

#[test]
fn an_unavailable_choice_cannot_be_chosen_r_ds_11() {
    let mut s = gui_session(true);
    let mut orders = radio_buttons();
    // The second choice text's flag byte 1: B'11', not available.
    let second = orders.len() - 2 * 11 + 2;
    orders[second] = 0xC8;
    s.receive(&host_screen(&orders));
    s.click(20, 28);
    assert_eq!(s.screen().selection_fields()[0].selected().count(), 0);
    assert_eq!(s.take_outbound(), [0u8; 0]);
}

#[test]
fn a_define_selection_field_cut_short_is_a_bad_record_r_ds_11() {
    let mut s = gui_session(true);
    let mut orders = vec![SBA, 20, 25];
    orders.extend(wdsf(0x50, &[0x00, 0x60, 0x00, 0x11]));
    s.receive(&host_screen(&orders));
    let notice = s.notice().expect("a notice");
    assert!(notice.contains("cut short"), "{notice}");
}

/// A vertical scroll bar at row 5 column 80: 100 rows, at the first, ten rows high.
fn scroll_bar() -> Vec<u8> {
    let mut o = vec![SBA, 5, 80];
    o.extend(wdsf(0x53, &[0x00, 0x00, 0, 0, 0, 100, 0, 0, 0, 0, 10]));
    o
}

#[test]
fn moving_a_scroll_bar_sends_roll_up_with_its_position_r_ds_12() {
    let mut s = gui_session(true);
    s.receive(&host_screen(&scroll_bar()));
    let bar = &s.screen().scroll_bars()[0];
    assert_eq!((bar.total, bar.position, bar.size), (100, 0, 10));
    s.scroll(0, 30);
    assert_eq!(sent_one(&mut s), [1, 1, 0xF5, SBA, 5, 80, 0, 0, 0, 30]);
}

#[test]
fn a_scroll_bar_past_the_bottom_is_a_bad_record_and_remove_takes_one_away_r_ds_12() {
    let mut s = gui_session(true);
    let mut orders = vec![SBA, 20, 80];
    orders.extend(wdsf(0x53, &[0x00, 0x00, 0, 0, 0, 100, 0, 0, 0, 0, 10]));
    s.receive(&host_screen(&orders));
    assert!(s.notice().is_some_and(|n| n.contains("does not fit")));
    let mut orders = scroll_bar();
    orders.extend([SBA, 5, 80]);
    orders.extend(wdsf(0x5B, &[0x00, 0x00]));
    s.receive(&host_screen(&orders));
    assert_eq!(s.screen().scroll_bars(), []);
}

#[test]
fn write_data_fills_the_entry_field_at_the_address_r_ds_12() {
    let mut s = gui_session(true);
    let mut orders = vec![SBA, 6, 12, 0x1D, 0x40, 0x00, 0x24, 0x00, 10];
    orders.extend([SBA, 6, 13]);
    let mut rest = vec![0x80, 0x00];
    rest.extend(ebcdic("Hej æøå"));
    orders.extend(wdsf(0x54, &rest));
    s.receive(&host_screen(&orders));
    assert_eq!(row_text(&s, 6), format!("{}Hej æøå", " ".repeat(12)));
}

#[test]
fn a_programmable_mouse_button_moves_the_cursor_and_sends_its_aid_r_ds_12() {
    let mut s = gui_session(true);
    // A left double-click (X'03') that moves the cursor and sends F1; a right press then
    // release (X'04', X'05') that sends F3.
    let orders = wdsf(
        0x55,
        &[0x00, 0x00, 0x40, 0x03, 0x00, 0x31, 0x80, 0x04, 0x05, 0x33],
    );
    s.receive(&host_screen(&orders));
    let event = |button, action| MouseEvent {
        button,
        action,
        shift: false,
    };
    assert!(!s.mouse(event(MouseButton::Left, MouseAction::Press), 9, 4));
    assert!(s.mouse(event(MouseButton::Left, MouseAction::DoubleClick), 9, 4));
    assert_eq!(sent_one(&mut s), [10, 5, 0x31]);
}

#[test]
fn a_two_event_mouse_button_sends_on_the_second_event_r_ds_12() {
    let mut s = gui_session(true);
    let orders = wdsf(0x55, &[0x00, 0x00, 0x80, 0x04, 0x05, 0x33]);
    s.receive(&host_screen(&orders));
    let right = |action| MouseEvent {
        button: MouseButton::Right,
        action,
        shift: false,
    };
    assert!(s.mouse(right(MouseAction::Press), 2, 2));
    assert_eq!(s.take_outbound(), [0u8; 0], "waits for the second");
    assert!(s.mouse(right(MouseAction::Release), 2, 2));
    assert_eq!(sent_one(&mut s), [1, 1, 0x33]);
}

#[test]
fn without_the_switch_the_mouse_does_what_it_did_r_ds_12() {
    let mut s = gui_session(false);
    let orders = wdsf(0x55, &[0x00, 0x00, 0x00, 0x01, 0x00, 0x31]);
    s.receive(&host_screen(&orders));
    let press = MouseEvent {
        button: MouseButton::Left,
        action: MouseAction::Press,
        shift: false,
    };
    assert!(!s.mouse(press, 0, 0));
}
