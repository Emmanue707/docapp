use crate::types::{FieldType, TypedGuess};

const MONTHS: &[(&str, u8)] = &[
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("may", 5),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

pub fn predict_type(text: &str) -> TypedGuess {
    if let Some(g) = try_date(text) {
        return g;
    }
    if let Some(g) = try_currency(text) {
        return g;
    }
    if let Some(g) = try_email(text) {
        return g;
    }
    if let Some(g) = try_code_id(text) {
        return g;
    }
    if let Some(g) = try_number(text) {
        return g;
    }
    TypedGuess {
        field_type: FieldType::Text,
        normalized: None,
        score: 0.2,
    }
}

fn try_date(text: &str) -> Option<TypedGuess> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(iso) = parse_numeric_date(t) {
        return Some(TypedGuess {
            field_type: FieldType::Date,
            normalized: Some(iso),
            score: 0.95,
        });
    }
    if let Some(iso) = parse_named_month_date(t) {
        return Some(TypedGuess {
            field_type: FieldType::Date,
            normalized: Some(iso),
            score: 0.9,
        });
    }
    None
}

fn parse_numeric_date(t: &str) -> Option<String> {
    let sep = t.find(['/', '-', '.'])?;
    let sep_ch = t.as_bytes()[sep] as char;
    let parts: Vec<&str> = t.split(sep_ch).map(str::trim).collect();
    if parts.len() != 3 {
        return None;
    }
    let a = parse_u16(parts[0])?;
    let b = parse_u16(parts[1])?;
    let mut c = parse_u16(parts[2])?;
    let (y, m, d) = if parts[0].len() == 4 {
        (a, b, c)
    } else if parts[2].len() == 4 || parts[2].len() == 2 {
        if parts[2].len() == 2 {
            c = expand_year(c);
        }
        if a > 31 {
            (a, b, c)
        } else if b > 12 && a <= 12 {
            (c, a, b)
        } else {
            (c, b, a)
        }
    } else {
        return None;
    };
    if valid_ymd(y, m, d) {
        return Some(format!("{y:04}-{m:02}-{d:02}"));
    }
    None
}

fn parse_named_month_date(t: &str) -> Option<String> {
    let lower = t.to_ascii_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.len() < 3 {
        return None;
    }
    for window in tokens.windows(3) {
        if let Some((d, m, y)) = parse_d_mon_y(window) {
            return Some(format!("{y:04}-{m:02}-{d:02}"));
        }
        if let Some((d, m, y)) = parse_mon_d_y(window) {
            return Some(format!("{y:04}-{m:02}-{d:02}"));
        }
    }
    None
}

fn parse_d_mon_y(w: &[&str]) -> Option<(u16, u16, u16)> {
    let d = parse_u16(w[0])?;
    let m = month_num(w[1])?;
    let mut y = parse_u16(w[2])?;
    if w[2].len() == 2 {
        y = expand_year(y);
    }
    valid_ymd(y, m, d).then_some((d, m, y))
}

fn parse_mon_d_y(w: &[&str]) -> Option<(u16, u16, u16)> {
    let m = month_num(w[0])?;
    let d = parse_u16(w[1])?;
    let mut y = parse_u16(w[2])?;
    if w[2].len() == 2 {
        y = expand_year(y);
    }
    valid_ymd(y, m, d).then_some((d, m, y))
}

fn month_num(s: &str) -> Option<u16> {
    let key = &s[..s.len().min(3)];
    MONTHS
        .iter()
        .find(|(name, _)| name.starts_with(key))
        .map(|(_, m)| *m as u16)
}

fn expand_year(y: u16) -> u16 {
    if y >= 70 { 1900 + y } else { 2000 + y }
}

fn valid_ymd(y: u16, m: u16, d: u16) -> bool {
    if y < 1 || !(1..=12).contains(&m) || d < 1 {
        return false;
    }
    let mdays = days_in_month(y, m);
    d <= mdays
}

fn days_in_month(y: u16, m: u16) -> u16 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn is_leap(y: u16) -> bool {
    let y = y as u32;
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn parse_u16(s: &str) -> Option<u16> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn try_currency(text: &str) -> Option<TypedGuess> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let upper = t.to_ascii_uppercase();
    let has_marker = t.contains('$')
        || t.contains('€')
        || t.contains('£')
        || t.contains(',')
        || t.starts_with('-')
        || t.starts_with('(')
        || upper.contains("PGK")
        || upper.contains("USD")
        || upper.contains("AUD")
        || upper.contains("EUR")
        || upper.contains("GBP")
        || t.contains('K') && t.bytes().any(|b| b.is_ascii_digit());
    if !has_marker {
        return None;
    }
    let mut code = String::new();
    let mut amount_str = t.to_owned();
    let negative = t.starts_with('-') || t.starts_with('(');
    for c in ["PGK", "USD", "AUD", "EUR", "GBP", "K"] {
        if t.to_ascii_uppercase().contains(c) {
            code = c.to_owned();
        }
    }
    if t.contains('€') {
        code = "EUR".to_owned();
    }
    if t.contains('£') {
        code = "GBP".to_owned();
    }
    if t.contains('$') && code.is_empty() {
        code = "USD".to_owned();
    }
    amount_str = amount_str.replace(['$', '€', '£', ','], "");
    for word in [
        "PGK", "USD", "AUD", "EUR", "GBP", "pgk", "usd", "aud", "K", "k",
    ] {
        amount_str = amount_str.replace(word, "");
    }
    let amount_str = amount_str.split_whitespace().collect::<String>();
    let amount: f64 = amount_str.parse().ok()?;
    if !amount_str.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let sign = if negative { "-" } else { "" };
    let norm = format!("{sign}{amount:.2}");
    let full = if code.is_empty() {
        norm
    } else {
        format!("{code} {norm}")
    };
    Some(TypedGuess {
        field_type: FieldType::Currency,
        normalized: Some(full),
        score: if code.is_empty() { 0.6 } else { 0.92 },
    })
}

fn try_email(text: &str) -> Option<TypedGuess> {
    let t = text.trim();
    let at = t.find('@')?;
    if at == 0 || at + 1 >= t.len() {
        return None;
    }
    let local = &t[..at];
    let domain = &t[at + 1..];
    if local.is_empty() || domain.is_empty() {
        return None;
    }
    if !domain.contains('.') {
        return None;
    }
    if !local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
    {
        return None;
    }
    if !domain
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
    {
        return None;
    }
    Some(TypedGuess {
        field_type: FieldType::Email,
        normalized: Some(t.to_ascii_lowercase()),
        score: 0.93,
    })
}

fn try_code_id(text: &str) -> Option<TypedGuess> {
    let t = text.trim();
    let len = t.len();
    if !(4..=24).contains(&len) {
        return None;
    }
    let mut has_digit = false;
    for c in t.chars() {
        if c.is_ascii_digit() {
            has_digit = true;
        } else if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
            return None;
        }
    }
    if !has_digit {
        return None;
    }
    Some(TypedGuess {
        field_type: FieldType::CodeId,
        normalized: Some(t.to_ascii_uppercase()),
        score: 0.85,
    })
}

fn try_number(text: &str) -> Option<TypedGuess> {
    let t = text.trim().replace(',', "");
    if t.is_empty() {
        return None;
    }
    let mut s = t.as_str();
    let negative = if let Some(rest) = s.strip_prefix('-') {
        s = rest;
        true
    } else {
        false
    };
    if !s.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    let _v: f64 = s.parse().ok()?;
    let norm = if negative {
        format!("-{s}")
    } else {
        s.to_owned()
    };
    Some(TypedGuess {
        field_type: FieldType::Number,
        normalized: Some(norm),
        score: 0.7,
    })
}

pub fn apply_confusions_for_scoring(text: &str, field: FieldType) -> String {
    match field {
        FieldType::Date | FieldType::Number | FieldType::CodeId => text
            .chars()
            .map(|c| match c {
                'O' | 'o' => '0',
                'l' | 'I' => '1',
                'S' | 's' => '5',
                'B' => '8',
                other => other,
            })
            .collect(),
        _ => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_table() {
        let cases = [
            ("31/01/2024", "2024-01-31"),
            ("29-02-2024", "2024-02-29"),
            ("2024-02-29", "2024-02-29"),
            ("12 Jan 2026", "2026-01-12"),
            ("Jan 12, 2026", "2026-01-12"),
        ];
        for (raw, iso) in cases {
            let g = predict_type(raw);
            assert_eq!(g.field_type, FieldType::Date, "{raw}");
            assert_eq!(g.normalized.as_deref(), Some(iso), "{raw}");
        }
    }

    #[test]
    fn invalid_date_feb_non_leap() {
        let g = predict_type("29/02/2023");
        assert_ne!(g.field_type, FieldType::Date);
    }

    #[test]
    fn currency_table() {
        let g = predict_type("K 1,200.50");
        assert_eq!(g.field_type, FieldType::Currency);
        assert!(g.normalized.as_ref().is_some_and(|n| n.contains("1200.50")));
        let g2 = predict_type("PGK 50.00");
        assert_eq!(g2.field_type, FieldType::Currency);
        let g3 = predict_type("-$12.50");
        assert_eq!(g3.field_type, FieldType::Currency);
    }

    #[test]
    fn email_and_code() {
        let e = predict_type("user.name+tag@example.co.uk");
        assert_eq!(e.field_type, FieldType::Email);
        let c = predict_type("INV-2026-0042");
        assert_eq!(c.field_type, FieldType::CodeId);
        let c2 = predict_type("AB123456");
        assert_eq!(c2.field_type, FieldType::CodeId);
    }

    #[test]
    fn ambiguous_defaults_text_or_number() {
        let n = predict_type("42");
        assert_eq!(n.field_type, FieldType::Number);
        let t = predict_type("hello");
        assert_eq!(t.field_type, FieldType::Text);
    }
}
