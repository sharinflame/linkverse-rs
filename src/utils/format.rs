use regex::Regex;
use unicode_normalization::UnicodeNormalization;

pub fn normalize_tag(tag: &str) -> String {
    let tag = tag.trim().to_lowercase();
    let tag = tag.nfkd().collect::<String>();

    let re_non_word = Regex::new(r"[^\w\s-]").unwrap();
    let tag = re_non_word.replace_all(&tag, "");

    let re_spaces = Regex::new(r"[\s]+").unwrap();
    let tag = re_spaces.replace_all(&tag, "-");

    tag.chars().take(50).collect()
}
