//! Date/time formatting using Qt format strings (`yyyy-MM-dd HH:mm:ss.zzz`), as used by Beeftext's
//! `#{dateTime:...}` variable, plus the `w`/`ww` week number extension and date/time shifts (`+1d-2h`).

use chrono::{DateTime, Datelike, Duration, Local, Locale, Months, Timelike};

/// The locale of the system, used for day and month names.
pub fn system_locale() -> Locale {
    let tag = sys_locale::get_locale().unwrap_or_else(|| "en-US".into());
    locale_from_tag(&tag)
}

pub fn locale_from_tag(tag: &str) -> Locale {
    let tag = tag.split(['.', '@']).next().unwrap_or("en_US").replace('-', "_");
    if let Ok(l) = Locale::try_from(tag.as_str()) {
        return l;
    }
    let lang = tag.split('_').next().unwrap_or("en").to_lowercase();
    let guess = format!("{lang}_{}", lang.to_uppercase());
    Locale::try_from(guess.as_str()).unwrap_or(Locale::en_US)
}

/// Default (locale-dependent) representations, used by `#{date}`, `#{time}` and `#{dateTime}` on platforms
/// that do not provide a native formatter.
pub fn default_date(dt: &DateTime<Local>, locale: Locale) -> String {
    dt.format_localized("%x", locale).to_string()
}

pub fn default_time(dt: &DateTime<Local>, locale: Locale) -> String {
    dt.format_localized("%X", locale).to_string()
}

pub fn default_date_time(dt: &DateTime<Local>, locale: Locale) -> String {
    format!("{} {}", default_date(dt, locale), default_time(dt, locale))
}

/// Apply a shift string such as `+1d-1w+3h` to a date/time. Invalid parts, and shifts that would leave the
/// range of dates (`+100000000d`), are ignored.
pub fn apply_shift(dt: DateTime<Local>, shifts: &str) -> DateTime<Local> {
    let mut result = dt;
    for (sign, value, unit) in parse_shifts(shifts).unwrap_or_default() {
        let Ok(v) = i64::try_from(value) else { continue };
        let v = if sign { v } else { -v };
        result = match unit {
            'y' => add_months(result, v.saturating_mul(12)),
            'M' => add_months(result, v),
            _ => {
                let delta = match unit {
                    'w' => Duration::try_weeks(v),
                    'd' => Duration::try_days(v),
                    'h' => Duration::try_hours(v),
                    'm' => Duration::try_minutes(v),
                    's' => Duration::try_seconds(v),
                    'z' => Duration::try_milliseconds(v),
                    _ => None,
                };
                delta.and_then(|d| result.checked_add_signed(d)).unwrap_or(result)
            }
        };
    }
    result
}

fn add_months(dt: DateTime<Local>, months: i64) -> DateTime<Local> {
    let m = Months::new(months.unsigned_abs().min(u32::MAX as u64) as u32);
    let r = if months >= 0 { dt.checked_add_months(m) } else { dt.checked_sub_months(m) };
    r.unwrap_or(dt)
}

/// Parse a complete shift sequence. Returns None if the string is not entirely made of shifts.
pub fn parse_shifts(s: &str) -> Option<Vec<(bool, u64, char)>> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let sign = match chars[i] {
            '+' => true,
            '-' => false,
            _ => return None,
        };
        i += 1;
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        if i == start || i >= chars.len() {
            return None;
        }
        let value: u64 = chars[start..i].iter().collect::<String>().parse().ok()?;
        let unit = chars[i];
        if !"yMwdhmsz".contains(unit) {
            return None;
        }
        i += 1;
        out.push((sign, value, unit));
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Split the parameter of a `dateTime:` variable into an optional shift and a format.
/// Mirrors the regular expression `^dateTime(:(([+-]\d+[yMwdhmsz])+))?:(.*)$` used by Beeftext.
pub fn split_shift_and_format(param: &str) -> (Option<&str>, &str) {
    if let Some(colon) = param.find(':') {
        let candidate = &param[..colon];
        if parse_shifts(candidate).is_some() {
            return (Some(candidate), &param[colon + 1..]);
        }
    }
    (None, param)
}

/// Format a date/time using a Qt format string.
pub fn format_qt(dt: &DateTime<Local>, fmt: &str, locale: Locale) -> String {
    let chars: Vec<char> = fmt.chars().collect();
    let ampm = has_ampm(&chars);
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            // quoted text; '' is a literal quote
            if i + 1 < chars.len() && chars[i + 1] == '\'' {
                out.push('\'');
                i += 2;
                continue;
            }
            i += 1;
            while i < chars.len() {
                if chars[i] == '\'' {
                    if i + 1 < chars.len() && chars[i + 1] == '\'' {
                        out.push('\'');
                        i += 2;
                        continue;
                    }
                    break;
                }
                out.push(chars[i]);
                i += 1;
            }
            i += 1;
            continue;
        }
        let mut run = 1;
        while i + run < chars.len() && chars[i + run] == c {
            run += 1;
        }
        let used = match c {
            'd' => {
                let n = run.min(4);
                match n {
                    4 => out.push_str(&dt.format_localized("%A", locale).to_string()),
                    3 => out.push_str(&dt.format_localized("%a", locale).to_string()),
                    2 => out.push_str(&format!("{:02}", dt.day())),
                    _ => out.push_str(&dt.day().to_string()),
                }
                n
            }
            'M' => {
                let n = run.min(4);
                match n {
                    4 => out.push_str(&dt.format_localized("%B", locale).to_string()),
                    3 => out.push_str(&dt.format_localized("%b", locale).to_string()),
                    2 => out.push_str(&format!("{:02}", dt.month())),
                    _ => out.push_str(&dt.month().to_string()),
                }
                n
            }
            'y' => {
                if run >= 4 {
                    out.push_str(&format!("{:04}", dt.year()));
                    4
                } else if run >= 2 {
                    out.push_str(&format!("{:02}", dt.year().rem_euclid(100)));
                    2
                } else {
                    out.push('y');
                    1
                }
            }
            'h' => {
                let n = run.min(2);
                let h = if ampm {
                    let h = dt.hour() % 12;
                    if h == 0 { 12 } else { h }
                } else {
                    dt.hour()
                };
                out.push_str(&if n == 2 { format!("{h:02}") } else { h.to_string() });
                n
            }
            'H' => {
                let n = run.min(2);
                out.push_str(&if n == 2 { format!("{:02}", dt.hour()) } else { dt.hour().to_string() });
                n
            }
            'm' => {
                let n = run.min(2);
                out.push_str(&if n == 2 { format!("{:02}", dt.minute()) } else { dt.minute().to_string() });
                n
            }
            's' => {
                let n = run.min(2);
                out.push_str(&if n == 2 { format!("{:02}", dt.second()) } else { dt.second().to_string() });
                n
            }
            'z' => {
                let ms = dt.timestamp_subsec_millis().min(999);
                if run >= 3 {
                    out.push_str(&format!("{ms:03}"));
                    3
                } else {
                    let s = format!("{ms:03}");
                    let t = s.trim_end_matches('0');
                    out.push_str(if t.is_empty() { "0" } else { t });
                    1
                }
            }
            'a' | 'A' => {
                let upper = c == 'A';
                let text = if dt.hour() < 12 { "AM" } else { "PM" };
                out.push_str(&if upper { text.to_string() } else { text.to_lowercase() });
                // consume an optional following p/P
                if i + 1 < chars.len() && (chars[i + 1] == 'p' || chars[i + 1] == 'P') { 2 } else { 1 }
            }
            't' => {
                out.push_str(&dt.format("%:z").to_string());
                run
            }
            'w' => {
                let week = dt.iso_week().week();
                if run >= 2 {
                    out.push_str(&format!("{week:02}"));
                    2
                } else {
                    out.push_str(&week.to_string());
                    1
                }
            }
            _ => {
                out.push(c);
                1
            }
        };
        i += used.max(1);
    }
    out
}

fn has_ampm(chars: &[char]) -> bool {
    let mut in_quote = false;
    for &c in chars {
        if c == '\'' {
            in_quote = !in_quote;
        } else if !in_quote && (c == 'a' || c == 'A') {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample() -> DateTime<Local> {
        Local.with_ymd_and_hms(2018, 1, 29, 20, 35, 12).unwrap() + Duration::milliseconds(120)
    }

    #[test]
    fn basic_format() {
        let dt = sample();
        assert_eq!(format_qt(&dt, "yyyy-MM-dd, HH:mm:ss.zzz", Locale::en_US), "2018-01-29, 20:35:12.120");
        assert_eq!(format_qt(&dt, "d/M/yy h:m:s", Locale::en_US), "29/1/18 20:35:12");
        assert_eq!(format_qt(&dt, "h:mm AP", Locale::en_US), "8:35 PM");
        assert_eq!(format_qt(&dt, "hh:mm ap", Locale::en_US), "08:35 pm");
        assert_eq!(format_qt(&dt, "z", Locale::en_US), "12");
        assert_eq!(format_qt(&dt, "'Year:' yyyy 'it''s'", Locale::en_US), "Year: 2018 it's");
    }

    #[test]
    fn localized_names() {
        let dt = sample();
        assert_eq!(format_qt(&dt, "dddd d MMMM yyyy", Locale::en_US), "Monday 29 January 2018");
        assert_eq!(format_qt(&dt, "dddd d MMMM yyyy", Locale::it_IT), "lunedì 29 gennaio 2018");
        assert_eq!(format_qt(&dt, "ddd MMM", Locale::fr_FR), "lun. janv.");
    }

    #[test]
    fn week_numbers() {
        let dt = Local.with_ymd_and_hms(2018, 2, 7, 12, 0, 0).unwrap();
        assert_eq!(format_qt(&dt, "w", Locale::en_US), "6");
        assert_eq!(format_qt(&dt, "ww", Locale::en_US), "06");
    }

    #[test]
    fn shifts() {
        assert_eq!(split_shift_and_format("+1d-1y+2h:yyyy"), (Some("+1d-1y+2h"), "yyyy"));
        assert_eq!(split_shift_and_format("+1d:"), (Some("+1d"), ""));
        assert_eq!(split_shift_and_format("yyyy-MM-dd"), (None, "yyyy-MM-dd"));
        assert_eq!(split_shift_and_format("+1d"), (None, "+1d"));
        assert_eq!(split_shift_and_format("1d:yyyy"), (None, "1d:yyyy"));
        let dt = Local.with_ymd_and_hms(2020, 1, 31, 10, 0, 0).unwrap();
        let s = apply_shift(dt, "+1M");
        assert_eq!((s.month(), s.day()), (2, 29));
        let s = apply_shift(dt, "+1d-1y+2h");
        assert_eq!((s.year(), s.month(), s.day(), s.hour()), (2019, 2, 1, 12));
        // out of the range of dates: ignored, no crash
        for huge in ["+100000000d", "-100000000d", "+9999999999999w", "+9223372036854775807y", "+18446744073709551615s"] {
            assert_eq!(apply_shift(dt, huge), dt, "{huge}");
        }
        assert_eq!(apply_shift(dt, "+100000000d+1d").day(), 1);
    }

    #[test]
    fn locale_tags() {
        assert_eq!(locale_from_tag("it-IT"), Locale::it_IT);
        assert_eq!(locale_from_tag("it"), Locale::it_IT);
        assert_eq!(locale_from_tag("fr_FR.UTF-8"), Locale::fr_FR);
        assert_eq!(locale_from_tag("zz"), Locale::en_US);
    }
}
