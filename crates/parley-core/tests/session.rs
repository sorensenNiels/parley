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
    assert_eq!(&data[49..51], &[0x00, 0x11], "24×80 and colour");

    let mut wide = Session::new(SessionConfig {
        screen_size: ScreenSize::Wide,
        ..SessionConfig::default()
    });
    wide.receive(&query);
    let (_, _, data) = sent_one(&mut wide);
    assert_eq!(data.len(), 61);
    assert_eq!(&data[30..37], ebcdic("3477FC ").as_slice());
    assert_eq!(&data[49..51], &[0x00, 0x31], "27×132, 24×80 and colour");
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
