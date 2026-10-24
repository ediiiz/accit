//! Durations as written in the file: whole numbers with a unit, `90m`, `12h`, `7d`.

use std::time::Duration;

use anyhow::{Result, bail};

pub(crate) fn parse(text: &str) -> Result<Duration> {
    let Some((unit_at, unit)) = text.char_indices().next_back() else {
        bail!("empty duration");
    };
    let digits = &text[..unit_at];
    let seconds_per_unit: u64 = match unit {
        's' => 1,
        'm' => 60,
        'h' => 3_600,
        'd' => 86_400,
        _ => bail!("duration `{text}` needs a unit: s, m, h or d"),
    };

    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("duration `{text}` is not a whole number with a unit");
    }

    let count: u64 = digits.parse()?;
    let Some(seconds) = count.checked_mul(seconds_per_unit) else {
        bail!("duration `{text}` is too large");
    };

    Ok(Duration::from_secs(seconds))
}
