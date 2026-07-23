//! Shared bounded regular-expression helpers for deterministic inspectors.

use regex::Regex;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct SafeMatch {
    pub start: usize,
    pub end: usize,
    pub redacted: String,
    pub sha256: String,
}

pub fn compile(patterns: &[String]) -> Result<Vec<Regex>, regex::Error> {
    patterns
        .iter()
        .filter(|pattern| pattern.len() <= 4096)
        .map(|pattern| Regex::new(pattern))
        .collect()
}

pub fn find(patterns: &[Regex], text: &str, maximum: usize) -> Vec<SafeMatch> {
    let mut matches = patterns
        .iter()
        .flat_map(|pattern| pattern.find_iter(text))
        .map(|value| safe_match(text, value.start(), value.end()))
        .take(maximum)
        .collect::<Vec<_>>();
    matches.sort_by_key(|value| (value.start, value.end));
    matches.dedup_by_key(|value| (value.start, value.end));
    matches
}

pub fn safe_match(text: &str, start: usize, end: usize) -> SafeMatch {
    let value = &text[start..end];
    let prefix = value.chars().take(4).collect::<String>();
    SafeMatch {
        start,
        end,
        redacted: format!("{prefix}…[REDACTED]"),
        sha256: format!("{:x}", Sha256::digest(value.as_bytes())),
    }
}
