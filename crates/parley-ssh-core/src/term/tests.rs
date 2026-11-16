//! R-SSH-6: the terminal, byte for byte.

use super::*;

fn term(columns: usize, rows: usize, input: &[u8]) -> Terminal {
    let mut t = Terminal::new(columns, rows);
    t.feed(input);
    t
}

/// The screen's rows as text, trailing blanks dropped.
fn screen(t: &Terminal) -> Vec<String> {
    (0..t.rows())
        .map(|r| t.visible_row(r).map(Row::text).unwrap_or_default())
        .collect()
}

fn cell(t: &Terminal, row: usize, column: usize) -> &Cell {
    &t.visible_row(row).expect("row on screen").cells[column]
}

#[test]
fn text_is_printed_and_cr_lf_move_the_cursor() {
    let t = term(10, 3, b"abc\r\ndef");
    assert_eq!(screen(&t), ["abc", "def", ""]);
    assert_eq!(t.cursor(), (1, 3));
}

#[test]
fn utf8_text_is_printed_also_when_split_between_reads() {
    let mut t = Terminal::new(10, 2);
    let bytes = "æøå".as_bytes();
    t.feed(&bytes[..3]);
    t.feed(&bytes[3..]);
    assert_eq!(screen(&t)[0], "æøå");
}

#[test]
fn a_sequence_split_between_reads_is_carried_out_once_complete() {
    let mut t = Terminal::new(10, 3);
    t.feed(b"\x1b[");
    t.feed(b"3;");
    t.feed(b"4Hx");
    assert_eq!(cell(&t, 2, 3).ch, 'x');
}

#[test]
fn text_wraps_at_the_right_edge_and_the_line_is_marked_wrapped() {
    let t = term(4, 3, b"abcdef");
    assert_eq!(screen(&t), ["abcd", "ef", ""]);
    assert!(t.visible_row(0).expect("row").wrapped);
    assert!(!t.visible_row(1).expect("row").wrapped);
}

#[test]
fn the_last_column_waits_for_the_next_character_before_wrapping() {
    // Writing the last column leaves the cursor there; a CR then returns on the same line.
    let t = term(4, 2, b"abcd\rX");
    assert_eq!(screen(&t), ["Xbcd", ""]);
}

#[test]
fn without_auto_wrap_the_last_column_is_overwritten() {
    let t = term(4, 2, b"\x1b[?7labcdef");
    assert_eq!(screen(&t), ["abcf", ""]);
}

#[test]
fn line_feed_at_the_bottom_scrolls_into_the_scrollback() {
    let t = term(5, 2, b"one\r\ntwo\r\nthree");
    assert_eq!(screen(&t), ["two", "three"]);
    assert_eq!(t.history_len(), 1);
    assert_eq!(t.line(0).map(Row::text).as_deref(), Some("one"));
}

#[test]
fn the_scrollback_keeps_only_its_limit() {
    let mut t = Terminal::new(5, 2);
    t.set_history_limit(3);
    for n in 0..10 {
        t.feed(format!("{n}\r\n").as_bytes());
    }
    assert_eq!(t.history_len(), 3);
    assert_eq!(t.dropped(), 6);
    assert_eq!(t.line(0).map(Row::text).as_deref(), Some("6"));
}

#[test]
fn scrolling_back_shows_the_history_and_stays_put_while_output_arrives_r_ssh_8() {
    let mut t = term(5, 2, b"a\r\nb\r\nc\r\nd");
    t.scroll_display(2);
    assert_eq!(screen(&t), ["a", "b"]);
    t.feed(b"\r\ne");
    assert_eq!(screen(&t), ["a", "b"]);
    t.scroll_to_bottom();
    assert_eq!(screen(&t), ["d", "e"]);
    t.scroll_display(100);
    assert_eq!(t.display_offset(), t.history_len());
    t.scroll_display(-100);
    assert_eq!(t.display_offset(), 0);
}

#[test]
fn cursor_position_is_one_based_and_kept_on_the_screen() {
    let mut t = term(10, 5, b"\x1b[2;3Hx");
    assert_eq!(cell(&t, 1, 2).ch, 'x');
    t.feed(b"\x1b[99;99H");
    assert_eq!(t.cursor(), (4, 9));
    t.feed(b"\x1b[H");
    assert_eq!(t.cursor(), (0, 0));
    t.feed(b"\x1b[0;0H");
    assert_eq!(t.cursor(), (0, 0));
}

#[test]
fn relative_moves_stop_at_the_edges() {
    let mut t = term(10, 5, b"\x1b[3;3H\x1b[2A");
    assert_eq!(t.cursor(), (0, 2));
    t.feed(b"\x1b[9B\x1b[99C");
    assert_eq!(t.cursor(), (4, 9));
    t.feed(b"\x1b[99D");
    assert_eq!(t.cursor(), (4, 0));
    t.feed(b"\x1b[3G\x1b[2d");
    assert_eq!(t.cursor(), (1, 2));
    t.feed(b"\x1b[E");
    assert_eq!(t.cursor(), (2, 0));
}

#[test]
fn erase_in_line_and_display_r_ssh_6() {
    let mut t = term(5, 3, b"aaaaa\r\nbbbbb\r\nccccc\x1b[2;3H\x1b[K");
    assert_eq!(screen(&t), ["aaaaa", "bb", "ccccc"]);
    t.feed(b"\x1b[1K");
    assert_eq!(screen(&t), ["aaaaa", "", "ccccc"]);
    t.feed(b"\x1b[J");
    assert_eq!(screen(&t), ["aaaaa", "", ""]);
    t.feed(b"\x1b[2J");
    assert_eq!(screen(&t), ["", "", ""]);
}

#[test]
fn erase_display_3_clears_the_scrollback() {
    let mut t = term(5, 2, b"1\r\n2\r\n3\r\n4");
    assert!(t.history_len() > 0);
    t.feed(b"\x1b[3J");
    assert_eq!(t.history_len(), 0);
}

#[test]
fn erasing_keeps_the_background_colour() {
    let t = term(5, 2, b"\x1b[44m\x1b[2J");
    assert_eq!(cell(&t, 1, 4).style.bg, Color::Indexed(4));
}

#[test]
fn insert_and_delete_characters_r_ssh_6() {
    let mut t = term(6, 1, b"abcdef\x1b[1;2H\x1b[2@");
    assert_eq!(screen(&t), ["a  bcd"]);
    t.feed(b"\x1b[3P");
    assert_eq!(screen(&t), ["acd"]);
    t.feed(b"\x1b[2X");
    assert_eq!(screen(&t), ["a"]);
}

#[test]
fn insert_mode_pushes_text_right() {
    let t = term(6, 1, b"abc\x1b[1;1H\x1b[4hXY\x1b[4lZ");
    assert_eq!(screen(&t), ["XYZbc"]);
}

#[test]
fn insert_and_delete_lines_inside_the_region() {
    let mut t = term(3, 4, b"1\r\n2\r\n3\r\n4\x1b[2;1H\x1b[L");
    assert_eq!(screen(&t), ["1", "", "2", "3"]);
    t.feed(b"\x1b[2M");
    assert_eq!(screen(&t), ["1", "3", "", ""]);
    assert_eq!(
        t.history_len(),
        0,
        "deleted lines never go into the scrollback"
    );
}

#[test]
fn a_scrolling_region_scrolls_only_its_rows_and_not_into_the_scrollback() {
    let mut t = term(3, 4, b"1\r\n2\r\n3\r\n4\x1b[2;3r");
    assert_eq!(t.cursor(), (0, 0), "DECSTBM homes the cursor");
    t.feed(b"\x1b[3;1H\nX");
    assert_eq!(screen(&t), ["1", "3", "X", "4"]);
    assert_eq!(t.history_len(), 0);
    t.feed(b"\x1b[2;1H\x1bM");
    assert_eq!(screen(&t), ["1", "", "3", "4"]);
}

#[test]
fn origin_mode_counts_from_the_region() {
    let mut t = term(5, 5, b"\x1b[2;4r\x1b[?6h\x1b[1;1Hx\x1b[9;1Hy");
    assert_eq!(cell(&t, 1, 0).ch, 'x');
    assert_eq!(cell(&t, 3, 0).ch, 'y');
    t.feed(b"\x1b[6n");
    assert_eq!(t.take_replies(), b"\x1b[3;2R");
}

#[test]
fn scroll_up_and_down_r_ssh_6() {
    let mut t = term(3, 3, b"1\r\n2\r\n3\x1b[S");
    assert_eq!(screen(&t), ["2", "3", ""]);
    t.feed(b"\x1b[2T");
    assert_eq!(screen(&t), ["", "", "2"]);
}

#[test]
fn tabs_stop_every_eight_columns_and_can_be_set_and_cleared() {
    let mut t = term(20, 1, b"\tx");
    assert_eq!(cell(&t, 0, 8).ch, 'x');
    t.feed(b"\x1b[3g\x1b[1;4H\x1bH\r\ty");
    assert_eq!(cell(&t, 0, 3).ch, 'y');
    t.feed(b"\t\t");
    assert_eq!(t.cursor(), (0, 19), "no stop left: the last column");
    t.feed(b"\x1b[Z");
    assert_eq!(t.cursor(), (0, 3));
}

#[test]
fn sgr_sets_attributes_and_colours_r_ssh_6() {
    let t = term(10, 1, b"\x1b[1;3;4;7;9;31;42ma\x1b[0mb");
    let style = cell(&t, 0, 0).style;
    assert_eq!(style.weight, Weight::Bold);
    assert!(style.italic && style.inverse && style.strike);
    assert_eq!(style.underline, Underline::Single);
    assert_eq!(style.fg, Color::Indexed(1));
    assert_eq!(style.bg, Color::Indexed(2));
    assert_eq!(cell(&t, 0, 1).style, Style::default());
}

#[test]
fn sgr_bright_256_and_true_colours_in_both_forms() {
    let t = term(10, 1, b"\x1b[91;102ma\x1b[38;5;200;48;2;1;2;3mb\x1b[38:2::10:20:30;48:5:17mc\x1b[4:3md\x1b[39;49;24me");
    assert_eq!(
        (cell(&t, 0, 0).style.fg, cell(&t, 0, 0).style.bg),
        (Color::Indexed(9), Color::Indexed(10))
    );
    assert_eq!(
        (cell(&t, 0, 1).style.fg, cell(&t, 0, 1).style.bg),
        (Color::Indexed(200), Color::Rgb(1, 2, 3))
    );
    assert_eq!(
        (cell(&t, 0, 2).style.fg, cell(&t, 0, 2).style.bg),
        (Color::Rgb(10, 20, 30), Color::Indexed(17))
    );
    assert_eq!(cell(&t, 0, 3).style.underline, Underline::Curly);
    assert_eq!(cell(&t, 0, 4).style, Style::default());
}

#[test]
fn truncated_sgr_colours_are_ignored_without_harm() {
    let t = term(10, 1, b"\x1b[38;5ma\x1b[48;2;1mb\x1b[38:2mc");
    assert_eq!(screen(&t), ["abc"]);
    assert_eq!(cell(&t, 0, 0).style.fg, Color::Default);
}

#[test]
fn wide_characters_take_two_cells_r_ssh_6() {
    let t = term(6, 2, "日本x".as_bytes());
    assert_eq!(cell(&t, 0, 0).width, Width::Wide);
    assert_eq!(cell(&t, 0, 1).width, Width::Spacer);
    assert_eq!(cell(&t, 0, 4).ch, 'x');
    assert_eq!(t.cursor(), (0, 5));
    assert_eq!(screen(&t)[0], "日本x");
}

#[test]
fn a_wide_character_that_does_not_fit_goes_to_the_next_line() {
    let t = term(3, 2, "ab日".as_bytes());
    assert_eq!(screen(&t), ["ab", "日"]);
    assert!(t.visible_row(0).expect("row").wrapped);
}

#[test]
fn overwriting_half_a_wide_character_blanks_the_other_half() {
    let t = term(4, 1, "日\x1b[1;2Hx".as_bytes());
    assert_eq!(cell(&t, 0, 0).ch, ' ');
    assert_eq!(cell(&t, 0, 0).width, Width::Narrow);
    assert_eq!(cell(&t, 0, 1).ch, 'x');
}

#[test]
fn combining_marks_join_the_character_before_them() {
    let t = term(5, 1, "e\u{301}x".as_bytes());
    assert_eq!(cell(&t, 0, 0).text(), "e\u{301}");
    assert_eq!(cell(&t, 0, 1).ch, 'x');
    // A combining mark at the start of a line has nothing to join and is dropped.
    let t = term(5, 1, "\u{301}a".as_bytes());
    assert_eq!(screen(&t), ["a"]);
}

#[test]
fn dec_line_drawing_is_shown_as_box_characters() {
    // G0 switched to line drawing and back; SO then selects G1, still ASCII.
    let t = term(10, 1, b"\x1b(0lqk\x1b(Bq\x0ex\x0f");
    assert_eq!(screen(&t), ["┌─┐qx"]);
    let t = term(10, 1, b"\x1b)0a\x0eqx\x0fq");
    assert_eq!(screen(&t), ["a─│q"]);
}

#[test]
fn the_alternate_screen_keeps_the_main_screen_and_cursor_r_ssh_6() {
    let mut t = term(5, 3, b"main\x1b[?1049h");
    assert!(t.alternate_screen());
    assert_eq!(screen(&t), ["", "", ""]);
    t.feed(b"\x1b[2;2Hvim");
    t.feed(b"\x1b[?1049l");
    assert!(!t.alternate_screen());
    assert_eq!(screen(&t), ["main", "", ""]);
    assert_eq!(t.cursor(), (0, 4));
}

#[test]
fn the_alternate_screen_has_no_scrollback() {
    let t = term(5, 2, b"\x1b[?1049h1\r\n2\r\n3\r\n4");
    assert_eq!(t.history_len(), 0);
}

#[test]
fn save_and_restore_the_cursor_with_its_style() {
    let mut t = term(10, 3, b"\x1b[2;3H\x1b[31m\x1b7\x1b[H\x1b[0m\x1b8x");
    assert_eq!(cell(&t, 1, 2).style.fg, Color::Indexed(1));
    t.feed(b"\x1b[3;1H\x1b[s\x1b[H\x1b[uy");
    assert_eq!(cell(&t, 2, 0).ch, 'y');
}

#[test]
fn modes_are_set_and_reset() {
    let mut t = term(
        5,
        2,
        b"\x1b[?1h\x1b[?25l\x1b[?2004h\x1b[?1004h\x1b=\x1b[5 q",
    );
    let modes = *t.modes();
    assert_eq!(modes.cursor_keys, KeyMode::Application);
    assert_eq!(modes.keypad, KeyMode::Application);
    assert!(!modes.cursor_visible && modes.bracketed_paste && modes.focus_events);
    assert_eq!(
        (modes.cursor_shape, modes.cursor_blink),
        (CursorShape::Bar, true)
    );
    t.feed(b"\x1b[?1l\x1b[?25l\x1b[?25h\x1b[?2004l\x1b>\x1b[2 q");
    let modes = *t.modes();
    assert_eq!(modes.cursor_keys, KeyMode::Normal);
    assert!(modes.cursor_visible && !modes.bracketed_paste);
    assert_eq!(
        (modes.cursor_shape, modes.cursor_blink),
        (CursorShape::Block, false)
    );
}

#[test]
fn device_attributes_and_status_are_answered_r_ssh_6() {
    let mut t = term(10, 5, b"\x1b[c\x1b[>c\x1b[5n\x1b[3;4H\x1b[6n\x1b[18t");
    assert_eq!(
        t.take_replies(),
        b"\x1b[?62;22c\x1b[>1;10;0c\x1b[0n\x1b[3;4R\x1b[8;5;10t"
    );
    assert_eq!(t.take_replies(), b"");
}

#[test]
fn the_title_comes_from_osc_0_and_2_without_controls() {
    let mut t = term(5, 1, b"\x1b]0;user@host: ~\x07");
    assert_eq!(t.title(), "user@host: ~");
    t.feed(b"\x1b]2;a;b\x1b\\");
    assert_eq!(t.title(), "a;b");
    t.feed(b"\x1b]1;icon\x07");
    assert_eq!(t.title(), "a;b");
}

#[test]
fn the_bell_is_counted() {
    let mut t = term(5, 1, b"\x07\x07");
    assert_eq!(t.take_bells(), 2);
    assert_eq!(t.take_bells(), 0);
}

#[test]
fn rep_repeats_the_last_character() {
    let t = term(10, 1, b"x\x1b[3b");
    assert_eq!(screen(&t), ["xxxx"]);
}

#[test]
fn decaln_fills_the_screen_with_e() {
    let t = term(3, 2, b"\x1b#8");
    assert_eq!(screen(&t), ["EEE", "EEE"]);
}

#[test]
fn ris_resets_the_screen_and_modes_but_keeps_the_scrollback() {
    let mut t = term(5, 2, b"1\r\n2\r\n3\x1b[?1h\x1b[31m");
    let history = t.history_len();
    t.feed(b"\x1bc");
    assert_eq!(screen(&t), ["", ""]);
    assert_eq!(t.modes().cursor_keys, KeyMode::Normal);
    assert_eq!(t.history_len(), history);
}

#[test]
fn soft_reset_keeps_the_screen() {
    let t = term(5, 2, b"abc\x1b[?7l\x1b[2;2r\x1b[!p");
    assert_eq!(screen(&t)[0], "abc");
    assert!(t.modes().auto_wrap);
}

#[test]
fn unknown_and_malformed_sequences_are_skipped_and_never_shown() {
    let t = term(
        10,
        2,
        b"a\x1b[?9999h\x1b[999999999999;1Hb\x1b]\x07\x1bP1$r\x1b\\\x1b[>4;1mc\x1b%Gd\x1b[",
    );
    // The huge row is taken as the last row, so b, c and d are printed there.
    assert_eq!(
        t.all_text()
            .chars()
            .filter(|c| c.is_alphabetic())
            .collect::<String>(),
        "abcd"
    );
}

#[test]
fn every_byte_value_is_fed_without_panic() {
    let mut t = Terminal::new(4, 2);
    let bytes: Vec<u8> = (0..=255).cycle().take(4096).collect();
    t.feed(&bytes);
    t.feed(b"\x1b[1;1H\x1b[99L\x1b[99M\x1b[99@\x1b[99P\x1b[99X\x1b[99S\x1b[99T\x1b[1;1r");
}

#[test]
fn copy_drops_trailing_blanks_and_joins_wrapped_lines_r_ssh_9() {
    let t = term(4, 3, b"abcdef\r\ngh  ");
    let all = t.text(Point { line: 0, column: 0 }, Point { line: 2, column: 3 });
    assert_eq!(all, "abcdef\ngh");
    // Selected backwards, the same text.
    assert_eq!(
        t.text(Point { line: 2, column: 3 }, Point { line: 0, column: 0 }),
        all
    );
    assert_eq!(
        t.text(Point { line: 0, column: 1 }, Point { line: 0, column: 2 }),
        "bc"
    );
}

#[test]
fn copy_all_takes_the_scrollback_and_the_screen() {
    let t = term(5, 2, b"1\r\n2\r\n3");
    assert_eq!(t.all_text(), "1\n2\n3");
}

#[test]
fn points_in_the_view_count_the_scrollback() {
    let mut t = term(5, 2, b"1\r\n2\r\n3\r\n4");
    assert_eq!(t.point_in_view(0, 0), Point { line: 2, column: 0 });
    t.scroll_display(1);
    assert_eq!(t.point_in_view(0, 0), Point { line: 1, column: 0 });
}

#[test]
fn growing_adds_blank_rows_and_columns() {
    let mut t = term(3, 2, b"abc\r\nd");
    t.resize(5, 3);
    assert_eq!(screen(&t), ["abc", "d", ""]);
    assert_eq!((t.columns(), t.rows()), (5, 3));
    t.feed(b"\x1b[3;5Hz");
    assert_eq!(cell(&t, 2, 4).ch, 'z');
}

#[test]
fn shrinking_drops_blank_rows_below_the_cursor_first_then_moves_top_rows_to_the_scrollback() {
    let mut t = term(5, 4, b"1\r\n2");
    t.resize(5, 2);
    assert_eq!(screen(&t), ["1", "2"]);
    assert_eq!(t.history_len(), 0);
    t.feed(b"\r\n3");
    t.resize(3, 1);
    assert_eq!(screen(&t), ["3"]);
    assert_eq!(t.history_len(), 2);
    assert_eq!(t.cursor(), (0, 1));
}

#[test]
fn a_size_of_zero_is_taken_as_one() {
    let mut t = Terminal::new(0, 0);
    t.feed(b"abc\r\n\x1b[5;5H");
    assert_eq!((t.columns(), t.rows()), (1, 1));
    t.resize(0, 0);
}

#[test]
fn shrinking_cuts_a_wide_character_at_the_edge() {
    let mut t = term(4, 1, "a日".as_bytes());
    t.resize(2, 1);
    assert_eq!(cell(&t, 0, 1).width, Width::Narrow);
}

#[test]
fn the_users_cursor_stands_until_the_host_chooses_and_comes_back_with_its_default_r_ssh_20() {
    let mut t = Terminal::new(5, 2);
    t.set_default_cursor(CursorShape::Bar, false);
    let shape = |t: &Terminal| (t.modes().cursor_shape, t.modes().cursor_blink);
    assert_eq!(shape(&t), (CursorShape::Bar, false));
    t.feed(b"\x1b[3 q");
    assert_eq!(
        shape(&t),
        (CursorShape::Underline, true),
        "the host's choice"
    );
    t.set_default_cursor(CursorShape::Block, false);
    assert_eq!(
        shape(&t),
        (CursorShape::Underline, true),
        "a cursor the host chose stays"
    );
    t.feed(b"\x1b[0 q");
    assert_eq!(
        shape(&t),
        (CursorShape::Block, false),
        "DECSCUSR 0 is the user's"
    );
    t.feed(b"\x1b[5 q\x1bc");
    assert_eq!(shape(&t), (CursorShape::Block, false), "RIS too");
}
