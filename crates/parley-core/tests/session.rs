//! Whole-session tests: host records in, screen and reply bytes out. The records are built by
//! hand from the 5250 Functions Reference; requirement IDs are in the test names' comments.

use parley_core::password::Password;
use parley_core::record::{self, Opcode};
use parley_core::screen::ScreenSize;
use parley_core::telnet::Telnet;
use std::time::{Duration, Instant};

use parley_core::macros::Step;
use parley_core::session::MACRO_TIMEOUT;
use parley_core::{
    Aid, CodePage, Key, KeyboardState, MacroState, OperatorError, Session, SessionConfig,
};

const ESC: u8 = 0x04;
const IAC: u8 = 255;
const SBA: u8 = 0x11;
const READ_MDT: [u8; 4] = [ESC, 0x52, 0x00, 0x00];

fn session() -> Session {
    Session::new(SessionConfig {
        host: "test".into(),
        ..SessionConfig::default()
    })
}

fn host_record(opcode: Opcode, data: &[u8]) -> Vec<u8> {
    Telnet::frame(&record::build(0, opcode, data))
}

fn host(data: &[u8]) -> Vec<u8> {
    host_record(Opcode::PutGet, data)
}

fn ebcdic(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| CodePage::Ccsid37.to_byte(c).expect("CCSID 37 character"))
        .collect()
}

/// The records the session sent, without telnet framing.
fn sent(session: &mut Session) -> Vec<Vec<u8>> {
    let bytes = session.take_outbound();
    let mut records = Vec::new();
    let mut current = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match (bytes[i], bytes.get(i + 1)) {
            (IAC, Some(&IAC)) => {
                current.push(IAC);
                i += 2;
            }
            (IAC, Some(&239)) => {
                records.push(std::mem::take(&mut current));
                i += 2;
            }
            (b, _) => {
                current.push(b);
                i += 1;
            }
        }
    }
    records
}

/// The data of the only record the session sent, and its opcode and flags.
fn sent_one(session: &mut Session) -> (Opcode, u8, Vec<u8>) {
    let records = sent(session);
    assert_eq!(records.len(), 1, "expected one record, got {records:?}");
    let record = record::parse(&records[0]).expect("a valid record");
    (record.opcode, record.flags, record.data.to_vec())
}

fn keys(session: &mut Session, text: &str) {
    for c in text.chars() {
        session.key(Key::Char(c));
    }
}

fn address(session: &Session, row: u8, column: u8) -> usize {
    session
        .screen()
        .address(row, column)
        .expect("on the screen")
}

fn row_text(session: &Session, row: usize) -> String {
    let text = session.screen().text(session.code_page());
    text.lines()
        .nth(row - 1)
        .expect("row exists")
        .trim_end()
        .to_owned()
}

/// Write To Display with no control actions, then `orders`, then Read MDT Fields.
fn screen_with(orders: &[u8]) -> Vec<u8> {
    let mut data = vec![ESC, 0x40, ESC, 0x11, 0x00, 0x00];
    data.extend(orders);
    data.extend(READ_MDT);
    data
}

/// Start of Field with a field format word: `SF ffw attribute length`.
fn field(ffw: [u8; 2], attribute: u8, len: u8) -> Vec<u8> {
    vec![0x1D, ffw[0], ffw[1], attribute, 0x00, len]
}

/// A sign-on screen like IBM i's: a monocase user field and a non-display password field.
fn sign_on() -> Vec<u8> {
    let mut o = vec![0x01, 0x07, 0, 0, 0, 24, 0, 0, 0]; // Start of Header, error row 24
    o.extend([SBA, 1, 30, 0x22]);
    o.extend(ebcdic("Sign On"));
    o.push(0x20);
    o.extend([SBA, 6, 17]);
    o.extend(ebcdic("User . . ."));
    o.extend([SBA, 6, 52]);
    o.extend(field([0x40, 0x20], 0x24, 10));
    o.extend([SBA, 6, 63, 0x20]);
    o.extend([SBA, 7, 17]);
    o.extend(ebcdic("Password ."));
    o.extend([SBA, 7, 52]);
    o.extend(field([0x40, 0x00], 0x27, 10));
    o.extend([SBA, 7, 63, 0x20]);
    o.extend([0x13, 6, 53]); // Insert Cursor
    screen_with(&o)
}

fn signed_on() -> Session {
    let mut s = session();
    s.receive(&host(&sign_on()));
    s
}

#[test]
fn the_sign_on_screen_shows_and_the_keyboard_unlocks_at_the_insert_cursor() {
    // R-DS-1, R-DS-2
    let s = signed_on();
    assert_eq!(row_text(&s, 1).trim(), "Sign On");
    assert!(row_text(&s, 6).contains("User . . ."));
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
    assert_eq!(s.screen().cursor, address(&s, 6, 53));
    assert_eq!(s.screen().fields().len(), 2);
    assert_eq!(s.notice(), None);
}

#[test]
fn enter_sends_the_cursor_aid_and_modified_fields_then_locks() {
    // R-DS-3, R-KEY-1
    let mut s = signed_on();
    keys(&mut s, "qsecofr");
    s.key(Key::Tab);
    keys(&mut s, "secret");
    assert!(
        !s.screen().text(s.code_page()).contains("secret"),
        "the password is not shown"
    );
    s.key(Key::Aid(Aid::ENTER));

    let (opcode, flags, data) = sent_one(&mut s);
    assert_eq!((opcode, flags), (Opcode::PutGet, 0));
    let mut expected = vec![7, 59, 0xF1, SBA, 6, 53];
    expected.extend(ebcdic("QSECOFR   "));
    expected.extend([SBA, 7, 53]);
    expected.extend(ebcdic("secret    "));
    assert_eq!(data, expected);
    assert_eq!(s.keyboard(), KeyboardState::Locked);
}

#[test]
fn a_session_never_signs_on_by_itself_r_net_7() {
    // The sign-on screen is left to the operator: nothing goes to the host until a key does.
    let mut s = signed_on();
    assert_eq!(sent(&mut s), Vec::<Vec<u8>>::new());
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
}

#[test]
fn keep_alive_queues_a_telnet_no_operation() {
    // R-NET-5
    let mut s = session();
    s.keep_alive();
    assert_eq!(s.take_outbound(), [IAC, 241]);
}

#[test]
fn keys_are_ignored_while_the_keyboard_is_locked() {
    let mut s = signed_on();
    s.key(Key::Aid(Aid::ENTER));
    sent(&mut s);
    keys(&mut s, "abc");
    s.key(Key::Aid(Aid::ENTER));
    assert_eq!(s.take_outbound(), Vec::<u8>::new());
}

#[test]
fn typing_in_a_protected_area_is_ignored_and_tab_moves_on_r_key_8() {
    // D18: no operator error and no Reset; the screen is left as it was.
    let mut s = signed_on();
    let before = s.screen().text(s.code_page());
    s.key(Key::MoveTo { row: 0, column: 0 });
    for key in [
        Key::Char('x'),
        Key::Char('æ'),
        Key::Delete,
        Key::Backspace,
        Key::EraseEof,
        Key::FieldExit,
    ] {
        s.key(key);
        assert_eq!(s.keyboard(), KeyboardState::Unlocked, "{key:?}");
    }
    assert_eq!(s.screen().text(s.code_page()), before);
    s.key(Key::Tab);
    assert_ne!(s.screen().cursor, 0, "Tab goes to the next input field");
    s.key(Key::Char('a'));
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
}

#[test]
fn danish_letters_are_typed_and_sent_in_ccsid_277() {
    // R-EBC-1
    let mut s = Session::new(SessionConfig {
        code_page: CodePage::Ccsid277,
        ..SessionConfig::default()
    });
    s.receive(&host(&sign_on()));
    keys(&mut s, "Æblegrød");
    assert!(
        row_text(&s, 6).ends_with("ÆBLEGRØD"),
        "monocase: {}",
        row_text(&s, 6)
    );
    s.key(Key::Aid(Aid::ENTER));
    let (_, _, data) = sent_one(&mut s);
    assert_eq!(
        &data[6..14],
        &[0x7B, 0xC2, 0xD3, 0xC5, 0xC7, 0xD9, 0x7C, 0xC4]
    );
}

#[test]
fn field_exit_clears_the_rest_of_the_field_and_moves_on() {
    let mut s = signed_on();
    keys(&mut s, "abcdef");
    s.key(Key::MoveTo { row: 5, column: 54 });
    s.key(Key::FieldExit);
    assert_eq!(s.screen().cursor, address(&s, 7, 53));
    assert!(row_text(&s, 6).ends_with("AB"));
}

#[test]
fn backspace_deletes_left_and_delete_shifts_the_field() {
    let mut s = signed_on();
    keys(&mut s, "abcd");
    s.key(Key::Backspace);
    assert!(row_text(&s, 6).ends_with("ABC"));
    s.key(Key::Home);
    s.key(Key::Delete);
    assert!(row_text(&s, 6).ends_with("BC"));
}

#[test]
fn insert_mode_shifts_right_and_refuses_a_full_field() {
    let mut s = signed_on();
    keys(&mut s, "bc");
    s.key(Key::Home);
    s.key(Key::ToggleInsert);
    s.key(Key::Char('a'));
    assert!(row_text(&s, 6).ends_with("ABC"));
    keys(&mut s, "1234567");
    s.key(Key::Home);
    s.key(Key::Char('x'));
    assert_eq!(
        s.keyboard(),
        KeyboardState::Error(OperatorError::NoRoomToInsert)
    );
}

#[test]
fn end_goes_after_the_last_character() {
    let mut s = signed_on();
    keys(&mut s, "abc");
    s.key(Key::Home);
    s.key(Key::End);
    assert_eq!(s.screen().cursor, address(&s, 6, 56));
}

/// Fields: digits only (row 2), signed numeric (row 3), numeric only with zero fill (row 4).
fn numeric_screen() -> Session {
    let mut o = vec![SBA, 2, 10];
    o.extend(field([0x45, 0x00], 0x24, 4));
    o.extend([SBA, 3, 10]);
    o.extend(field([0x47, 0x00], 0x24, 6));
    o.extend([SBA, 4, 10]);
    o.extend(field([0x43, 0x05], 0x24, 4));
    let mut s = session();
    s.receive(&host(&screen_with(&o)));
    s
}

#[test]
fn digits_only_fields_refuse_letters() {
    let mut s = numeric_screen();
    s.key(Key::Char('a'));
    assert_eq!(
        s.keyboard(),
        KeyboardState::Error(OperatorError::DigitsOnly)
    );
}

#[test]
fn a_signed_numeric_field_right_adjusts_and_sends_a_negative_zone() {
    let mut s = numeric_screen();
    s.key(Key::Tab);
    keys(&mut s, "12-");
    assert_eq!(row_text(&s, 3), "             12-");
    assert_eq!(
        s.screen().cursor,
        address(&s, 4, 11),
        "Field- went on to the next field"
    );
    s.key(Key::Aid(Aid::ENTER));
    let (_, _, data) = sent_one(&mut s);
    assert_eq!(&data[3..], &[SBA, 3, 11, 0x40, 0x40, 0x40, 0xF1, 0xD2]);
}

#[test]
fn field_exit_zero_fills_and_field_minus_makes_the_last_digit_negative() {
    let mut s = numeric_screen();
    s.key(Key::MoveTo { row: 3, column: 10 });
    s.key(Key::Char('7'));
    s.key(Key::FieldMinus);
    assert_eq!(row_text(&s, 4), "          000P", "X'D7' shows as P");
    s.key(Key::MoveTo { row: 3, column: 10 });
    s.key(Key::Char('5'));
    s.key(Key::FieldExit);
    assert_eq!(row_text(&s, 4), "          0005");
}

#[test]
fn field_minus_is_refused_in_an_alphanumeric_field() {
    let mut s = signed_on();
    s.key(Key::FieldMinus);
    assert_eq!(
        s.keyboard(),
        KeyboardState::Error(OperatorError::FieldMinusNotAllowed)
    );
}

#[test]
fn an_auto_enter_field_sends_enter_when_it_is_full() {
    let mut o = vec![SBA, 2, 10];
    o.extend(field([0x40, 0x80], 0x24, 2));
    let mut s = session();
    s.receive(&host(&screen_with(&o)));
    keys(&mut s, "42");
    let (_, _, data) = sent_one(&mut s);
    assert_eq!(&data[..3], &[2, 12, 0xF1]);
}

#[test]
fn control_character_1_can_null_modified_fields_and_lock_the_keyboard() {
    let mut s = signed_on();
    keys(&mut s, "abc");
    s.receive(&host(&[ESC, 0x11, 0x80, 0x00]));
    assert_eq!(s.keyboard(), KeyboardState::Locked);
    assert_eq!(row_text(&s, 6), "                User . . .");
}

#[test]
fn repeat_to_address_and_erase_to_address_fill_ranges() {
    // R-DS-1
    let mut s = session();
    s.receive(&host(&screen_with(&[
        SBA, 2, 1, 0x02, 2, 10, 0xC1, SBA, 2, 3, 0x03, 2, 5, 0x02, 0xFF,
    ])));
    assert_eq!(row_text(&s, 2), "AA   AAAAA");
}

#[test]
fn writing_past_the_last_position_wraps_to_the_first() {
    let mut s = session();
    let mut o = vec![SBA, 24, 79];
    o.extend(ebcdic("ABC"));
    s.receive(&host(&screen_with(&o)));
    assert_eq!(row_text(&s, 24).trim(), "AB");
    assert_eq!(row_text(&s, 1), "C");
}

#[test]
fn write_error_code_shows_a_message_until_reset_puts_the_line_back() {
    let mut s = signed_on();
    let mut data = vec![ESC, 0x21, 0x22];
    data.extend(ebcdic("Value not valid"));
    s.receive(&host(&data));
    assert_eq!(row_text(&s, 24), " Value not valid");
    assert_eq!(s.keyboard(), KeyboardState::Error(OperatorError::Host));
    s.key(Key::Reset);
    assert_eq!(row_text(&s, 24), "");
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
}

#[test]
fn a_saved_screen_comes_back_with_its_fields_and_pending_read() {
    let mut s = signed_on();
    s.receive(&host_record(Opcode::SaveScreen, &[]));
    let (opcode, _, restore_data) = sent_one(&mut s);
    assert_eq!(opcode, Opcode::SaveScreen);
    assert_eq!(&restore_data[..2], &[ESC, 0x12]);

    s.receive(&host(&[ESC, 0x40]));
    assert_eq!(row_text(&s, 1), "");
    s.receive(&host_record(Opcode::RestoreScreen, &restore_data));
    assert_eq!(row_text(&s, 1).trim(), "Sign On");
    assert_eq!(s.screen().fields().len(), 2);
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
    assert_eq!(s.notice(), None);
}

#[test]
fn restore_data_from_elsewhere_is_reported_not_applied() {
    let mut s = signed_on();
    s.receive(&host_record(
        Opcode::RestoreScreen,
        &[ESC, 0x12, 1, 2, 3, 4],
    ));
    assert!(s.notice().is_some_and(|n| n.contains("did not save")));
    assert_eq!(row_text(&s, 1).trim(), "Sign On");
}

/// Save Partial Screen of rows 5-10, columns 10-49, then a pop-up written over the screen.
fn save_partial_then_popup() -> Vec<u8> {
    let mut data = vec![ESC, 0x03, 0x00, 5, 10, 6, 40];
    data.extend([ESC, 0x11, 0x00, 0x00, SBA, 1, 2]);
    data.extend(ebcdic("Popup"));
    data
}

#[test]
fn save_partial_screen_replies_after_the_rest_of_the_record_with_restore_partial_data_r_ds_6() {
    let mut s = signed_on();
    s.receive(&host(&save_partial_then_popup()));
    assert_eq!(s.notice(), None);
    assert!(row_text(&s, 1).starts_with(" Popup"), "the record went on");
    let (opcode, _, data) = sent_one(&mut s);
    assert_eq!(opcode, Opcode::SaveScreen);
    // ESC, Restore Partial Screen, a length counting itself, then Parley's id (D7).
    assert_eq!(&data[..6], &[ESC, 0x13, 0x00, 0x06, 0xD7, 0xE8]);
    assert_eq!(data.len(), 8);
}

#[test]
fn restore_partial_screen_puts_the_screen_back_and_goes_on_with_the_record_r_ds_6() {
    let mut s = signed_on();
    s.receive(&host(&save_partial_then_popup()));
    let (_, _, saved) = sent_one(&mut s);
    // The host echoes the data, then reads at once in the same record.
    let mut restore = saved.clone();
    restore.extend([ESC, 0x72]);
    s.receive(&host(&restore));
    assert_eq!(s.notice(), None);
    assert_eq!(row_text(&s, 1).trim(), "Sign On");
    assert_eq!(s.screen().fields().len(), 2);
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
    assert_eq!(
        sent(&mut s).len(),
        1,
        "the Read Immediate after it was answered"
    );
}

#[test]
fn restore_partial_data_parley_did_not_make_or_cut_short_is_reported_r_ds_6() {
    let mut s = signed_on();
    s.receive(&host(&[ESC, 0x13, 0x00, 0x06, 1, 2, 3, 4]));
    assert!(
        s.notice().is_some_and(|n| n.contains("did not save")),
        "{:?}",
        s.notice()
    );
    let mut s = signed_on();
    s.receive(&host(&[ESC, 0x13, 0x00, 0x20, 0xD7, 0xE8]));
    assert!(s.notice().is_some(), "a length past the record");
    let mut s = signed_on();
    s.receive(&host(&[ESC, 0x13, 0x00]));
    assert!(s.notice().is_some(), "a truncated length");
    assert_eq!(row_text(&s, 1).trim(), "Sign On");
}

#[test]
fn the_query_reply_claims_read_mdt_immediate_alternate_r_ds_7() {
    let mut s = session();
    s.receive(&host(&[ESC, 0xF3, 0x00, 0x05, 0xD9, 0x70, 0x00]));
    assert_eq!(
        sent_one(&mut s).2[49],
        0x01,
        "device capability byte 0, bit 7"
    );
}

#[test]
fn read_mdt_immediate_alternate_answers_at_once_with_cursor_aid_00_and_modified_fields_r_ds_7() {
    let mut s = signed_on();
    assert_eq!(s.screen().row_column(s.screen().cursor), (5, 52));
    s.receive(&host(&[ESC, 0x83]));
    assert_eq!(sent_one(&mut s).2, [6, 53, 0x00], "nothing modified");

    keys(&mut s, "ab");
    s.receive(&host(&[ESC, 0x83]));
    let mut expected = vec![6, 55, 0x00, SBA, 6, 53];
    expected.extend(ebcdic("AB")); // trailing nulls left out
    assert_eq!(sent_one(&mut s).2, expected);
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
}

/// The sign-on screen with its Start of Header's command key switches set to `switches`, or
/// with a Start of Header of `len` bytes.
fn sign_on_with_header(len: u8, switches: [u8; 3]) -> Session {
    let mut data = sign_on();
    let soh = data
        .windows(9)
        .position(|w| w == [0x01, 0x07, 0, 0, 0, 24, 0, 0, 0])
        .expect("the Start of Header");
    let mut header = vec![
        0x01,
        len,
        0,
        0,
        0,
        24,
        switches[0],
        switches[1],
        switches[2],
    ];
    header.truncate(2 + usize::from(len));
    data.splice(soh..soh + 9, header);
    let mut s = session();
    s.receive(&host(&data));
    assert_eq!(s.notice(), None);
    s
}

fn press(s: &mut Session, n: u8) -> Vec<u8> {
    s.key(Key::Aid(Aid::function(n).expect("F1-F24")));
    sent_one(s).2
}

#[test]
fn a_function_key_switched_off_in_the_header_sends_only_cursor_and_aid_r_ds_8() {
    // F3 (byte 7 bit 5) and F24 (byte 5 bit 0).
    let mut s = sign_on_with_header(7, [0x80, 0x00, 0x04]);
    keys(&mut s, "ab");
    assert_eq!(press(&mut s, 3), [6, 55, 0x33]);
    assert_eq!(
        s.keyboard(),
        KeyboardState::Locked,
        "it still waits for the host"
    );

    let mut s = sign_on_with_header(7, [0x80, 0x00, 0x04]);
    keys(&mut s, "ab");
    assert_eq!(press(&mut s, 24), [6, 55, 0xBC]);

    let mut s = sign_on_with_header(7, [0x80, 0x00, 0x04]);
    keys(&mut s, "ab");
    let f4 = press(&mut s, 4);
    assert_eq!(&f4[..6], &[6, 55, 0x34, SBA, 6, 53], "F4 sends its data");
}

#[test]
fn a_header_without_the_switches_lets_every_key_send_data_r_ds_8() {
    let mut s = sign_on_with_header(6, [0xFF, 0xFF, 0xFF]);
    keys(&mut s, "ab");
    assert!(press(&mut s, 3).len() > 3);
}

#[test]
fn clear_format_table_switches_the_keys_back_on_and_restore_screen_keeps_them_r_ds_8() {
    let mut s = sign_on_with_header(7, [0x00, 0x00, 0x04]);
    s.receive(&host_record(Opcode::SaveScreen, &[]));
    let (_, _, saved) = sent_one(&mut s);
    s.receive(&host(&[ESC, 0x50]));
    s.receive(&host_record(Opcode::RestoreScreen, &saved));
    keys(&mut s, "ab");
    assert_eq!(press(&mut s, 3), [6, 55, 0x33], "restored with the screen");

    let mut s = sign_on_with_header(7, [0x00, 0x00, 0x04]);
    s.receive(&host(&[ESC, 0x50, ESC, 0x52, 0x00, 0x00]));
    assert!(
        s.screen().function_key_sends_data(3),
        "the switches went with the table"
    );
}

#[test]
fn roll_up_moves_rows_and_blanks_the_bottom() {
    let mut s = session();
    let mut o = Vec::new();
    for (row, text) in [(1, "one"), (2, "two"), (3, "three")] {
        o.extend([SBA, row, 1]);
        o.extend(ebcdic(text));
    }
    s.receive(&host(&screen_with(&o)));
    s.receive(&host(&[ESC, 0x23, 0x01, 1, 3]));
    assert_eq!(
        (row_text(&s, 1), row_text(&s, 2), row_text(&s, 3)),
        ("two".into(), "three".into(), String::new())
    );
    s.receive(&host(&[ESC, 0x23, 0x82, 1, 3]));
    assert_eq!(
        (row_text(&s, 1), row_text(&s, 3)),
        (String::new(), "two".into())
    );
}

#[test]
fn the_query_is_answered_with_a_3179_reply() {
    let mut s = session();
    s.receive(&host(&[ESC, 0xF3, 0x00, 0x05, 0xD9, 0x70, 0x00]));
    let (opcode, _, data) = sent_one(&mut s);
    assert_eq!(opcode, Opcode::NoOperation);
    assert_eq!(data.len(), 61);
    assert_eq!(&data[2..7], &[0x88, 0x00, 0x3A, 0xD9, 0x70]);
    assert_eq!(&data[30..37], ebcdic("3179002").as_slice());
}

/// R-DS-5: the query reply's data field as SC30-3533-02 table 12-39 lays it out (it starts
/// at byte 8 of the record data): the model is 3 bytes, so the device capabilities start at
/// data-field byte 41, and byte 1 of them holds the screen sizes and colour.
#[test]
fn the_query_reply_puts_the_screen_sizes_in_device_capability_byte_1_r_ds_5() {
    let query = host(&[ESC, 0xF3, 0x00, 0x05, 0xD9, 0x70, 0x00]);
    let mut standard = session();
    standard.receive(&query);
    let (_, _, data) = sent_one(&mut standard);
    assert_eq!(&data[44..46], &[0x01, 0x00], "256 input fields");
    // Byte 0 bit 7: Read MDT Immediate Alternate (R-DS-7).
    assert_eq!(&data[49..51], &[0x01, 0x11], "24×80 and colour");

    let mut wide = Session::new(SessionConfig {
        screen_size: ScreenSize::Wide,
        ..SessionConfig::default()
    });
    wide.receive(&query);
    let (_, _, data) = sent_one(&mut wide);
    assert_eq!(data.len(), 61);
    assert_eq!(&data[30..37], ebcdic("3477FC ").as_slice());
    assert_eq!(&data[49..51], &[0x01, 0x31], "27×132, 24×80 and colour");
}

/// R-DS-5: SC30-3533-02, "Start of Header Order": a length of 0 is a parameter error.
#[test]
fn a_start_of_header_with_length_0_is_reported_r_ds_5() {
    let mut s = session();
    s.receive(&host(&screen_with(&[0x01, 0x00])));
    assert!(
        s.notice().is_some_and(|n| n.contains("Start of Header")),
        "{:?}",
        s.notice()
    );
}

/// R-DS-5: SC30-3533-02, "READ MDT ALTERNATE Command": leading and embedded nulls stay nulls,
/// trailing nulls are stripped, and a modified field of only nulls sends just its address.
#[test]
fn read_mdt_alternate_strips_trailing_nulls_and_keeps_the_others_r_ds_5() {
    let mut orders = vec![SBA, 3, 9];
    orders.extend(field([0x40, 0x00], 0x20, 6));
    orders.extend([SBA, 4, 9]);
    orders.extend(field([0x48, 0x00], 0x20, 4)); // MDT set by the host
    orders.extend([0x13, 3, 11]); // Insert Cursor on the field's second position
    let mut data = vec![ESC, 0x40, ESC, 0x11, 0x00, 0x00];
    data.extend(orders);
    data.extend([ESC, 0x82, 0x00, 0x00]);
    let mut s = session();
    s.receive(&host(&data));
    keys(&mut s, "æ");
    s.key(Key::Aid(Aid::ENTER));
    let (_, _, sent) = sent_one(&mut s);
    let mut expected = vec![SBA, 3, 10, 0x00];
    expected.extend(ebcdic("æ"));
    expected.extend([SBA, 4, 10]);
    assert_eq!(&sent[3..], expected.as_slice());
}

/// R-DS-5: SC30-3533-02, "READ INPUT FIELDS Command" (which Read MDT Fields follows): null
/// positions are sent as blanks, the whole length of the field.
#[test]
fn read_mdt_fields_sends_nulls_as_blanks_r_ds_5() {
    let mut orders = vec![SBA, 3, 9];
    orders.extend(field([0x40, 0x00], 0x20, 4));
    orders.extend([0x13, 3, 11]);
    let mut s = session();
    s.receive(&host(&screen_with(&orders)));
    keys(&mut s, "x");
    s.key(Key::Aid(Aid::ENTER));
    let (_, _, sent) = sent_one(&mut s);
    let mut expected = vec![SBA, 3, 10, 0x40];
    expected.extend(ebcdic("x"));
    expected.extend([0x40, 0x40]);
    assert_eq!(&sent[3..], expected.as_slice());
}

#[test]
fn a_wide_session_switches_to_27_by_132_and_a_standard_one_reports_it() {
    let mut wide = Session::new(SessionConfig {
        screen_size: ScreenSize::Wide,
        ..SessionConfig::default()
    });
    wide.receive(&host(&[ESC, 0x20, 0x00]));
    assert_eq!(wide.screen().size(), ScreenSize::Wide);

    let mut standard = session();
    standard.receive(&host(&[ESC, 0x20, 0x00]));
    assert_eq!(standard.screen().size(), ScreenSize::Standard);
    assert!(standard.notice().is_some_and(|n| n.contains("27×132")));
}

#[test]
fn negotiation_replies_go_out_and_the_code_page_is_announced() {
    // R-NET-1, R-NET-3
    let mut s = Session::new(SessionConfig {
        code_page: CodePage::Ccsid277,
        ..SessionConfig::default()
    });
    s.receive(&[IAC, 253, 24, IAC, 253, 39, IAC, 250, 39, 1, IAC, 240]);
    let out = s.take_outbound();
    assert!(out.starts_with(&[IAC, 251, 24, IAC, 251, 39]));
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("CODEPAGE\u{1}277") && text.contains("KBDTYPE\u{1}DMB"),
        "{text}"
    );
}

#[test]
fn read_immediate_sends_fields_without_cursor_or_aid() {
    let mut s = signed_on();
    s.receive(&host(&[ESC, 0x72]));
    assert_eq!(sent_one(&mut s).2, Vec::<u8>::new(), "nothing modified");
    keys(&mut s, "ab");
    s.receive(&host(&[ESC, 0x72]));
    let mut expected = ebcdic("AB        ");
    expected.extend([0x40; 10]);
    assert_eq!(sent_one(&mut s).2, expected);
}

#[test]
fn attention_is_sent_even_while_the_keyboard_is_locked() {
    let mut s = session();
    s.key(Key::Attention);
    let (opcode, flags, data) = sent_one(&mut s);
    assert_eq!((opcode, flags, data.len()), (Opcode::NoOperation, 0x40, 0));
}

#[test]
fn a_bad_record_is_reported_and_the_next_one_still_works() {
    let mut s = session();
    s.receive(&host(&[ESC, 0x11, 0x00, 0x00, SBA, 30, 1]));
    assert!(
        s.notice().is_some_and(|n| n.contains("outside the screen")),
        "{:?}",
        s.notice()
    );
    s.receive(&host(&[ESC, 0x11, 0x00, 0x00, SBA, 2]));
    assert!(s.notice().is_some_and(|n| n.contains("ends inside")));
    s.receive(&host(&sign_on()));
    assert_eq!(row_text(&s, 1).trim(), "Sign On");
}

#[test]
fn random_host_data_never_panics() {
    // Host data is untrusted (CLAUDE.md): any bytes give a notice at worst.
    let mut seed: u32 = 0x5250_5250;
    let mut random = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for wide in [false, true] {
        let size = if wide {
            ScreenSize::Wide
        } else {
            ScreenSize::Standard
        };
        let mut s = Session::new(SessionConfig {
            screen_size: size,
            ..SessionConfig::default()
        });
        for _ in 0..3000 {
            let len = (random() % 40) as usize;
            let mut data = vec![
                ESC,
                [0x40, 0x20, 0x11, 0x21, 0x52, 0x23, 0x12, 0xF3, 0x72][(random() % 9) as usize],
            ];
            data.extend((0..len).map(|_| random().to_le_bytes()[0]));
            s.receive(&host(&data));
            for key in [
                Key::Char('1'),
                Key::FieldMinus,
                Key::Delete,
                Key::End,
                Key::Aid(Aid::ENTER),
                Key::Reset,
            ] {
                s.key(key);
            }
            s.take_outbound();
        }
    }
}

// Macros (R-KEY-3, R-KEY-4).

/// A screen after sign-on: one input field on row 5, waiting for Enter.
fn menu_screen() -> Vec<u8> {
    let mut o = vec![SBA, 5, 9];
    o.extend(field([0x40, 0x00], 0x24, 10));
    o.extend([0x13, 5, 10]);
    screen_with(&o)
}

fn enter() -> Step {
    Step::Key(Key::Aid(Aid::ENTER))
}

#[test]
fn recording_keeps_text_keys_and_a_wait_after_enter_but_not_the_password_r_key_3() {
    let mut s = signed_on();
    s.start_recording();
    assert_eq!(s.macro_state(), MacroState::Recording);
    keys(&mut s, "quser");
    s.key(Key::Tab);
    keys(&mut s, "secret");
    s.key(Key::Backspace);
    s.key(Key::MoveTo { row: 0, column: 0 }); // a click: not recorded
    s.key(Key::Aid(Aid::ENTER));
    let recorded = s.stop_recording().expect("was recording");
    assert_eq!(
        recorded.steps,
        [
            Step::Text("quser".into()),
            Step::Key(Key::Tab),
            Step::Password,
            enter(),
            Step::Wait
        ]
    );
    assert!(!recorded.to_text().contains("secret"));
    assert_eq!(s.macro_state(), MacroState::Off);
}

#[test]
fn a_hotspot_s_move_to_its_option_field_is_recorded_r_ui_20() {
    let mut s = signed_on();
    s.start_recording();
    s.hotspot_keys(&[
        Key::MoveTo { row: 4, column: 9 },
        Key::EraseEof,
        Key::Char('2'),
    ]);
    assert_eq!(
        s.stop_recording().expect("was recording").steps,
        [
            Step::Key(Key::MoveTo { row: 4, column: 9 }),
            Step::Key(Key::EraseEof),
            Step::Text("2".into())
        ]
    );
}

#[test]
fn an_operator_error_has_the_standard_5250_code_r_key_9() {
    let mut s = numeric_screen();
    s.key(Key::Char('a')); // digits only
    let KeyboardState::Error(error) = s.keyboard() else {
        panic!("an error, got {:?}", s.keyboard());
    };
    assert_eq!(error.code(), Some("0010"));
    assert_eq!(error.display(), "0010 Only digits are allowed here.");
    assert_eq!(OperatorError::NoRoomToInsert.code(), Some("0012"));
    assert_eq!(OperatorError::Host.code(), None);
    assert_eq!(
        OperatorError::NotInCodePage.display(),
        "That character is not in the session's code page."
    );
}

/// R-KEY-10: a mandatory enter field at row 5, a mandatory fill field (5 long) at row 6, a
/// digits-only modulus 10 self-check field (2 long) at row 7, and a plain field at row 8.
fn checked_screen() -> Session {
    let mut o = vec![SBA, 5, 9];
    o.extend(field([0x40, 0x08], 0x24, 5)); // FFW bit 12: mandatory enter
    o.extend([SBA, 6, 9]);
    o.extend(field([0x40, 0x07], 0x24, 5)); // FFW bits 13-15 = 111: mandatory fill
    o.extend([SBA, 7, 9]);
    o.extend([0x1D, 0x45, 0x00, 0xB1, 0xA0, 0x24, 0x00, 2]); // digits only, FCW modulus 10
    o.extend([SBA, 8, 9]);
    o.extend(field([0x40, 0x00], 0x24, 5));
    o.extend([0x13, 8, 10]); // Insert Cursor: the plain field
    let mut s = session();
    s.receive(&host(&screen_with(&o)));
    assert_eq!(s.notice(), None);
    s
}

fn error_code(s: &Session) -> Option<&'static str> {
    match s.keyboard() {
        KeyboardState::Error(e) => e.code(),
        KeyboardState::Unlocked | KeyboardState::Locked => None,
    }
}

fn cursor(s: &Session) -> (usize, usize) {
    s.screen().row_column(s.screen().cursor)
}

#[test]
fn enter_with_a_mandatory_enter_field_empty_is_0007_and_goes_to_it_r_key_10() {
    let mut s = checked_screen();
    keys(&mut s, "x");
    s.key(Key::Aid(Aid::ENTER));
    assert_eq!(error_code(&s), Some("0007"));
    assert_eq!(cursor(&s), (4, 9), "on the mandatory enter field");
    assert!(sent(&mut s).is_empty(), "nothing goes to the host");
    s.key(Key::Char('y'));
    assert_eq!(error_code(&s), Some("0007"), "keys wait for Reset");
    s.key(Key::Reset);
    keys(&mut s, "a");
    s.key(Key::Aid(Aid::ENTER));
    assert_eq!(s.keyboard(), KeyboardState::Locked, "now it is sent");
}

#[test]
fn with_nothing_typed_on_the_screen_mandatory_enter_is_not_enforced_r_key_10() {
    let mut s = checked_screen();
    s.key(Key::Aid(Aid::ENTER));
    assert_eq!(s.keyboard(), KeyboardState::Locked);
}

#[test]
fn field_exit_past_an_empty_mandatory_enter_field_is_0021_r_key_10() {
    let mut s = checked_screen();
    s.key(Key::MoveTo { row: 4, column: 9 });
    s.key(Key::FieldExit);
    assert_eq!(error_code(&s), Some("0021"));
    assert_eq!(cursor(&s), (4, 9));
}

#[test]
fn a_half_filled_mandatory_fill_field_cannot_be_left_r_key_10() {
    let mut s = checked_screen();
    s.key(Key::MoveTo { row: 5, column: 9 });
    keys(&mut s, "ab");
    s.key(Key::Tab);
    assert_eq!(error_code(&s), Some("0014"));
    s.key(Key::Reset);
    s.key(Key::FieldExit);
    assert_eq!(error_code(&s), Some("0017"));
    s.key(Key::Reset);
    keys(&mut s, "cde"); // filled: the cursor moves on by itself
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
    assert_eq!(cursor(&s), (6, 9));
}

#[test]
fn field_exit_from_the_first_position_empties_a_mandatory_fill_field_r_key_10() {
    let mut s = checked_screen();
    s.key(Key::MoveTo { row: 5, column: 9 });
    keys(&mut s, "ab");
    s.key(Key::MoveTo { row: 5, column: 9 });
    s.key(Key::FieldExit);
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
    assert_eq!(row_text(&s, 6), "");
}

#[test]
fn a_wrong_check_digit_is_0015_and_the_cursor_goes_back_to_the_field_r_key_10() {
    let mut s = checked_screen();
    s.key(Key::MoveTo { row: 6, column: 9 });
    keys(&mut s, "58"); // 5 doubled is 10, 1 + 0 = 1: the check digit is 9
    assert_eq!(error_code(&s), Some("0015"));
    assert_eq!(cursor(&s), (6, 9));
    s.key(Key::Reset);
    keys(&mut s, "59");
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
    assert_eq!(cursor(&s), (7, 9), "a good number moves on");
}

/// Signed on with type-ahead on, Enter pressed: the keyboard waits for the host (R-KEY-11).
fn waiting_for_the_host() -> Session {
    let mut s = signed_on();
    s.set_type_ahead(true);
    s.key(Key::Aid(Aid::ENTER));
    assert_eq!(s.keyboard(), KeyboardState::Locked);
    s.take_outbound();
    s
}

#[test]
fn without_type_ahead_keys_while_the_host_works_are_dropped_r_key_11() {
    let mut s = signed_on();
    s.key(Key::Aid(Aid::ENTER));
    keys(&mut s, "yz");
    assert_eq!(s.typed_ahead(), 0);
    s.receive(&host(&menu_screen()));
    assert_eq!(row_text(&s, 5), "");
}

#[test]
fn keys_typed_ahead_are_carried_out_when_the_host_unlocks_r_key_11() {
    let mut s = waiting_for_the_host();
    keys(&mut s, "yzæ");
    assert_eq!(s.typed_ahead(), 3);
    s.receive(&host(&menu_screen()));
    assert_eq!(s.typed_ahead(), 0);
    assert_eq!(row_text(&s, 5).trim(), "yzæ");
}

#[test]
fn a_key_typed_ahead_that_waits_for_the_host_leaves_the_rest_for_the_next_screen_r_key_11() {
    let mut s = waiting_for_the_host();
    keys(&mut s, "a");
    s.key(Key::Aid(Aid::ENTER));
    keys(&mut s, "b");
    s.receive(&host(&menu_screen()));
    assert_eq!(
        s.keyboard(),
        KeyboardState::Locked,
        "the Enter typed ahead went"
    );
    assert_eq!(s.typed_ahead(), 1);
    assert_eq!(sent(&mut s).len(), 1);
    s.receive(&host(&menu_screen()));
    assert_eq!(row_text(&s, 5).trim(), "b");
}

#[test]
fn reset_drops_the_keys_typed_ahead_r_key_11() {
    let mut s = waiting_for_the_host();
    keys(&mut s, "yz");
    s.key(Key::Reset);
    assert_eq!(s.typed_ahead(), 0);
    s.receive(&host(&menu_screen()));
    assert_eq!(row_text(&s, 5), "");
}

#[test]
fn an_error_while_replaying_drops_the_rest_r_key_11() {
    let mut s = numeric_screen();
    s.set_type_ahead(true);
    s.key(Key::Aid(Aid::ENTER));
    keys(&mut s, "q1");
    let mut o = vec![SBA, 2, 10];
    o.extend(field([0x45, 0x00], 0x24, 4));
    s.receive(&host(&screen_with(&o)));
    assert!(matches!(s.keyboard(), KeyboardState::Error(_)));
    assert_eq!(s.typed_ahead(), 0, "the 1 after the refused q is dropped");
}

#[test]
fn type_ahead_keeps_at_most_its_limit_r_key_11() {
    let mut s = waiting_for_the_host();
    for _ in 0..parley_core::session::TYPE_AHEAD_KEYS + 10 {
        s.key(Key::Char('x'));
    }
    assert_eq!(s.typed_ahead(), parley_core::session::TYPE_AHEAD_KEYS);
    s.set_type_ahead(false);
    assert_eq!(s.typed_ahead(), 0, "turning it off drops them");
}

#[test]
fn write_extended_attribute_colours_text_without_taking_a_position_r_ds_9() {
    let mut o = vec![SBA, 3, 1, 0x12, 0x03, 0x88]; // red from row 3, column 1
    o.extend(ebcdic("red"));
    o.extend([0x12, 0x03, 0x80]); // back to the screen's colour
    o.extend(ebcdic("green"));
    o.extend([0x12, 0x05, 0x81]); // ideographic: not claimed, ignored
    let mut s = session();
    s.receive(&host(&screen_with(&o)));
    assert_eq!(s.notice(), None);
    assert_eq!(row_text(&s, 3), "redgreen", "no positions taken");
    let cells = s.screen().cells(CodePage::Ccsid37);
    let start = address(&s, 3, 1);
    assert_eq!(cells[start].style.color, parley_core::Color::Red);
    assert_eq!(cells[start + 3].style.color, parley_core::Color::Green);
}

#[test]
fn the_query_reply_claims_extended_attributes_and_seven_colours_r_ds_9() {
    let mut s = session();
    s.receive(&host(&[ESC, 0xF3, 0x00, 0x05, 0xD9, 0x70, 0x00]));
    assert_eq!(sent_one(&mut s).2[51], 0x05, "device capability byte 2");
}

fn gui_session(on: bool) -> Session {
    Session::new(SessionConfig {
        gui: on,
        ..SessionConfig::default()
    })
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

/// Write To Display: SBA 5,10, then a Create Window of 3 rows by 20 columns that keeps the
/// cursor inside, with a centred title, then an input field inside it.
fn window_screen() -> Vec<u8> {
    let mut title = vec![0x00, 0x10, 0x00, 0x00, 0x00, 0x00];
    title.extend(ebcdic("Æble"));
    title[0] = u8::try_from(title.len()).expect("short");
    let mut create = vec![0x80, 0x00, 0x00, 3, 20];
    create.extend(&title);
    let mut o = vec![SBA, 5, 10];
    o.extend(wdsf(0x51, &create));
    o.extend([SBA, 6, 13]);
    o.extend(field([0x40, 0x00], 0x24, 5));
    o.extend([0x13, 6, 14]);
    screen_with(&o)
}

#[test]
fn with_the_gui_switch_on_create_window_draws_a_window_r_ds_10() {
    let mut s = gui_session(true);
    s.receive(&host(&window_screen()));
    assert_eq!(s.notice(), None);
    assert_eq!(s.screen().windows().len(), 1);
    assert!(row_text(&s, 5).contains("Æble"), "{}", row_text(&s, 5));
    assert!(row_text(&s, 6).contains(':'));
    // The cursor stays inside: a move out of the window does not happen.
    s.key(Key::MoveTo { row: 0, column: 0 });
    assert_eq!(s.screen().row_column(s.screen().cursor), (5, 13));
}

#[test]
fn with_the_gui_switch_off_create_window_is_skipped_r_ds_10() {
    let mut s = gui_session(false);
    s.receive(&host(&window_screen()));
    assert_eq!(s.notice(), None);
    assert_eq!(s.screen().windows().len(), 0);
    s.key(Key::MoveTo { row: 0, column: 0 });
    assert_eq!(s.screen().row_column(s.screen().cursor), (0, 0));
}

#[test]
fn remove_gui_window_and_clear_unit_take_the_window_away_r_ds_10() {
    let mut s = gui_session(true);
    s.receive(&host(&window_screen()));
    let mut remove = vec![ESC, 0x11, 0x00, 0x00, SBA, 5, 10];
    remove.extend(wdsf(0x59, &[0x00, 0x00, 0x00]));
    s.receive(&host(&remove));
    assert_eq!(s.notice(), None);
    assert_eq!(s.screen().windows().len(), 0);
    assert_eq!(s.screen().cursor_limit(), None);
    s.receive(&host(&window_screen()));
    s.receive(&host(&[ESC, 0x40]));
    assert_eq!(s.screen().windows().len(), 0);
}

#[test]
fn a_window_that_does_not_fit_is_a_bad_record_r_ds_10() {
    let mut s = gui_session(true);
    let mut o = vec![SBA, 23, 1];
    o.extend(wdsf(0x51, &[0x00, 0x00, 0x00, 3, 20]));
    s.receive(&host(&screen_with(&o)));
    assert!(
        s.notice().is_some_and(|n| n.contains("does not fit")),
        "{:?}",
        s.notice()
    );
}

#[test]
fn the_keep_alive_interval_defaults_to_a_minute_and_stays_in_range_r_net_11() {
    let config = |seconds| SessionConfig {
        keep_alive_seconds: seconds,
        ..SessionConfig::default()
    };
    assert_eq!(
        SessionConfig::default().keep_alive_interval(),
        Duration::from_secs(60)
    );
    assert_eq!(config(25).keep_alive_interval(), Duration::from_secs(25));
    assert_eq!(config(1).keep_alive_interval(), Duration::from_secs(10));
    assert_eq!(
        config(60_000).keep_alive_interval(),
        Duration::from_secs(3600)
    );
    let old: SessionConfig =
        serde_json::from_str(r#"{"host": "x", "keep_alive": true}"#).expect("reads");
    assert_eq!(
        old.keep_alive_interval(),
        Duration::from_secs(60),
        "saved before R-NET-11"
    );
}

#[test]
fn the_gui_switch_is_stored_and_old_sessions_read_as_off_d57() {
    let saved: SessionConfig =
        serde_json::from_str(r#"{"host": "dksoft00", "port": 23, "gui": true}"#).expect("reads");
    assert!(saved.gui);
    let old: SessionConfig = serde_json::from_str(r#"{"host": "dksoft00"}"#).expect("reads");
    assert!(!old.gui, "off by default");
    let stored = serde_json::to_string(gui_session(true).config()).expect("writes");
    assert!(stored.contains(r#""gui":true"#), "{stored}");
}

#[test]
fn the_query_reply_claims_the_gui_only_with_the_switch_on_r_ds_10() {
    let query = host(&[ESC, 0xF3, 0x00, 0x05, 0xD9, 0x70, 0x00]);
    let mut on = gui_session(true);
    on.receive(&query);
    let reply = sent_one(&mut on).2;
    assert_eq!(
        &reply[3..5],
        &[0x00, 0x40],
        "64 bytes, as tn5250j's enhanced mode"
    );
    assert_eq!(reply.len(), 3 + 0x40);
    assert_eq!(
        reply[53], 0x0F,
        "byte 4: pointer device (D57), GUI-like characters, level 1, error to window"
    );
    assert_eq!(reply[54], 0x88, "byte 5: levels 2 (D57) and 3");
    let mut off = gui_session(false);
    off.receive(&query);
    let reply = sent_one(&mut off).2;
    assert_eq!((reply.len(), reply[53]), (61, 0x00));
}

#[test]
fn recording_leaves_out_refused_keys_and_the_reset_after_them_r_key_3() {
    let mut s = numeric_screen();
    s.start_recording();
    s.key(Key::Char('a')); // digits only: refused
    s.key(Key::Reset);
    s.key(Key::Char('1'));
    s.key(Key::Aid(Aid::ENTER));
    s.key(Key::Char('2')); // the keyboard is locked until the host answers
    assert_eq!(
        s.stop_recording().expect("was recording").steps,
        [Step::Text("1".into()), enter(), Step::Wait]
    );
}

#[test]
fn a_macro_plays_waits_for_the_host_and_asks_for_a_password_r_key_4() {
    let mut s = signed_on();
    let steps = vec![
        Step::Text("quser".into()),
        Step::Key(Key::Tab),
        Step::Password,
        enter(),
        Step::Wait,
        Step::Text("wrkactjob".into()),
    ];
    s.play("start", steps);
    assert_eq!(s.macro_state(), MacroState::NeedsPassword);
    assert!(row_text(&s, 6).ends_with("QUSER"), "monocase user field");
    s.give_macro_password(Some(&Password::new("æøå".into())));
    assert_eq!(sent(&mut s).len(), 1, "Enter was sent");
    assert_eq!(s.macro_state(), MacroState::Playing);
    assert_eq!(s.playing(), Some("start"));
    s.receive(&host(&menu_screen()));
    assert_eq!(row_text(&s, 5), "         wrkactjob");
    assert_eq!(s.macro_state(), MacroState::Off, "the macro ended");
}

#[test]
fn reset_stops_a_running_macro_and_other_keys_are_ignored_r_key_4() {
    let mut s = session();
    s.receive(&host(&menu_screen()));
    s.play("m", vec![enter(), Step::Wait, Step::Text("x".into())]);
    s.key(Key::Char('y'));
    s.key(Key::Reset);
    assert_eq!(s.macro_state(), MacroState::Off);
    assert!(s.notice().is_some_and(|n| n.contains("\"m\" stopped")));
    s.receive(&host(&menu_screen()));
    assert_eq!(row_text(&s, 5), "", "nothing typed after the stop");
}

#[test]
fn a_macro_gives_up_after_30_seconds_without_an_answer_r_key_4() {
    let mut s = session();
    s.receive(&host(&menu_screen()));
    s.play("m", vec![enter(), Step::Wait, Step::Text("x".into())]);
    let start = Instant::now();
    assert!(!s.check_macro_time(start));
    assert!(!s.check_macro_time(start + Duration::from_secs(29)));
    assert!(s.check_macro_time(start + MACRO_TIMEOUT));
    assert_eq!(s.macro_state(), MacroState::Off);
    assert!(s.notice().is_some_and(|n| n.contains("within 30 s")));
}

#[test]
fn a_macro_stops_at_an_operator_error_r_key_4() {
    let mut s = numeric_screen();
    s.play("m", vec![Step::Text("1a2".into())]);
    assert_eq!(s.macro_state(), MacroState::Off);
    assert_eq!(
        s.keyboard(),
        KeyboardState::Error(OperatorError::DigitsOnly),
        "the error stays on screen"
    );
    assert!(s.notice().is_some_and(|n| n.contains("error")));
}

#[test]
fn a_macro_at_connect_starts_on_the_sign_on_screen_r_key_4() {
    // The macro may be the sign-on: it starts on the first screen that waits for input.
    let mut s = session();
    s.play(
        "sign on",
        vec![
            Step::Text("qsecofr".into()),
            Step::Key(Key::FieldExit),
            Step::Text("secret".into()),
            Step::Key(Key::Aid(Aid::ENTER)),
            Step::Wait,
        ],
    );
    s.receive(&host(&sign_on()));
    assert_eq!(
        row_text(&s, 6),
        "                User . . .                          QSECOFR"
    );
    assert_eq!(s.keyboard(), KeyboardState::Locked, "Enter was pressed");
    s.receive(&host(&menu_screen()));
    assert_eq!(s.macro_state(), MacroState::Off);
}

#[test]
fn end_in_a_one_position_signed_numeric_field_keeps_the_cursor_r_key_1() {
    // A signed numeric field of one position has only its sign position: End stays put.
    let mut o = vec![SBA, 2, 10];
    o.extend(field([0x47, 0x00], 0x24, 1));
    let mut s = session();
    s.receive(&host(&screen_with(&o)));
    let start = address(&s, 2, 11);
    assert_eq!(s.screen().cursor, start);
    s.key(Key::End);
    assert_eq!(s.screen().cursor, start);
}

// Keys on random screens: whatever fields the host defines, no key may panic.

/// Every key, with characters of each kind a field may refuse.
fn every_key(rows: usize, columns: usize) -> Vec<Key> {
    vec![
        Key::Char('1'),
        Key::Char('a'),
        Key::Char('-'),
        Key::Char('+'),
        Key::Char(' '),
        Key::Char('æ'),
        Key::Char('😀'),
        Key::Aid(Aid::ENTER),
        Key::Aid(Aid::function(24).expect("F24")),
        Key::FieldExit,
        Key::FieldPlus,
        Key::FieldMinus,
        Key::Tab,
        Key::BackTab,
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::Home,
        Key::End,
        Key::Backspace,
        Key::Delete,
        Key::ToggleInsert,
        Key::EraseEof,
        Key::Reset,
        Key::Attention,
        Key::SystemRequest,
        Key::MoveTo { row: 0, column: 0 },
        Key::MoveTo {
            row: rows - 1,
            column: columns - 1,
        },
        Key::MoveTo {
            row: rows,
            column: columns,
        },
        Key::MoveTo {
            row: usize::MAX,
            column: usize::MAX,
        },
    ]
}

#[test]
fn every_key_on_random_fields_near_the_screens_end_never_panics_r_key_1() {
    let mut seed: u32 = 0x1234_5678;
    // Below `n`, which is at most 256: it fits a byte.
    let mut random = move |n: u16| -> u8 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % u32::from(n)).to_le_bytes()[0]
    };
    for size in [ScreenSize::Standard, ScreenSize::Wide] {
        let mut s = Session::new(SessionConfig {
            screen_size: size,
            ..SessionConfig::default()
        });
        let rows = u8::try_from(size.rows()).expect("a screen has at most 27 rows");
        let columns = u8::try_from(size.columns()).expect("a screen has at most 132 columns");
        // Clear Unit Alternate picks the session's largest screen.
        let clear = if size == ScreenSize::Wide {
            vec![ESC, 0x20, 0x00]
        } else {
            vec![ESC, 0x40]
        };
        for _ in 0..300 {
            let mut data = clear.clone();
            data.extend([ESC, 0x11, 0x00, 0x00]);
            for _ in 0..=random(4) {
                // Most fields on the last row, near or at its end; some anywhere.
                let (row, column) = if random(4) == 0 {
                    (1 + random(rows.into()), 1 + random(columns.into()))
                } else {
                    (rows, columns - random(6))
                };
                let ffw0 = 0x40 | if random(5) == 0 { 0x20 } else { 0 } | random(8);
                let ffw1 = random(256);
                data.extend([SBA, row, column]);
                data.extend([0x1D, ffw0, ffw1, 0x24, 0x00, random(4)]);
            }
            data.extend(READ_MDT);
            s.receive(&host(&data));
            let starts: Vec<usize> = s.screen().fields().iter().map(|f| f.start).collect();
            let positions = s.screen().buffer().len();
            for start in starts.iter().chain(&[0, positions - 1]) {
                for insert in [false, true] {
                    for key in every_key(usize::from(rows), usize::from(columns)) {
                        let (row, column) = s.screen().row_column(*start + usize::from(random(3)));
                        s.key(Key::MoveTo { row, column });
                        if s.insert_mode() != insert {
                            s.key(Key::ToggleInsert);
                        }
                        s.key(key);
                        s.key(Key::Reset);
                        if s.keyboard() == KeyboardState::Locked {
                            s.receive(&host(&READ_MDT));
                        }
                        s.take_outbound();
                    }
                }
            }
        }
    }
}

// Commands and orders (R-DS-1) and what the host's data does to the keyboard and indicators.

#[test]
fn clear_format_table_removes_the_fields_and_locks_the_keyboard_r_ds_1() {
    let mut s = signed_on();
    s.receive(&host(&[ESC, 0x50]));
    assert_eq!(s.screen().fields().len(), 0);
    assert_eq!(s.keyboard(), KeyboardState::Locked);
    assert_eq!(row_text(&s, 1).trim(), "Sign On", "the text stays");
}

#[test]
fn write_error_code_to_window_shows_the_message_on_the_error_row_r_ds_1() {
    let mut s = signed_on();
    let mut data = vec![ESC, 0x22, 0x00, 0x00, 0x22];
    data.extend(ebcdic("æøå not valid"));
    s.receive(&host(&data));
    assert_eq!(row_text(&s, 24), " æøå not valid");
    assert_eq!(s.keyboard(), KeyboardState::Error(OperatorError::Host));
}

#[test]
fn start_of_header_moves_the_error_row_r_ds_1() {
    let mut s = session();
    s.receive(&host(&screen_with(&[0x01, 0x04, 0, 0, 0, 5])));
    let mut data = vec![ESC, 0x21, 0x22];
    data.extend(ebcdic("Wrong"));
    s.receive(&host(&data));
    assert_eq!(row_text(&s, 5), " Wrong");
    assert_eq!(row_text(&s, 24), "");
}

#[test]
fn a_start_of_header_longer_than_7_is_reported_r_ds_1() {
    let mut s = session();
    s.receive(&host(&screen_with(&[0x01, 0x08, 0, 0, 0, 0, 0, 0, 0, 0])));
    assert!(s.notice().is_some_and(|n| n.contains("Start of Header")));
}

/// Two alphanumeric fields, on rows 2 (5 positions) and 3 (3 positions), then `read`.
fn two_fields_then(read: &[u8]) -> Session {
    let mut data = vec![ESC, 0x40, ESC, 0x11, 0x00, 0x00, SBA, 2, 10];
    data.extend(field([0x40, 0x00], 0x24, 5));
    data.extend([SBA, 3, 10]);
    data.extend(field([0x40, 0x00], 0x24, 3));
    data.extend(read);
    let mut s = session();
    s.receive(&host(&data));
    s
}

#[test]
fn read_input_fields_sends_every_field_without_addresses_r_ds_3() {
    let mut s = two_fields_then(&[ESC, 0x42, 0x00, 0x00]);
    keys(&mut s, "ab");
    s.key(Key::Aid(Aid::ENTER));
    let (_, _, data) = sent_one(&mut s);
    let mut expected = ebcdic("ab");
    expected.extend([0x40; 6]);
    assert_eq!(&data[3..], expected.as_slice());
}

#[test]
fn read_input_fields_sends_no_fields_when_none_was_modified_r_ds_3() {
    let mut s = two_fields_then(&[ESC, 0x42, 0x00, 0x00]);
    s.key(Key::Aid(Aid::ENTER));
    let (_, _, data) = sent_one(&mut s);
    assert_eq!(data, [2, 11, Aid::ENTER.code()]);
}

#[test]
fn read_screen_immediate_sends_the_whole_buffer_r_ds_1() {
    let mut s = session();
    s.receive(&host(&screen_with(&ebcdic("Hi"))));
    s.take_outbound();
    s.receive(&host(&[ESC, 0x62]));
    let (opcode, _, data) = sent_one(&mut s);
    assert_eq!(opcode, Opcode::NoOperation);
    assert_eq!(data.len(), 24 * 80);
    assert_eq!(&data[..2], ebcdic("Hi").as_slice());
}

#[test]
fn transparent_data_is_written_as_it_is_even_when_it_looks_like_orders_r_ds_1() {
    let mut s = session();
    s.receive(&host(&screen_with(&[
        SBA, 1, 1, 0x10, 0x00, 0x03, 0xC1, 0x1D, 0xC2,
    ])));
    assert_eq!(&s.screen().buffer()[..3], &[0xC1, 0x1D, 0xC2]);
    assert_eq!(s.screen().fields().len(), 0);
    assert_eq!(s.notice(), None);
}

#[test]
fn a_start_of_field_without_a_format_word_writes_the_attribute_but_no_field_r_ds_1() {
    let mut s = session();
    s.receive(&host(&screen_with(&[SBA, 2, 10, 0x1D, 0x22, 0x00, 0x05])));
    assert_eq!(s.screen().fields().len(), 0);
    assert_eq!(s.screen().get(address(&s, 2, 10)), 0x22);
    assert_eq!(s.notice(), None);
}

#[test]
fn malformed_commands_and_orders_are_reported_not_applied_r_ds_1() {
    const WTD: [u8; 4] = [ESC, 0x11, 0x00, 0x00];
    let with_wtd = |orders: &[u8]| [&WTD[..], orders].concat();
    for (data, says) in [
        (vec![0x40], "expected a command"),
        (vec![ESC], "ends inside a command"),
        (vec![ESC, 0x99], "command X'99'"),
        (vec![ESC, 0x20, 0x00], "27×132"),
        (vec![ESC, 0x11, 0x00], "ends inside"),
        (with_wtd(&[SBA, 25, 1]), "outside the screen"),
        (with_wtd(&[SBA, 1, 0]), "outside the screen"),
        (
            with_wtd(&[SBA, 2, 1, 0x02, 1, 1, 0x40]),
            "Repeat to Address goes backwards",
        ),
        (with_wtd(&[0x03, 1, 5, 1]), "Erase to Address with length 1"),
        (with_wtd(&[0x03, 1, 5, 6]), "Erase to Address with length 6"),
        (
            with_wtd(&[SBA, 2, 1, 0x03, 1, 1, 2, 0xFF]),
            "Erase to Address goes backwards",
        ),
        (
            with_wtd(&[0x10, 0x00, 0x09, 0xC1]),
            "ends inside Transparent Data",
        ),
        (
            with_wtd(&[0x1D, 0x80, 0x00, 0x24, 0x00, 0x05]),
            "field format word",
        ),
        (
            with_wtd(&[0x1D, 0x40, 0x00, 0x24]),
            "ends inside Start of Field",
        ),
        (
            with_wtd(&[0x12, 0x01]),
            "ends inside Write Extended Attribute",
        ),
        (
            with_wtd(&[0x12, 0x07, 0x80]),
            "extended attribute of type X'07'",
        ),
        (
            with_wtd(&[0x15, 0x00, 0x09, 0x00]),
            "ends inside Write to Display Structured Field",
        ),
        (with_wtd(&[0x13, 30, 1]), "Insert Cursor"),
        (with_wtd(&[0x14, 1, 81]), "Move Cursor"),
        (vec![ESC, 0x23, 0x01, 3, 2], "Roll of rows 3 to 2"),
        (vec![ESC, 0x23, 0x01, 0, 2], "Roll of rows 0 to 2"),
        (vec![ESC, 0x23, 0x01, 1, 25], "Roll of rows 1 to 25"),
        (vec![ESC, 0x23, 0x01], "ends inside Roll"),
        (vec![ESC, 0x12, 0xD7], "ends inside Restore Screen"),
        (
            vec![ESC, 0xF3, 0x00, 0x09, 0xD9],
            "ends inside Write Structured Field",
        ),
        (vec![ESC, 0x52, 0x00], "ends inside the read command"),
    ] {
        let mut s = signed_on();
        s.receive(&host(&data));
        let notice = s.notice().unwrap_or_default().to_owned();
        assert!(notice.contains(says), "{data:02X?}: {notice:?}");
    }
}

#[test]
fn a_record_too_long_for_5250_is_dropped_with_a_notice_and_the_next_still_works_r_net_1() {
    let mut s = session();
    let mut bytes = vec![0x40; 70_000];
    bytes.extend([IAC, 239]);
    s.receive(&bytes);
    assert!(s.notice().is_some_and(|n| n.contains("dropped")));
    s.receive(&host(&sign_on()));
    assert_eq!(row_text(&s, 1).trim(), "Sign On");
}

#[test]
fn the_message_light_follows_its_opcodes_r_ds_2() {
    let mut s = session();
    s.receive(&host_record(Opcode::MessageLightOn, &[]));
    assert!(s.message_waiting());
    s.receive(&host_record(Opcode::MessageLightOff, &[]));
    assert!(!s.message_waiting());
}

#[test]
fn control_character_2_sets_the_message_light_sounds_the_alarm_and_unlocks_r_ds_2() {
    let mut s = session();
    assert_eq!(s.keyboard(), KeyboardState::Locked);
    s.receive(&host(&[ESC, 0x11, 0x00, 0x01]));
    assert!(s.message_waiting());
    assert_eq!(s.keyboard(), KeyboardState::Locked, "CC2 without unlock");
    s.receive(&host(&[ESC, 0x11, 0x00, 0x02]));
    assert!(!s.message_waiting());
    assert!(!s.take_alarm());
    s.receive(&host(&[ESC, 0x11, 0x00, 0x04]));
    assert!(s.take_alarm());
    assert!(!s.take_alarm(), "the alarm sounds once");
    s.receive(&host(&[ESC, 0x11, 0x00, 0x08]));
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
    s.receive(&host(&[ESC, 0x52, 0x00, 0x05]));
    assert!(
        s.message_waiting() && s.take_alarm(),
        "a read command's CC2 too"
    );
}

/// One field of the given field format word on row 2, column 10.
fn one_field(ffw: [u8; 2], len: u8) -> Session {
    let mut o = vec![SBA, 2, 9];
    o.extend(field(ffw, 0x24, len));
    let mut s = session();
    s.receive(&host(&screen_with(&o)));
    s
}

#[test]
fn each_kind_of_field_refuses_what_it_does_not_take_r_ds_2() {
    for (ffw, c, error) in [
        ([0x41, 0x00], '1', OperatorError::AlphaOnly),
        ([0x43, 0x00], 'a', OperatorError::NumericOnly),
        ([0x46, 0x00], 'a', OperatorError::NotKeyboardInput),
        ([0x40, 0x00], '😀', OperatorError::NotInCodePage),
    ] {
        let mut s = one_field(ffw, 5);
        s.key(Key::Char(c));
        assert_eq!(s.keyboard(), KeyboardState::Error(error), "{ffw:02X?} {c}");
        s.key(Key::Reset);
        assert_eq!(s.keyboard(), KeyboardState::Unlocked);
        assert_eq!(s.screen().get(address(&s, 2, 10)), 0, "nothing typed");
    }
}

#[test]
fn letters_comma_period_minus_and_space_go_into_an_alpha_only_field_r_ds_2() {
    let mut s = one_field([0x41, 0x00], 6);
    keys(&mut s, "æ,.- ");
    assert_eq!(s.keyboard(), KeyboardState::Unlocked);
    assert_eq!(row_text(&s, 2), "         æ,.-");
}

#[test]
fn typing_or_deleting_on_the_sign_position_is_an_operator_error_r_ds_2() {
    for key in [Key::Char('1'), Key::Delete] {
        let mut s = one_field([0x47, 0x00], 3);
        s.key(Key::MoveTo { row: 1, column: 11 });
        s.key(key);
        assert_eq!(
            s.keyboard(),
            KeyboardState::Error(OperatorError::SignPosition),
            "{key:?}"
        );
    }
}

#[test]
fn cancelling_the_password_stops_the_macro_and_sends_nothing_r_key_4() {
    let mut s = signed_on();
    s.play(
        "start",
        vec![Step::Password, enter(), Step::Text("x".into())],
    );
    assert_eq!(s.macro_state(), MacroState::NeedsPassword);
    s.give_macro_password(None);
    assert_eq!(s.macro_state(), MacroState::Off);
    assert!(s.notice().is_some_and(|n| n.contains("no password")));
    assert_eq!(sent(&mut s), Vec::<Vec<u8>>::new());
    assert_eq!(
        row_text(&s, 6).trim_end(),
        "                User . . .",
        "nothing typed"
    );
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn a_screen_is_kept_when_a_host_write_unlocks_the_keyboard_and_only_once_r_ui_37() {
    let mut s = signed_on();
    assert_eq!(s.history().len(), 1);
    let kept = &s.history().screens()[0];
    assert_eq!(
        kept.screen.cursor,
        address(&s, 6, 53),
        "with the cursor's place"
    );
    assert!(kept.screen.text(s.code_page()).contains("Sign On"));

    // The same screen again is not kept again.
    s.receive(&host(&sign_on()));
    assert_eq!((s.history().len(), s.history().added()), (1, 1));

    // A write that leaves the keyboard locked keeps nothing.
    s.key(Key::Aid(Aid::ENTER));
    let mut write = vec![ESC, 0x11, 0x00, 0x00, SBA, 1, 2];
    write.extend(ebcdic("Working æøå"));
    s.receive(&host(&write));
    assert_eq!(s.keyboard(), KeyboardState::Locked);
    assert_eq!(s.history().len(), 1);

    // Its unlock keeps the screen as it then is.
    s.receive(&host(&READ_MDT));
    assert_eq!(s.history().len(), 2);
    assert!(
        s.history().screens()[1]
            .screen
            .text(s.code_page())
            .contains("Working æøå")
    );
}

#[test]
fn a_typed_password_never_reaches_the_kept_screens_r_ui_37() {
    let mut s = signed_on();
    keys(&mut s, "qsecofr");
    s.key(Key::Tab);
    keys(&mut s, "secret");
    s.key(Key::Aid(Aid::ENTER));
    sent(&mut s);
    // The host unlocks again without writing the fields anew: the password is still in the live screen.
    s.receive(&host(&READ_MDT));
    assert!(contains(s.screen().buffer(), &ebcdic("secret")));
    assert_eq!(
        s.history().len(),
        2,
        "the typed user name makes it another screen"
    );
    for kept in s.history().screens() {
        assert!(!contains(kept.screen.buffer(), &ebcdic("secret")));
    }
    assert!(contains(
        s.history().screens()[1].screen.buffer(),
        &ebcdic("QSECOFR")
    ));
}

#[test]
fn the_history_goes_on_in_the_session_that_replaces_this_one_r_ui_37() {
    let mut old = signed_on();
    old.set_screens_kept(20);
    let mut new = session();
    new.adopt_history(old.take_history());
    assert!(old.history().is_empty());
    assert_eq!(old.history().limit(), 20, "the setting stays");
    assert_eq!((new.history().len(), new.history().limit()), (1, 20));
}
