//! The checks a 5250 display makes on a field before the operator may leave it or press Enter
//! (R-KEY-10): mandatory enter, mandatory fill and self-check (SC30-3533-04 §15.6.12.1 "Field
//! Format Word", §15.6.12.2 "Field Control Word"; the algorithms as IBM's "DDS for display
//! files", CHECK keyword, gives them). A failed check is an operator error with the standard
//! 5250 code, and the keyboard stays inhibited until Reset.

use super::input::Aid;
use super::{OperatorError, Session};
use crate::screen::{Adjust, Field, SelfCheck};

/// How the operator is leaving a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Leaving {
    /// Field Exit, Field+ or Field-.
    FieldExit,
    /// Tab, Back Tab, or typing past the last position.
    Moving,
}

impl Session {
    /// §15.6.12.1: Field+, Field- or Field Exit may not bypass a mandatory enter field with
    /// nothing typed. Checked before Field Exit changes the field.
    pub(super) const fn check_mandatory_enter_exit(field: &Field) -> Result<(), OperatorError> {
        if field.format.mandatory_enter() && !field.modified {
            return Err(OperatorError::MandatoryEnterExit);
        }
        Ok(())
    }

    /// The checks for leaving `field` (as it is now, after Field Exit has adjusted it). A
    /// failed self-check puts the cursor at the field's start, as a 5250 does, so the number
    /// can be typed again.
    pub(super) fn check_leaving(
        &mut self,
        field: &Field,
        leaving: Leaving,
    ) -> Result<(), OperatorError> {
        let data = self.typed_part(field);
        // Mandatory fill: once anything is typed, every position must be; Field Exit from the
        // first position empties the field instead.
        let at_start = self.screen.cursor == field.start;
        if field.format.adjust() == Adjust::MandatoryFill
            && field.modified
            && data.contains(&0)
            && data.iter().any(|&b| b != 0)
        {
            match leaving {
                Leaving::FieldExit if at_start => {}
                Leaving::FieldExit => return Err(OperatorError::MandatoryFillExit),
                Leaving::Moving => return Err(OperatorError::MandatoryFill),
            }
        }
        if let Some(check) = field.self_check
            && !(leaving == Leaving::FieldExit && at_start)
            && !self_check_passes(check, &data)
        {
            self.screen.cursor = field.start;
            return Err(OperatorError::CheckDigit);
        }
        Ok(())
    }

    /// Enter is refused while a mandatory enter field has nothing typed, if anything on the
    /// screen was (§15.6.12.1; DDS CHECK(ME): with no field changed, it is not enforced). The
    /// cursor goes to that field.
    pub(super) fn check_before_aid(&mut self, aid: Aid) -> Result<(), OperatorError> {
        if aid != Aid::ENTER || !self.screen.fields().iter().any(|f| f.modified) {
            return Ok(());
        }
        let empty = self
            .screen
            .fields()
            .iter()
            .find(|f| f.is_input() && f.format.mandatory_enter() && !f.modified);
        if let Some(field) = empty {
            self.screen.cursor = field.start;
            return Err(OperatorError::MandatoryEnter);
        }
        Ok(())
    }

    /// The positions the operator types in: a signed numeric field's sign is not checked.
    fn typed_part(&self, field: &Field) -> Vec<u8> {
        self.screen
            .field_bytes(field)
            .get(..field.data_end() - field.start)
            .unwrap_or_default()
            .to_vec()
    }
}

/// Whether `data` (EBCDIC, the check digit last) is a valid number for `check`. Each byte
/// counts as the value of its low 4 bits when they are 0-9, else 0, so nulls and blanks are 0
/// and an all-null field passes (SC30-3533-04 §15.6.12.2 "Self-Check").
pub(super) fn self_check_passes(check: SelfCheck, data: &[u8]) -> bool {
    let digits: Vec<u32> = data
        .iter()
        .map(|&b| u32::from(b & 0x0F))
        .map(|d| if d <= 9 { d } else { 0 })
        .collect();
    let Some((&check_digit, base)) = digits.split_last() else {
        return true;
    };
    // Weights count from the base number's rightmost digit (DDS, CHECK keyword).
    let weighted = base.iter().rev().enumerate();
    match check {
        SelfCheck::Modulus10 => {
            let sum: u32 = weighted
                .map(|(i, &d)| if i % 2 == 0 { d * 2 } else { d })
                .map(|p| p / 10 + p % 10)
                .sum();
            (10 - sum % 10) % 10 == check_digit
        }
        SelfCheck::Modulus11 => {
            let sum: u32 = weighted
                .map(|(i, &d)| d * (2 + u32::try_from(i % 6).unwrap_or(0)))
                .sum();
            // A remainder of 1 leaves the base number with no check digit at all.
            match sum % 11 {
                0 => check_digit == 0,
                1 => false,
                r => 11 - r == check_digit,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CodePage;

    fn ebcdic(text: &str) -> Vec<u8> {
        text.chars()
            .map(|c| CodePage::Ccsid37.to_byte(c).expect("CCSID 37"))
            .collect()
    }

    #[test]
    fn modulus_11_follows_ibm_s_example_r_key_10() {
        // DDS for display files, CHECK keyword: base 13739 has self-check digit 1.
        assert!(self_check_passes(SelfCheck::Modulus11, &ebcdic("137391")));
        assert!(!self_check_passes(SelfCheck::Modulus11, &ebcdic("137392")));
    }

    #[test]
    fn modulus_11_with_remainder_1_has_no_valid_check_digit_r_key_10() {
        // Base 6: 6 x 2 = 12, and 12 mod 11 = 1.
        for d in 0..=9 {
            assert!(!self_check_passes(
                SelfCheck::Modulus11,
                &ebcdic(&format!("6{d}"))
            ));
        }
        // Remainder 0: the check digit is 0.
        assert!(self_check_passes(SelfCheck::Modulus11, &ebcdic("00")));
    }

    #[test]
    fn modulus_10_doubles_every_other_digit_from_the_right_r_key_10() {
        // 7992739871 with check digit 3 is the well-known Luhn example.
        assert!(self_check_passes(
            SelfCheck::Modulus10,
            &ebcdic("79927398713")
        ));
        assert!(!self_check_passes(
            SelfCheck::Modulus10,
            &ebcdic("79927398710")
        ));
        // A product over 9 adds its digits: 5 x 2 = 10 counts 1 + 0 = 1, so the check is 9.
        assert!(self_check_passes(SelfCheck::Modulus10, &ebcdic("59")));
    }

    #[test]
    fn letters_count_by_their_low_bits_and_an_empty_field_passes_r_key_10() {
        // A = X'C1' counts 1, R = X'D9' counts 9 (SC30-3533-04); "A9" equals "19".
        assert_eq!(
            self_check_passes(SelfCheck::Modulus10, &ebcdic("A8")),
            self_check_passes(SelfCheck::Modulus10, &ebcdic("18"))
        );
        assert!(self_check_passes(SelfCheck::Modulus10, &[0, 0, 0]));
        assert!(self_check_passes(SelfCheck::Modulus11, &[0x40, 0x40]));
        assert!(self_check_passes(SelfCheck::Modulus11, &[]));
    }
}
