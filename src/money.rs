use std::cmp::Ordering;

use rust_decimal::{Decimal, RoundingStrategy};

/// `bytes / 1024^3`, rounded once, half away from zero, to the digits a decimal holds.
/// The pricing page says GB = 1024^3 bytes.
#[must_use]
pub fn gb_from_bytes(bytes: Decimal) -> Decimal {
    let negative = bytes.is_sign_negative();
    let Some(amount) = Amount::from_decimal(bytes.abs()) else {
        return Decimal::ZERO;
    };
    let gb = amount.to_gb_decimal();
    if negative {
        -gb
    } else {
        gb
    }
}

/// Days since 1970-01-01 in the civil calendar. Howard Hinnant's algorithm.
/// `year`, `month`, and `day` must be a real Gregorian date.
#[must_use]
pub fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let mut y = i64::from(year);
    let m = i64::from(month);
    let d = i64::from(day);
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Midnight UTC of a real civil date, as Unix milliseconds.
#[must_use]
pub fn epoch_millis(year: i32, month: u32, day: u32) -> i64 {
    days_from_civil(year, month, day) * 86_400_000
}

pub(crate) fn ymd_to_epoch_millis(text: &str) -> Option<i64> {
    let (year, month, day) = parse_ymd(text)?;
    if !(1970..=2199).contains(&year) {
        return None;
    }
    Some(epoch_millis(year, month, day))
}

pub(crate) fn parse_ymd(text: &str) -> Option<(i32, u32, u32)> {
    if !valid_date(text) {
        return None;
    }
    Some((
        text[0..4].parse().ok()?,
        text[5..7].parse().ok()?,
        text[8..10].parse().ok()?,
    ))
}

/// EC2 timestamp as Unix milliseconds. UTC only: `Z` or `+00:00`.
/// Optional fraction of a second, at most 9 digits. Years 1970 through 2199.
/// Second 60 is rejected. Extra fractional digits past milliseconds are truncated.
#[must_use]
pub fn parse_utc_millis(text: &str) -> Option<i64> {
    // Later slices are byte indexes. A multibyte second is not a timestamp.
    if !text.is_ascii() {
        return None;
    }
    let bytes = text.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let year: i32 = text[0..4].parse().ok()?;
    let month: u32 = text[5..7].parse().ok()?;
    let day: u32 = text[8..10].parse().ok()?;
    // `parse` accepts a leading '+'. Clock fields are digits only, before the slice.
    if !bytes[11].is_ascii_digit()
        || !bytes[12].is_ascii_digit()
        || !bytes[14].is_ascii_digit()
        || !bytes[15].is_ascii_digit()
        || !bytes[17].is_ascii_digit()
        || !bytes[18].is_ascii_digit()
    {
        return None;
    }
    let hour: u32 = text[11..13].parse().ok()?;
    let minute: u32 = text[14..16].parse().ok()?;
    let second: u32 = text[17..19].parse().ok()?;
    if !(1970..=2199).contains(&year) || !valid_date(&text[0..10]) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let mut index = 19;
    let mut millis = 0_u32;
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let digits = &text[start..index];
        if digits.is_empty() || digits.len() > 9 {
            return None;
        }
        let mut padded = digits.to_string();
        if padded.len() < 3 {
            padded.push_str(&"0".repeat(3 - padded.len()));
        }
        millis = padded[..3].parse().ok()?;
    }
    let rest = &text[index..];
    if rest != "Z" && rest != "+00:00" {
        return None;
    }
    let day_ms = epoch_millis(year, month, day);
    let clock = i64::from(hour) * 3_600_000
        + i64::from(minute) * 60_000
        + i64::from(second) * 1_000
        + i64::from(millis);
    day_ms.checked_add(clock)
}

/// Decimal text with no exponent. Trailing zeros that `normalize` drops stay dropped.
#[must_use]
pub fn format_dec(value: Decimal) -> String {
    let text = value.normalize().to_string();
    if text.contains(['e', 'E']) {
        return value.round_dp(12).normalize().to_string();
    }
    text
}

/// USD for a bill: half away from zero to 2 decimal places, then at least two digits.
#[must_use]
pub fn format_usd(value: impl UsdText) -> String {
    value.usd_text()
}

/// Something that can be shown as USD, rounded once.
pub trait UsdText {
    fn usd_text(&self) -> String;
}

impl UsdText for Decimal {
    fn usd_text(&self) -> String {
        format_usd_decimal(*self)
    }
}

impl UsdText for Amount {
    fn usd_text(&self) -> String {
        self.usd_text_owned()
    }
}

fn format_usd_decimal(value: Decimal) -> String {
    let rounded = value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
    let text = format_dec(rounded);
    let Some((whole, frac)) = text.split_once('.') else {
        return format!("{text}.00");
    };
    if frac.len() >= 2 {
        text
    } else {
        format!("{whole}.{frac}{}", "0".repeat(2 - frac.len()))
    }
}

/// True when a plain decimal text has more fraction digits than `Decimal` can hold (28).
pub(crate) fn too_precise(text: &str) -> bool {
    trim_ascii_ws(text)
        .split_once('.')
        .is_some_and(|(_, frac)| frac.len() > 28)
}

/// True when a plain decimal text is more than `Decimal` holds exactly, so that
/// parsing it would silently round it. Leading and trailing zeros do not count.
/// A text that is not a plain decimal is false: `parse_money` rejects it.
pub fn loses_digits(text: &str) -> bool {
    let text = trim_ascii_ws(text);
    let Ok(parsed) = parse_money(text) else {
        return false;
    };
    let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
    let whole = whole.trim_start_matches('0');
    let frac = frac.trim_end_matches('0');
    let want = format!(
        "{}{}{frac}",
        if whole.is_empty() { "0" } else { whole },
        if frac.is_empty() { "" } else { "." }
    );
    parsed.normalize().to_string() != want
}

/// Trims only space, tab, CR and LF. Unicode whitespace (NBSP, U+2028, U+3000) stays.
pub(crate) fn trim_ascii_ws(text: &str) -> &str {
    text.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}

pub(crate) fn parse_money(raw: &str) -> Result<Decimal, ()> {
    let text = trim_ascii_ws(raw);
    if text.is_empty() || text.len() > 40 {
        return Err(());
    }
    let bytes = text.as_bytes();
    let mut dot = false;
    for (i, byte) in bytes.iter().enumerate() {
        match byte {
            b'0'..=b'9' => {}
            b'.' if !dot && i > 0 && i + 1 < bytes.len() => dot = true,
            _ => return Err(()),
        }
    }
    text.parse().map_err(|_| ())
}

pub(crate) fn valid_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits_only = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    if !digits_only(0..4) || !digits_only(5..7) || !digits_only(8..10) {
        return false;
    }
    let Ok(year) = text[0..4].parse::<i32>() else {
        return false;
    };
    let Ok(month) = text[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = text[8..10].parse::<u32>() else {
        return false;
    };
    if !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=max).contains(&day)
}

/// Non-negative exact amount, `limbs / 10^scale`, little-endian, 512 bits.
/// A bill keeps this value and rounds once, at display, half away from zero.
#[derive(Clone, Copy, Debug)]
pub struct Amount {
    limbs: [u64; 8],
    scale: u32,
}

impl PartialEq for Amount {
    fn eq(&self, other: &Self) -> bool {
        self.cmp_amount(*other) == Ordering::Equal
    }
}

impl Eq for Amount {}

impl Amount {
    pub(crate) const ZERO: Self = Self {
        limbs: [0; 8],
        scale: 0,
    };

    /// `None` when `value` is negative. The mantissa is kept whole.
    pub(crate) fn from_decimal(value: Decimal) -> Option<Self> {
        let mantissa = u128::try_from(value.mantissa()).ok()?;
        Some(Self {
            limbs: limbs_from_u128(mantissa),
            scale: value.scale(),
        })
    }

    /// The once-rounded cent amount fits in a decimal after trailing zeros are dropped.
    pub(crate) fn cents_fit(self) -> bool {
        self.round_half_away(2)
            .is_some_and(|cents| decimal_fitting(cents).is_some())
    }

    /// Strictly greater than `limit`. A negative limit is below every amount.
    pub(crate) fn gt_decimal(self, limit: Decimal) -> bool {
        match Self::from_decimal(limit) {
            Some(other) => self.cmp_amount(other) == Ordering::Greater,
            None => true,
        }
    }

    #[cfg(test)]
    pub(crate) fn gt_amount(self, other: Self) -> bool {
        self.cmp_amount(other) == Ordering::Greater
    }

    #[cfg(test)]
    pub(crate) fn eq_decimal(self, value: Decimal) -> bool {
        Self::from_decimal(value).is_some_and(|other| self == other)
    }

    fn usd_text_owned(self) -> String {
        let Some(cents) = self.round_half_away(2) else {
            return self.to_string();
        };
        if let Some(dec) = decimal_fitting(cents) {
            return format_usd_decimal(dec);
        }
        cents.to_string()
    }

    fn to_gb_decimal(self) -> Decimal {
        for places in (0..=28).rev() {
            if let Some(rounded) = self.gb_at(places) {
                if let Some(dec) = decimal_fitting(rounded) {
                    return dec;
                }
            }
        }
        Decimal::ZERO
    }

    fn cmp_amount(self, other: Self) -> Ordering {
        if self.scale == other.scale {
            return cmp_limbs(self.limbs, other.limbs);
        }
        if self.scale < other.scale {
            return match mul_pow10(self.limbs, other.scale - self.scale) {
                Some(scaled) => cmp_limbs(scaled, other.limbs),
                None => Ordering::Greater,
            };
        }
        match mul_pow10(other.limbs, self.scale - other.scale) {
            Some(scaled) => cmp_limbs(self.limbs, scaled),
            None => Ordering::Less,
        }
    }

    /// Round half away from zero to `places` decimal places. `None` if the 512-bit
    /// accumulator overflows.
    fn round_half_away(self, places: u32) -> Option<Self> {
        if self.scale <= places {
            let limbs = mul_pow10(self.limbs, places - self.scale)?;
            return Some(Self {
                limbs,
                scale: places,
            });
        }
        let drop = self.scale - places;
        let mut limbs = self.limbs;
        let mut last = 0_u64;
        for _ in 0..drop {
            last = div10(&mut limbs);
        }
        if last >= 5 {
            limbs = add_one(limbs)?;
        }
        Some(Self {
            limbs,
            scale: places,
        })
    }

    /// `self / 2^30`, rounded half away from zero to `places` decimal places.
    fn gb_at(self, places: u32) -> Option<Self> {
        if places >= self.scale {
            let num = mul_pow10(self.limbs, places - self.scale)?;
            let (mut quot, rem) = shr30(num);
            if rem >= 1 << 29 {
                quot = add_one(quot)?;
            }
            return Some(Self {
                limbs: quot,
                scale: places,
            });
        }
        let drop = self.scale - places;
        let (quot_dec, rem_dec) = divmod_pow10(self.limbs, drop);
        let (mut quot, rem_bits) = shr30(quot_dec);
        if half_up(rem_bits, rem_dec, drop) {
            quot = add_one(quot)?;
        }
        Some(Self {
            limbs: quot,
            scale: places,
        })
    }
}

impl std::fmt::Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let digits = digits_of(self.limbs);
        if self.scale == 0 {
            return f.write_str(&digits);
        }
        let scale = self.scale as usize;
        if digits.len() <= scale {
            write!(f, "0.{}{digits}", "0".repeat(scale - digits.len()))
        } else {
            let split = digits.len() - scale;
            write!(f, "{}.{}", &digits[..split], &digits[split..])
        }
    }
}

/// Product of two amounts. Exact, or `None` when either amount is negative.
/// The product always fits in 512 bits. Whether the once-rounded cents fit in a
/// decimal is `Amount::cents_fit`, checked by the caller.
pub(crate) fn exact_mul(left: Decimal, right: Decimal) -> Option<Amount> {
    let left_m = unsigned_mantissa(left)?;
    let right_m = unsigned_mantissa(right)?;
    Some(Amount {
        limbs: mul_u128(left_m, right_m),
        scale: left.scale() + right.scale(),
    })
}

/// Sum of two amounts. `None` when the sum does not fit in 512 bits.
pub(crate) fn exact_add(left: Amount, right: Amount) -> Option<Amount> {
    if left.scale == right.scale {
        return add_limbs(left.limbs, right.limbs).map(|limbs| Amount {
            limbs,
            scale: left.scale,
        });
    }
    if left.scale < right.scale {
        let limbs = mul_pow10(left.limbs, right.scale - left.scale)?;
        return add_limbs(limbs, right.limbs).map(|limbs| Amount {
            limbs,
            scale: right.scale,
        });
    }
    let limbs = mul_pow10(right.limbs, left.scale - right.scale)?;
    add_limbs(left.limbs, limbs).map(|limbs| Amount {
        limbs,
        scale: left.scale,
    })
}

fn unsigned_mantissa(value: Decimal) -> Option<u128> {
    u128::try_from(value.mantissa()).ok()
}

#[allow(clippy::cast_possible_truncation)]
fn limbs_from_u128(value: u128) -> [u64; 8] {
    let mut limbs = [0_u64; 8];
    limbs[0] = value as u64;
    limbs[1] = (value >> 64) as u64;
    limbs
}

#[allow(clippy::cast_possible_truncation)]
fn mul_u128(left: u128, right: u128) -> [u64; 8] {
    let left = [left as u64, (left >> 64) as u64];
    let right = [right as u64, (right >> 64) as u64];
    let mut out = [0_u64; 8];
    for (i, &factor) in left.iter().enumerate() {
        let mut carry = 0_u128;
        for (j, &part) in right.iter().enumerate() {
            let cell = u128::from(out[i + j]) + u128::from(factor) * u128::from(part) + carry;
            out[i + j] = cell as u64;
            carry = cell >> 64;
        }
        let mut extra = carry;
        let mut index = i + 2;
        while extra != 0 {
            let cell = u128::from(out[index]) + extra;
            out[index] = cell as u64;
            extra = cell >> 64;
            index += 1;
        }
    }
    out
}

/// Multiplies by `10^digits`. `None` when the product does not fit in 512 bits.
#[allow(clippy::cast_possible_truncation)]
fn mul_pow10(mut limbs: [u64; 8], mut digits: u32) -> Option<[u64; 8]> {
    while digits > 0 {
        let step = digits.min(19);
        digits -= step;
        limbs = mul_u64(limbs, 10_u64.pow(step))?;
    }
    Some(limbs)
}

#[allow(clippy::cast_possible_truncation)]
fn mul_u64(limbs: [u64; 8], factor: u64) -> Option<[u64; 8]> {
    let mut out = [0_u64; 8];
    let mut carry = 0_u128;
    for i in 0..8 {
        let cell = u128::from(limbs[i]) * u128::from(factor) + carry;
        out[i] = cell as u64;
        carry = cell >> 64;
    }
    if carry == 0 {
        Some(out)
    } else {
        None
    }
}

fn add_limbs(left: [u64; 8], right: [u64; 8]) -> Option<[u64; 8]> {
    let mut out = [0_u64; 8];
    let mut carry = 0_u128;
    for i in 0..8 {
        let cell = u128::from(left[i]) + u128::from(right[i]) + carry;
        #[allow(clippy::cast_possible_truncation)]
        {
            out[i] = cell as u64;
        }
        carry = cell >> 64;
    }
    if carry == 0 {
        Some(out)
    } else {
        None
    }
}

fn add_one(limbs: [u64; 8]) -> Option<[u64; 8]> {
    let mut one = [0_u64; 8];
    one[0] = 1;
    add_limbs(limbs, one)
}

/// Divides by ten and returns the remainder (0..=9).
#[allow(clippy::cast_possible_truncation)]
fn div10(limbs: &mut [u64; 8]) -> u64 {
    let mut rest = 0_u128;
    for limb in limbs.iter_mut().rev() {
        let cell = (rest << 64) | u128::from(*limb);
        *limb = (cell / 10) as u64;
        rest = cell % 10;
    }
    rest as u64
}

fn divmod_pow10(mut limbs: [u64; 8], digits: u32) -> ([u64; 8], [u64; 8]) {
    let mut rem = [0_u64; 8];
    let mut place = [0_u64; 8];
    place[0] = 1;
    for _ in 0..digits {
        let digit = div10(&mut limbs);
        if digit != 0 {
            if let Some(term) = mul_u64(place, digit) {
                if let Some(sum) = add_limbs(rem, term) {
                    rem = sum;
                }
            }
        }
        if let Some(next) = mul_u64(place, 10) {
            place = next;
        }
    }
    (limbs, rem)
}

/// `2 * (rem_dec + rem_bits * 10^drop) >= 10^drop * 2^30`, with `rem_bits < 2^30`.
fn half_up(rem_bits: u64, rem_dec: [u64; 8], drop: u32) -> bool {
    if rem_bits >= 1 << 29 {
        return true;
    }
    let Some(pow) = mul_pow10([1, 0, 0, 0, 0, 0, 0, 0], drop) else {
        return false;
    };
    let factor = (1_u64 << 30) - rem_bits * 2;
    let Some(right) = mul_u64(pow, factor) else {
        return false;
    };
    let Some(left) = mul_u64(rem_dec, 2) else {
        return true;
    };
    cmp_limbs(left, right) != Ordering::Less
}

/// Shift right by 30 bits. The remainder is the low 30 bits.
fn shr30(limbs: [u64; 8]) -> ([u64; 8], u64) {
    let rem = limbs[0] & ((1 << 30) - 1);
    let mut out = [0_u64; 8];
    for i in 0..8 {
        let next = if i + 1 < 8 { limbs[i + 1] } else { 0 };
        out[i] = (limbs[i] >> 30) | (next << 34);
    }
    (out, rem)
}

fn cmp_limbs(left: [u64; 8], right: [u64; 8]) -> Ordering {
    for i in (0..8).rev() {
        match left[i].cmp(&right[i]) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

fn fits_96(limbs: &[u64; 8]) -> bool {
    limbs[2..].iter().all(|limb| *limb == 0) && limbs[1] >> 32 == 0
}

/// A decimal equal to `amount`, dropping trailing zeros only as far as a 96-bit
/// mantissa and 28 decimals require.
#[allow(clippy::cast_possible_truncation)]
fn decimal_fitting(mut amount: Amount) -> Option<Decimal> {
    while amount.scale > 0 && (amount.scale > 28 || !fits_96(&amount.limbs)) {
        let mut next = amount.limbs;
        if div10(&mut next) != 0 {
            return None;
        }
        amount.limbs = next;
        amount.scale -= 1;
    }
    if amount.scale > 28 || !fits_96(&amount.limbs) {
        return None;
    }
    Some(Decimal::from_parts(
        amount.limbs[0] as u32,
        (amount.limbs[0] >> 32) as u32,
        amount.limbs[1] as u32,
        false,
        amount.scale,
    ))
}

fn digits_of(mut limbs: [u64; 8]) -> String {
    if limbs.iter().all(|limb| *limb == 0) {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    while limbs.iter().any(|limb| *limb != 0) {
        let digit = div10(&mut limbs);
        digits.push(b'0' + u8::try_from(digit).unwrap_or(0));
    }
    digits.reverse();
    String::from_utf8(digits).unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{
        days_from_civil, epoch_millis, exact_add, exact_mul, format_dec, format_usd, gb_from_bytes,
        loses_digits, parse_money, parse_utc_millis, too_precise, valid_date, Amount,
    };

    #[test]
    fn unix_epoch_and_a_non_leap_span() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(epoch_millis(1970, 1, 1), 0);
        let span = days_from_civil(2026, 11, 9) - days_from_civil(2025, 11, 9);
        assert_eq!(span, 365);
        assert_eq!(
            epoch_millis(2026, 11, 9) - epoch_millis(2025, 11, 9),
            365 * 86_400_000
        );
    }

    #[test]
    fn leap_day_span() {
        assert_eq!(
            days_from_civil(2024, 3, 1) - days_from_civil(2024, 2, 28),
            2
        );
        assert_eq!(
            days_from_civil(2025, 3, 1) - days_from_civil(2025, 2, 28),
            1
        );
    }

    #[test]
    fn one_gibibyte_is_one_gb() {
        let gib = Decimal::from(1_073_741_824_u64);
        assert_eq!(gb_from_bytes(gib), Decimal::ONE);
        assert_eq!(gb_from_bytes(Decimal::ZERO), Decimal::ZERO);
        let two = gb_from_bytes(gib * Decimal::from(2));
        assert_eq!(two, Decimal::from(2));
        // Half away from zero: 2 bytes is exactly ...03125, so the last digit goes up.
        assert_eq!(
            format_dec(gb_from_bytes(Decimal::from(2))),
            "0.0000000018626451492309570313"
        );
        assert_eq!(
            format_dec(gb_from_bytes(Decimal::from(10))),
            "0.0000000093132257461547851563"
        );
        assert_eq!(
            format_dec(gb_from_bytes(Decimal::ONE)),
            "0.0000000009313225746154785156"
        );
    }

    #[test]
    fn money_rejects_exponent_sign_and_empty() {
        assert!(parse_money("0.08").is_ok());
        assert!(parse_money("0").is_ok());
        assert!(parse_money("40.96").is_ok());
        assert!(parse_money("1e-7").is_err());
        assert!(parse_money("1E-7").is_err());
        assert!(parse_money("-1").is_err());
        assert!(parse_money("+1").is_err());
        assert!(parse_money(".2").is_err());
        assert!(parse_money("1.").is_err());
        assert!(parse_money("").is_err());
        assert!(parse_money(" \t0.08\r\n").is_ok());
        assert!(parse_money("\u{a0}0.08").is_err());
        assert!(parse_money("0.08\u{2028}").is_err());
        assert!(parse_money("\u{3000}0.08").is_err());
        assert!(!too_precise("0.0000000000000000000000000001"));
        assert!(too_precise("0.00000000000000000000000000001"));
        assert!(parse_money("79228162514264337593543950335").is_ok());
    }

    #[test]
    fn digits_that_do_not_fit_in_a_decimal_are_found() {
        assert!(loses_digits("1844674407370955161.49999999999"));
        assert!(loses_digits("79228162514264337593543950334.9"));
        assert!(loses_digits("1234567890123456789.1234567890123456789"));
        assert!(!loses_digits("79228162514264337593543950335"));
        assert!(!loses_digits("1844674407370955161.5"));
        assert!(!loses_digits("0001844674407370955161.50000000000"));
        assert!(!loses_digits("123456789012345678.9012345678"));
        assert!(!loses_digits("0.0000000000000000000000000001"));
        assert!(!loses_digits("0"));
        assert!(!loses_digits("0.000"));
        assert!(!loses_digits("not a number"));
    }

    #[test]
    fn usd_keeps_two_digits() {
        assert_eq!(format_usd(Decimal::ZERO), "0.00");
        assert_eq!(format_usd(Decimal::from(2)), "2.00");
        assert_eq!(format_usd("0.3".parse::<Decimal>().unwrap()), "0.30");
        assert_eq!(format_dec("10.05".parse().unwrap()), "10.05");
        assert_eq!(format_dec("40.96".parse().unwrap()), "40.96");
    }

    #[test]
    fn usd_rounds_half_away_from_zero() {
        assert_eq!(format_usd("0.004".parse::<Decimal>().unwrap()), "0.00");
        assert_eq!(format_usd("0.005".parse::<Decimal>().unwrap()), "0.01");
        assert_eq!(format_usd("0.075".parse::<Decimal>().unwrap()), "0.08");
        assert_eq!(format_usd("0.025".parse::<Decimal>().unwrap()), "0.03");
        assert_eq!(format_usd("1.005".parse::<Decimal>().unwrap()), "1.01");
        assert_eq!(
            format_usd("2080.0455".parse::<Decimal>().unwrap()),
            "2080.05"
        );
        assert_eq!(
            format_usd("3536.03185".parse::<Decimal>().unwrap()),
            "3536.03"
        );
    }

    #[test]
    fn dates_follow_the_calendar() {
        assert!(valid_date("2026-11-09"));
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2025-02-29"));
        assert!(!valid_date("2026-13-01"));
        assert!(!valid_date("2026-+1-09"));
        assert!(!valid_date("2026-01-+9"));
        assert!(!valid_date("+026-01-09"));
        assert!(!valid_date("2026-0a-09"));
        assert!(!valid_date("2026-00-01"));
        assert!(!valid_date("2026-04-31"));
        assert!(!valid_date("2026-11-9"));
        assert!(!valid_date("20261109"));
        assert!(valid_date("2000-02-29"));
        assert!(!valid_date("1900-02-29"));
    }

    #[test]
    fn utc_timestamps_match_the_age_boundaries() {
        let as_of = epoch_millis(2026, 11, 9);
        let exact_30 = parse_utc_millis("2026-10-10T00:00:00Z").unwrap();
        let plus_fraction = parse_utc_millis("2026-10-09T00:00:00.000Z").unwrap();
        let plus_offset = parse_utc_millis("2026-10-09T00:00:00+00:00").unwrap();
        let plus_second = parse_utc_millis("2026-10-09T23:59:59Z").unwrap();
        assert_eq!(as_of - exact_30, 30 * 86_400_000);
        assert_eq!(as_of - plus_fraction, 31 * 86_400_000);
        assert_eq!(plus_fraction, plus_offset);
        assert_eq!(as_of - plus_second, 30 * 86_400_000 + 1_000);
        let tenth = parse_utc_millis("2026-10-09T00:00:00.1Z").unwrap();
        assert_eq!(tenth - plus_fraction, 100);
        let trunc = parse_utc_millis("2026-10-09T00:00:00.1234Z").unwrap();
        assert_eq!(trunc - plus_fraction, 123);
    }

    #[test]
    fn utc_timestamps_reject_offsets_leap_seconds_and_years() {
        assert!(parse_utc_millis("2026-10-09T00:00:00-05:00").is_none());
        assert!(parse_utc_millis("2026-10-09T00:00:00+00:01").is_none());
        assert!(parse_utc_millis("2026-10-09T00:00:60Z").is_none());
        assert!(parse_utc_millis("1969-12-31T23:59:59Z").is_none());
        assert!(parse_utc_millis("2200-01-01T00:00:00Z").is_none());
        assert!(parse_utc_millis("1970-01-01T00:00:00Z").is_some());
        assert!(parse_utc_millis("2199-12-31T23:59:59Z").is_some());
        assert!(parse_utc_millis("2026-10-09").is_none());
        assert!(parse_utc_millis("2026-10-09 00:00:00Z").is_none());
        assert!(parse_utc_millis("2025-02-29T00:00:00Z").is_none());
        assert!(parse_utc_millis("2026-10-09T00:00:00.Z").is_none());
        assert!(parse_utc_millis("2026-10-09T00:00:0éZ").is_none());
    }

    #[test]
    fn utc_clock_fields_are_digits_only() {
        assert!(parse_utc_millis("2026-10-09T00:00:00Z").is_some());
        assert!(parse_utc_millis("2026-10-09T23:59:59Z").is_some());
        assert!(parse_utc_millis("2026-10-09T+1:+2:+3Z").is_none());
        assert!(parse_utc_millis("2026-10-09T0a:00:00Z").is_none());
        assert!(parse_utc_millis("2026-10-09T00:+0:00Z").is_none());
        assert!(parse_utc_millis("2026-10-09T00:00:+0Z").is_none());
    }

    #[test]
    fn exact_math_refuses_what_does_not_fit() {
        let dec = |text: &str| -> Decimal { text.parse().expect("decimal") };
        let add = |left: Decimal, right: Decimal| -> Option<Amount> {
            exact_add(
                Amount::from_decimal(left).expect("left"),
                Amount::from_decimal(right).expect("right"),
            )
        };
        // Fits: the scale of the operands is kept, trailing zeros included.
        assert_eq!(
            exact_mul(dec("1.50"), dec("2")).expect("fits").to_string(),
            "3.00"
        );
        let overflow = add(dec("0.08"), dec("79228162514264337593543950334")).expect("exact");
        assert!(!overflow.cents_fit());
        assert_eq!(
            add(dec("1.5"), dec("2.25")).expect("fits").to_string(),
            "3.75"
        );
        // A product that needs 33 digits stays exact and its cents do not fit a decimal.
        let rate = dec("123456789.41249");
        assert!(rate.checked_mul(Decimal::from(u64::MAX)).is_some());
        let wide = exact_mul(rate, Decimal::from(u64::MAX)).expect("exact");
        assert!(!wide.cents_fit());
        // A 28 decimal GB times a rate stays exact. Its cents are zero and fit.
        let tiny = exact_mul(dec("0.0000000009313225746154785156"), dec("0.05")).expect("exact");
        assert!(tiny.cents_fit());
        assert_eq!(format_usd(tiny), "0.00");
        assert!(tiny.gt_decimal(Decimal::ZERO));
        let padded = exact_mul(dec("0.5000000000000000000000000000"), dec("0.2")).expect("fits");
        assert!(padded.eq_decimal(dec("0.1")));
        // Both operands at the maximum: the exact sum is kept, the cents do not fit.
        assert!(!add(Decimal::MAX, Decimal::MAX).expect("exact").cents_fit());
        assert!(!add(Decimal::MAX, dec("0.1")).expect("exact").cents_fit());
        assert!(add(Decimal::MAX, Decimal::ZERO)
            .expect("fits")
            .eq_decimal(Decimal::MAX));
        assert!(exact_mul(Decimal::MAX, dec("1.0"))
            .expect("fits")
            .eq_decimal(Decimal::MAX));
        assert!(!exact_mul(Decimal::MAX, dec("2"))
            .expect("exact")
            .cents_fit());
        // A negative amount never occurs; it is refused, not computed.
        assert!(exact_mul(dec("-1"), dec("2")).is_none());
        assert!(Amount::from_decimal(dec("-1")).is_none());
        let full = Amount {
            limbs: [u64::MAX; 8],
            scale: 0,
        };
        assert!(exact_add(full, Amount::from_decimal(Decimal::ONE).expect("one")).is_none());
    }

    #[test]
    fn a_value_near_a_half_cent_rounds_once() {
        let dec = |text: &str| -> Decimal { text.parse().expect("decimal") };
        let add = |left: &str, right: &str| -> Amount {
            exact_add(
                Amount::from_decimal(dec(left)).expect("left"),
                Amount::from_decimal(dec(right)).expect("right"),
            )
            .expect("exact")
        };
        // Product: 1001 * 0.4106043956043956043956043956 = 411.0149999999999999999999999956.
        let product = exact_mul(
            Decimal::from(1001_u32),
            dec("0.4106043956043956043956043956"),
        )
        .expect("exact");
        assert_eq!(format_usd(product), "411.01");
        // Product: 1000000000000038 * 0.9999999999999620150000000014 is just over ...00.0149.
        let product = exact_mul(
            Decimal::from(1_000_000_000_000_038_u64),
            dec("0.9999999999999620150000000014"),
        )
        .expect("exact");
        assert_eq!(format_usd(product), "1000000000000000.01");
        // Sum: 1000000000.0149999999999999999 + 0.00000000000000000009.
        let sum = add("1000000000.0149999999999999999", "0.00000000000000000009");
        assert_eq!(format_usd(sum), "1000000000.01");
        // Just over the half cent rounds away from zero.
        let sum = add("1000000000.0150000000000000001", "0.00000000000000000009");
        assert_eq!(format_usd(sum), "1000000000.02");
        let far = add("1000000000.0123456789012345678", "0.00000000000000000009");
        assert_eq!(format_usd(far), "1000000000.01");
        let tiny = exact_mul(dec("0.0000000009313225746154785156"), dec("0.05")).expect("exact");
        assert_eq!(format_usd(tiny), "0.00");
    }
}
