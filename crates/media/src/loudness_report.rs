use crate::{LoudnessError, LoudnessUnavailable, LoudnessValue};

fn number(line: &str, label: &str, unit: &str) -> Result<f64, LoudnessError> {
    let text = line
        .trim()
        .strip_prefix(label)
        .and_then(|v| v.trim().strip_suffix(unit))
        .ok_or(LoudnessError::InvalidReport)?;
    text.trim()
        .parse::<f64>()
        .map_err(|_| LoudnessError::InvalidReport)
}
fn field(lines: &[&str], label: &str, unit: &str) -> Result<f64, LoudnessError> {
    let mut found = lines.iter().filter(|s| s.trim().starts_with(label));
    let value = number(
        found.next().ok_or(LoudnessError::InvalidReport)?,
        label,
        unit,
    )?;
    if found.next().is_some() {
        return Err(LoudnessError::InvalidReport);
    }
    Ok(value)
}
pub(crate) fn parse(bytes: &[u8]) -> Result<LoudnessValue, LoudnessError> {
    let text = std::str::from_utf8(bytes).map_err(|_| LoudnessError::InvalidReport)?;
    let summary = text
        .rsplit_once("Summary:")
        .ok_or(LoudnessError::InvalidReport)?
        .1;
    let (integrated, remaining) = summary
        .split_once("Loudness range:")
        .ok_or(LoudnessError::InvalidReport)?;
    let (range, peak) = remaining
        .split_once("True peak:")
        .ok_or(LoudnessError::InvalidReport)?;
    if !integrated.trim_start().starts_with("Integrated loudness:") {
        return Err(LoudnessError::InvalidReport);
    }
    let lines: Vec<_> = integrated.lines().collect();
    let i = field(&lines, "I:", "LUFS")?;
    let threshold = field(&lines, "Threshold:", "LUFS")?;
    let lra = field(&range.lines().collect::<Vec<_>>(), "LRA:", "LU")?;
    let peak = field(&peak.lines().collect::<Vec<_>>(), "Peak:", "dBFS")?;
    if !i.is_finite()
        || !threshold.is_finite()
        || !lra.is_finite()
        || lra < 0.
        || (!peak.is_finite() && peak != f64::NEG_INFINITY)
    {
        return Err(LoudnessError::InvalidReport);
    }
    let true_peak_db = peak.is_finite().then_some(peak);
    // FFmpeg's summary uses -70 as its no-gated-block floor (0.1 LU precision).
    // Conservatively reject values rounded to that floor rather than invent a usable gain.
    if i <= -70. {
        return Ok(LoudnessValue::NotMeasurable {
            reason: LoudnessUnavailable::BelowGateOrSilence,
            true_peak_db,
        });
    }
    Ok(LoudnessValue::Measured {
        integrated_lufs: i,
        true_peak_db: true_peak_db.ok_or(LoudnessError::InvalidReport)?,
        range_lu: lra,
        threshold_lufs: threshold,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data(i: &str, peak: &str) -> Vec<u8> {
        format!("[Parsed_ebur128_1 @ address] Summary:\n\n  Integrated loudness:\n    I: {i} LUFS\n    Threshold: -30.0 LUFS\n\n  Loudness range:\n    LRA: 0.0 LU\n    Threshold: -40.0 LUFS\n    LRA low: -23.0 LUFS\n    LRA high: -23.0 LUFS\n\n  True peak:\n    Peak: {peak} dBFS\n[out#0/null @ address] size=N/A\n").into_bytes()
    }
    #[test]
    fn silence_and_below_gate_are_not_numeric_loudness() {
        assert_eq!(
            parse(&data("-70.0", "-inf")).unwrap(),
            LoudnessValue::NotMeasurable {
                reason: LoudnessUnavailable::BelowGateOrSilence,
                true_peak_db: None
            }
        );
        assert_eq!(
            parse(&data("-70.0", "-85.0")).unwrap(),
            LoudnessValue::NotMeasurable {
                reason: LoudnessUnavailable::BelowGateOrSilence,
                true_peak_db: Some(-85.)
            }
        );
        assert!(matches!(
            parse(&data("-16.0", "-1.0")).unwrap(),
            LoudnessValue::Measured {
                integrated_lufs: -16.,
                true_peak_db: -1.,
                threshold_lufs: -30.,
                ..
            }
        ));
    }
    #[test]
    fn malformed_nonfinite_missing_or_duplicate_reports_fail_closed() {
        for i in ["NaN", "inf", "-inf", "invalid"] {
            assert!(parse(&data(i, "-1.0")).is_err());
        }
        for peak in ["NaN", "inf", "-inf"] {
            assert!(parse(&data("-16.0", peak)).is_err());
        }
        assert!(parse(b"{}").is_err());
        let valid = String::from_utf8(data("-16.0", "-1.0")).unwrap();
        for invalid in [
            valid.replace("I: -16.0 LUFS", "I: -16.0 LUFS\nI: -15.0 LUFS"),
            valid.replace("True peak:", "Sample peak:"),
            valid.replace("LUFS", "dB"),
            valid.replace("LRA: 0.0", "LRA: -1.0"),
        ] {
            assert!(parse(invalid.as_bytes()).is_err());
        }
    }
}
