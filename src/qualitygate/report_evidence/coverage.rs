//! Coverage decisions use exact counts; rounded percentages are display evidence.

use anyhow::{Context, Result, ensure};

/// Compare `hit * 100 / total` with the threshold's canonical decimal value.
/// An empty measured inventory has no percentage and does not fail a threshold.
pub fn below_threshold(hit: usize, total: usize, threshold: f64) -> Result<bool> {
    ensure!(hit <= total, "Covered count exceeds measured count");
    ensure!(
        threshold.is_finite() && (0.0..=100.0).contains(&threshold),
        "Coverage threshold must be finite and within 0..100"
    );

    if threshold == 0.0 || total == 0 {
        return Ok(false);
    }
    // Use the shortest decimal form of the configured number: 99.9% must
    // accept 999/1000, despite the float storing a value slightly above 99.9.
    // Scientific form bounds the coefficient to at most 17 significant digits.
    let decimal = format!("{threshold:e}");
    let (coefficient, exponent) = decimal
        .split_once('e')
        .context("Coverage threshold lacks a decimal exponent")?;
    let places = coefficient
        .split_once('.')
        .map_or(0, |(_, tail)| tail.len());
    let coefficient = coefficient.replace('.', "").parse::<u64>()?;
    let exponent = exponent.parse::<i32>()? - places as i32;
    // A 64-bit count times a 17-digit coefficient fits u128. Compare the
    // integer hit * 100 with ceil(total * threshold) before display rounding.
    let product = total as u128 * u128::from(coefficient);
    let minimum = if exponent >= 0 {
        product * 10u128.pow(exponent as u32)
    } else {
        match 10u128.checked_pow(exponent.unsigned_abs()) {
            Some(divisor) => product.div_ceil(divisor),
            // Here the divisor exceeds u128, so the positive fraction is < 1.
            None => 1,
        }
    };
    Ok((hit as u128 * 100) < minimum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_thresholds_match_exact_cross_products_at_counter_boundaries() {
        for total in [0, 1, 3, 10, 100, usize::MAX / 2, usize::MAX] {
            for threshold in 0..=100u128 {
                let first_pass = (total as u128 * threshold).div_ceil(100) as usize;
                for hit in [0, first_pass.saturating_sub(1), first_pass, total] {
                    assert_eq!(
                        below_threshold(hit, total, threshold as f64).unwrap(),
                        hit as u128 * 100 < total as u128 * threshold,
                        "hit={hit}, total={total}, threshold={threshold}"
                    );
                }
            }
        }
    }

    #[test]
    fn fractional_thresholds_preserve_equality_and_adjacent_float_boundaries() {
        assert!(below_threshold(1, 8, 12.5_f64.next_up()).unwrap());
        assert!(!below_threshold(1, 8, 12.5).unwrap());
        assert!(!below_threshold(1, 8, 12.5_f64.next_down()).unwrap());
        assert!(!below_threshold(999, 1000, 99.9).unwrap());
        assert!(!below_threshold(1, 1000, 0.1).unwrap());
        // The configured float immediately above the exact 100/3 fraction
        // must fail even when displaying that fraction yields the same float.
        let third = 100.0 / 3.0_f64;
        assert!(below_threshold(1, 3, third).unwrap());
        assert!(!below_threshold(1, 3, third.next_down()).unwrap());
        assert!(!below_threshold(0, 0, 100.0).unwrap());
        for threshold in [-0.0, 0.0] {
            assert!(!below_threshold(0, usize::MAX, threshold).unwrap());
        }
        for threshold in [f64::from_bits(1), f64::MIN_POSITIVE, 1e-30] {
            assert!(below_threshold(0, usize::MAX, threshold).unwrap());
            assert!(!below_threshold(1, usize::MAX, threshold).unwrap());
        }
    }

    #[test]
    fn decimal_thresholds_match_exact_tenths_at_large_counter_boundaries() {
        for tenths in [1, 9, 101, 333, 501, 901, 999u128] {
            for total in [0, 1000, usize::MAX] {
                let first_pass = (total as u128 * tenths).div_ceil(1000) as usize;
                for hit in [0, first_pass.saturating_sub(1), first_pass, total] {
                    assert_eq!(
                        below_threshold(hit, total, tenths as f64 / 10.0).unwrap(),
                        hit as u128 * 1000 < total as u128 * tenths,
                        "hit={hit}, total={total}, tenths={tenths}"
                    );
                }
            }
        }
    }

    #[test]
    fn invalid_counts_and_thresholds_are_errors() {
        assert!(below_threshold(2, 1, 100.0).is_err());
        for threshold in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 100.1] {
            assert!(below_threshold(1, 1, threshold).is_err());
        }
    }
}
