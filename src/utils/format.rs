use regex::Regex;
use serde::{Deserialize, Deserializer};
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

/// Parses String into Vec<i64>, useful in params
pub fn parse_numbers<'de, D>(deserializer: D) -> Result<Vec<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.split(',')
        .map(|num| num.trim().parse::<i64>().map_err(serde::de::Error::custom))
        .collect()
}
