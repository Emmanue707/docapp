use std::collections::HashMap;

pub fn cmap_hex_to_string(hex: &str) -> String {
    let h: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if h.len() < 4 {
        return String::new();
    }
    if h.len() == 4 {
        let code = u32::from_str_radix(&h, 16).unwrap_or(0);
        return char::from_u32(code)
            .map(|c| c.to_string())
            .unwrap_or_default();
    }
    let mut out = String::new();
    let mut i = 0usize;
    while i + 3 < h.len() {
        let unit = u32::from_str_radix(&h[i..i + 4], 16).unwrap_or(0);
        if let Some(c) = char::from_u32(unit) {
            out.push(c);
        }
        i += 4;
    }
    out
}

pub fn glyph_name_to_unicode(name: &str) -> String {
    match name {
        "space" => " ".to_owned(),
        "fi" => "fi".to_owned(),
        "fl" => "fl".to_owned(),
        "ff" => "ff".to_owned(),
        "ffi" => "ffi".to_owned(),
        "ffl" => "ffl".to_owned(),
        "quoteright" | "quotesingle" => "\u{2019}".to_owned(),
        "quoteleft" => "\u{2018}".to_owned(),
        "quotedblright" => "\u{201D}".to_owned(),
        "quotedblleft" => "\u{201C}".to_owned(),
        "endash" => "\u{2013}".to_owned(),
        "emdash" => "\u{2014}".to_owned(),
        "bullet" => "\u{2022}".to_owned(),
        "ampersand" => "&".to_owned(),
        "asterisk" => "*".to_owned(),
        "period" => ".".to_owned(),
        "comma" => ",".to_owned(),
        "hyphen" => "-".to_owned(),
        "colon" => ":".to_owned(),
        "semicolon" => ";".to_owned(),
        "question" => "?".to_owned(),
        "at" => "@".to_owned(),
        "slash" => "/".to_owned(),
        "backslash" => "\\".to_owned(),
        "parenleft" => "(".to_owned(),
        "parenright" => ")".to_owned(),
        "bracketleft" => "[".to_owned(),
        "bracketright" => "]".to_owned(),
        "braceleft" => "{".to_owned(),
        "braceright" => "}".to_owned(),
        "zero" => "0".to_owned(),
        "one" => "1".to_owned(),
        "two" => "2".to_owned(),
        "three" => "3".to_owned(),
        "four" => "4".to_owned(),
        "five" => "5".to_owned(),
        "six" => "6".to_owned(),
        "seven" => "7".to_owned(),
        "eight" => "8".to_owned(),
        "nine" => "9".to_owned(),
        "less" => "<".to_owned(),
        "greater" => ">".to_owned(),
        "equal" => "=".to_owned(),
        "plus" => "+".to_owned(),
        "minus" => "-".to_owned(),
        "underscore" => "_".to_owned(),
        "dollar" => "$".to_owned(),
        "percent" => "%".to_owned(),
        "bar" => "|".to_owned(),
        "asciitilde" => "~".to_owned(),
        "asciicircum" => "^".to_owned(),
        "grave" => "`".to_owned(),
        _ if name.len() == 1 => name.to_owned(),
        _ if name.starts_with("uni") && name.len() >= 7 => {
            let hex = &name[3..];
            cmap_hex_to_string(hex)
        }
        _ if name.len() <= 3 && name.chars().all(|c| c.is_ascii_alphabetic()) => name.to_owned(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmap_hex_ligature_fi() {
        assert_eq!(cmap_hex_to_string("00660069"), "fi");
    }

    #[test]
    fn glyph_ligatures() {
        assert_eq!(glyph_name_to_unicode("fi"), "fi");
        assert_eq!(glyph_name_to_unicode("endash"), "\u{2013}");
    }
}
