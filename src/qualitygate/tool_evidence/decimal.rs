//! Bounded exact decimal values. No binary floating point enters comparisons.

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct Decimal {
    pub coefficient: String,
    pub scale: u8,
}

impl Decimal {
    pub fn from_i64(value: i64) -> Self {
        Self {
            coefficient: value.to_string(),
            scale: 0,
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        ensure!(
            !text.is_empty() && text.len() <= 128,
            "Decimal token exceeds bounds or is empty"
        );
        let (negative, token) = text.strip_prefix('-').map_or((false, text), |v| (true, v));
        let (mantissa, exponent) = match token.find(['e', 'E']) {
            Some(at) => {
                let exponent = token[at + 1..].parse::<i32>()?;
                ensure!(
                    (-128..=128).contains(&exponent),
                    "Decimal exponent exceeds bounds"
                );
                (&token[..at], exponent)
            }
            None => (token, 0),
        };
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        ensure!(
            !whole.is_empty() && whole.bytes().all(|v| v.is_ascii_digit()),
            "Invalid decimal integer"
        );
        ensure!(
            whole.len() == 1 || !whole.starts_with('0'),
            "Decimal has leading zeros"
        );
        ensure!(
            !mantissa.contains('.') || !fraction.is_empty(),
            "Decimal fraction is empty"
        );
        ensure!(
            fraction.bytes().all(|v| v.is_ascii_digit()),
            "Invalid decimal fraction"
        );
        let mut digits = format!("{whole}{fraction}")
            .trim_start_matches('0')
            .to_string();
        if digits.is_empty() {
            ensure!(!negative, "Negative zero is forbidden");
            return Ok(Self::from_i64(0));
        }
        let mut scale = i32::try_from(fraction.len())? - exponent;
        while scale > 0 && digits.ends_with('0') {
            digits.pop();
            scale -= 1;
        }
        if scale < 0 {
            let zeros = usize::try_from(-scale)?;
            ensure!(
                digits.len() + zeros <= 38,
                "Decimal coefficient exceeds 38 digits"
            );
            digits.extend(std::iter::repeat_n('0', zeros));
            scale = 0;
        }
        ensure!(
            digits.len() <= 38 && scale <= 18,
            "Decimal precision or scale exceeds bounds"
        );
        let coefficient = if negative {
            format!("-{digits}")
        } else {
            digits
        };
        let value = Self {
            coefficient,
            scale: u8::try_from(scale)?,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(self.scale <= 18, "Decimal scale exceeds 18");
        let digits = self
            .coefficient
            .strip_prefix('-')
            .unwrap_or(&self.coefficient);
        ensure!(
            !digits.is_empty() && digits.len() <= 38 && digits.bytes().all(|v| v.is_ascii_digit()),
            "Invalid decimal coefficient"
        );
        ensure!(
            digits.len() == 1 || !digits.starts_with('0'),
            "Decimal has leading zeros"
        );
        if digits == "0" {
            ensure!(
                self.coefficient == "0" && self.scale == 0,
                "Noncanonical decimal zero"
            );
        } else {
            ensure!(
                self.scale == 0 || !digits.ends_with('0'),
                "Decimal has removable trailing zeros"
            );
        }
        Ok(())
    }

    pub fn as_i64(&self) -> Result<i64> {
        self.validate()?;
        if self.scale != 0 {
            bail!("Expected an integer");
        }
        Ok(self.coefficient.parse()?)
    }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        let left_negative = self.coefficient.starts_with('-');
        let right_negative = other.coefficient.starts_with('-');
        if left_negative != right_negative {
            return if left_negative {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        // At most 56 decimal digits after alignment; no numeric multiplication.
        let scale = self.scale.max(other.scale);
        let mut left = self.coefficient.trim_start_matches('-').to_owned();
        let mut right = other.coefficient.trim_start_matches('-').to_owned();
        left.extend(std::iter::repeat_n('0', usize::from(scale - self.scale)));
        right.extend(std::iter::repeat_n('0', usize::from(scale - other.scale)));
        let left = left.trim_start_matches('0');
        let right = right.trim_start_matches('0');
        let order = left.len().cmp(&right.len()).then_with(|| left.cmp(right));
        if left_negative {
            order.reverse()
        } else {
            order
        }
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
#[path = "decimal_tests.rs"]
mod tests;
