//! Small shared helpers.

/// Formats seconds as `m:ss` (or `h:mm:ss` from one hour), e.g. `72.4 -> "1:12"`.
/// Negative and non-finite values render as `0:00`.
pub fn format_time(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 {
        seconds as u64
    } else {
        0
    };
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Combining accents and Arabic vowel marks / tatweel, dropped by [`fold`].
fn is_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x064B..=0x065F | 0x0670 | 0x0640)
}

/// The plain letter(s) for an accented Latin letter or an Arabic/Persian variant (lowercase in).
fn base_of(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'ç' | 'ć' | 'č' | 'ĉ' => "c",
        'ď' | 'đ' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => "e",
        'ğ' | 'ĝ' | 'ģ' => "g",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => "i",
        'ķ' => "k",
        'ł' | 'ľ' | 'ļ' => "l",
        'ñ' | 'ń' | 'ň' | 'ņ' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'ř' => "r",
        'ś' | 'š' | 'ş' | 'ș' => "s",
        'ť' | 'ţ' | 'ț' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' | 'ų' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'ß' => "ss",
        'æ' => "ae",
        'œ' => "oe",
        'ی' | 'ى' => "ي",
        'ک' => "ك",
        'ة' => "ه",
        'أ' | 'إ' | 'آ' | 'ٱ' => "ا",
        _ => return None,
    })
}

/// Text folded for loose matching: lowercase, Latin accents removed (Turkish İ/ı/ş/ğ/ç/ö/ü
/// included) and Arabic/Persian letter variants unified.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.to_lowercase().chars().filter(|c| !is_mark(*c)) {
        match base_of(c) {
            Some(b) => out.push_str(b),
            None => out.push(c),
        }
    }
    out
}

// Local-only tests (see build.rs).
#[cfg(all(test, private_tests))]
mod fold_tests {
    include!("util_fold_tests.rs");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_minutes_and_hours() {
        assert_eq!(format_time(72.4), "1:12");
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(3725.0), "1:02:05");
        assert_eq!(format_time(-3.0), "0:00");
        assert_eq!(format_time(f64::NAN), "0:00");
    }
}
