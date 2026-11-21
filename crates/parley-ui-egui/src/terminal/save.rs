//! Saving the history's screen (R-UI-39): as text, or as an HTML file of its own in the session's colours.

use std::fmt::Write as _;
use std::path::Path;

use egui::Color32;
use parley_core::CodePage;
use parley_core::screen::{Cell, Screen, Style};

use super::Palette;

/// Where a saved screen goes: the system's file window in the app, which only the launcher can show.
pub trait ScreenFiles: std::fmt::Debug {
    /// Asks where to save a screen, offering `suggested` (without an extension) as its name, and writes `text`
    /// or `html` there as [`format_of`] says for the path chosen: where it went, or `None` if cancelled.
    ///
    /// # Errors
    /// The file could not be written, said for the user.
    fn save(&self, suggested: &str, text: &str, html: &str) -> Result<Option<String>, String>;
}

/// No file window: a UI built without one.
#[derive(Debug)]
pub struct NoScreenFiles;

impl ScreenFiles for NoScreenFiles {
    fn save(&self, _: &str, _: &str, _: &str) -> Result<Option<String>, String> {
        Err("Parley 5250 cannot save files here.".to_owned())
    }
}

/// How a saved screen is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenFormat {
    Text,
    Html,
}

/// The format a path's extension asks for: HTML for `.html` and `.htm`, else text.
pub fn format_of(path: &Path) -> ScreenFormat {
    match path.extension().and_then(|e| e.to_str()) {
        Some(e) if e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm") => {
            ScreenFormat::Html
        }
        _ => ScreenFormat::Text,
    }
}

/// The screen's rows as text, trailing blanks cut, each ending in a line break.
pub fn screen_text(screen: &Screen, code_page: CodePage) -> String {
    let mut text = String::new();
    for row in screen.text(code_page).lines() {
        text.push_str(row.trim_end());
        text.push('\n');
    }
    text
}

/// How the HTML file looks: the session's colours and font, whether blinking text blinks (else it shows in
/// reverse, as on the screen), and the page's title.
#[derive(Debug, Clone, Copy)]
pub struct HtmlLook<'a> {
    pub palette: &'a Palette,
    pub font: &'a str,
    pub blinking: bool,
    pub title: &'a str,
}

/// The screen as one HTML file of its own: each position's colour, reverse image, underline and column
/// separators as the screen draws them (R-UI-39).
pub fn screen_html(screen: &Screen, code_page: CodePage, look: &HtmlLook<'_>) -> String {
    let columns = usize::from(screen.size().columns()).max(1);
    let cells = screen.cells(code_page);
    let mut rows = String::new();
    for row in cells.chunks(columns) {
        let mut run: Option<(Style, String)> = None;
        for cell in row {
            // A column separator is a mark on its own position, so each such position is a span of its own.
            let joins = !cell.style.column_separators;
            match &mut run {
                Some((style, text)) if *style == cell.style && joins => push_char(text, cell),
                _ => {
                    if let Some((style, text)) = run.take() {
                        rows.push_str(&span(style, &text, look));
                    }
                    let mut text = String::new();
                    push_char(&mut text, cell);
                    run = Some((cell.style, text));
                }
            }
        }
        if let Some((style, text)) = run {
            rows.push_str(&span(style, &text, look));
        }
        rows.push('\n');
    }
    let background = hex(look.palette.background);
    let separator = hex(look
        .palette
        .color(parley_core::Color::Green)
        .gamma_multiply(0.6));
    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n<style>\n\
         body {{ background: {background}; margin: 1.5em; }}\n\
         pre {{ font-family: \"{font}\", monospace; font-size: 15px; line-height: 1.25; margin: 0; }}\n\
         .cs {{ background-image: linear-gradient(to right, {separator} 1px, transparent 1px); \
         background-size: 100% 25%; background-position: left bottom; background-repeat: no-repeat; }}\n\
         </style>\n</head>\n<body>\n<pre>\n{rows}</pre>\n</body>\n</html>\n",
        title = escape(look.title),
        font = escape(&look.font.replace('"', "")),
    )
}

fn push_char(text: &mut String, cell: &Cell) {
    match cell.ch {
        '&' => text.push_str("&amp;"),
        '<' => text.push_str("&lt;"),
        '>' => text.push_str("&gt;"),
        c => text.push(c),
    }
}

/// A run of positions of one style.
fn span(style: Style, text: &str, look: &HtmlLook<'_>) -> String {
    let fg = look.palette.color(style.color);
    let reverse = style.reverse || (style.blink && !look.blinking);
    let (color, fill) = if reverse {
        (look.palette.background, Some(fg))
    } else {
        (fg, None)
    };
    let mut css = format!("color:{}", hex(color));
    if let Some(fill) = fill {
        // Writing to a String cannot fail.
        let _ = write!(css, ";background-color:{}", hex(fill));
    }
    if style.underline {
        css.push_str(";text-decoration:underline");
    }
    let class = if style.column_separators {
        " class=\"cs\""
    } else {
        ""
    };
    format!("<span{class} style=\"{css}\">{text}</span>")
}

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use parley_core::screen::ScreenSize;

    fn screen_saying(text: &str, attribute: Option<u8>) -> Screen {
        let mut s = Screen::new(ScreenSize::Standard);
        let mut at = 0;
        if let Some(a) = attribute {
            s.put(0, a);
            at = 1;
        }
        for (i, c) in text.chars().enumerate() {
            s.put(
                at + i,
                CodePage::Ccsid37.to_byte(c).expect("CCSID 37 character"),
            );
        }
        s
    }

    #[test]
    fn text_has_every_row_with_its_trailing_blanks_cut_r_ui_39() {
        let text = screen_text(&screen_saying("Æble <æøå>  ", None), CodePage::Ccsid37);
        assert_eq!(text.lines().count(), 24);
        assert_eq!(text.lines().next(), Some("Æble <æøå>"));
        assert!(text.ends_with("\n\n"));
    }

    #[test]
    fn html_is_one_file_in_the_session_s_colours_with_its_title_r_ui_39() {
        let palette = Palette::default();
        let look = HtmlLook {
            palette: &palette,
            font: "JetBrains Mono",
            blinking: true,
            title: "DKSOFT – <10:42:13>",
        };
        // Reverse image white, then a column separator field.
        let mut screen = screen_saying("a<b", Some(0x23));
        screen.put(4, 0x30);
        screen.put(5, CodePage::Ccsid37.to_byte('x').expect("x"));
        let html = screen_html(&screen, CodePage::Ccsid37, &look);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<title>DKSOFT – &lt;10:42:13&gt;</title>"));
        assert!(html.contains("\"JetBrains Mono\", monospace"));
        assert!(!html.contains("<script"), "nothing but the page itself");
        let white = hex(palette.color(parley_core::Color::White));
        let background = hex(palette.background);
        assert!(
            html.contains(&format!(
                "style=\"color:{background};background-color:{white}\">a&lt;b</span>"
            )),
            "{html}"
        );
        assert!(html.contains("<span class=\"cs\""), "{html}");
        assert_eq!(html.matches("\n<span").count(), 24, "a line per row");
    }

    #[test]
    fn a_path_s_extension_says_the_format_r_ui_39() {
        assert_eq!(format_of(Path::new("æøå.HTML")), ScreenFormat::Html);
        assert_eq!(format_of(Path::new("screen.htm")), ScreenFormat::Html);
        assert_eq!(format_of(Path::new("screen.txt")), ScreenFormat::Text);
        assert_eq!(format_of(Path::new("screen")), ScreenFormat::Text);
    }
}
