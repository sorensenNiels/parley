//! Select Graphic Rendition, `CSI … m` (ECMA-48 §8.3.117; xterm ctlseqs "Character Attributes"; the colon
//! forms of ITU T.416 for 38 and 48 and for 4:x underlines).

use super::cell::{Color, Style, Underline, Weight};

/// Applies the SGR parameters to `style`. Unknown parameters are skipped.
pub(super) fn apply(style: &mut Style, params: &vte::Params) {
    if params.is_empty() {
        *style = Style::default();
        return;
    }
    let mut params = params.iter();
    while let Some(param) = params.next() {
        let code = param.first().copied().unwrap_or(0);
        match code {
            0 => *style = Style::default(),
            1 => style.weight = Weight::Bold,
            2 => style.weight = Weight::Faint,
            3 => style.italic = true,
            4 => style.underline = underline(param.get(1).copied()),
            5 | 6 => style.blink = true,
            7 => style.inverse = true,
            8 => style.hidden = true,
            9 => style.strike = true,
            21 => style.underline = Underline::Double,
            22 => style.weight = Weight::Normal,
            23 => style.italic = false,
            24 => style.underline = Underline::None,
            25 => style.blink = false,
            27 => style.inverse = false,
            28 => style.hidden = false,
            29 => style.strike = false,
            30..=37 => style.fg = Color::Indexed(index(code - 30)),
            38 => {
                if let Some(color) = extended(param, &mut params) {
                    style.fg = color;
                }
            }
            39 => style.fg = Color::Default,
            40..=47 => style.bg = Color::Indexed(index(code - 40)),
            48 => {
                if let Some(color) = extended(param, &mut params) {
                    style.bg = color;
                }
            }
            49 => style.bg = Color::Default,
            90..=97 => style.fg = Color::Indexed(index(code - 90 + 8)),
            100..=107 => style.bg = Color::Indexed(index(code - 100 + 8)),
            _ => {}
        }
    }
}

fn index(value: u16) -> u8 {
    u8::try_from(value).unwrap_or(0)
}

/// `4` alone or `4:1` single, `4:2` double, `4:3` curly; `4:0` none; other styles (dotted, dashed) as single.
fn underline(style: Option<u16>) -> Underline {
    match style {
        Some(0) => Underline::None,
        Some(2) => Underline::Double,
        Some(3) => Underline::Curly,
        _ => Underline::Single,
    }
}

/// The colour after 38 or 48: `5;n` or `2;r;g;b` as separate parameters, or the colon forms `5:n`,
/// `2:r:g:b` and `2:colourspace:r:g:b` as sub-parameters.
fn extended<'a>(param: &[u16], rest: &mut impl Iterator<Item = &'a [u16]>) -> Option<Color> {
    if param.len() > 1 {
        return match param.get(1)? {
            5 => Some(Color::Indexed(index(*param.get(2)?))),
            2 => {
                let rgb = if param.len() >= 6 {
                    &param[3..6]
                } else {
                    param.get(2..5)?
                };
                Some(Color::Rgb(index(rgb[0]), index(rgb[1]), index(rgb[2])))
            }
            _ => None,
        };
    }
    let mut next = || rest.next().and_then(|p| p.first().copied());
    match next()? {
        5 => Some(Color::Indexed(index(next()?))),
        2 => Some(Color::Rgb(index(next()?), index(next()?), index(next()?))),
        _ => None,
    }
}
