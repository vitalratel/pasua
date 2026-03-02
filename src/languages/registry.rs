// ABOUTME: Language registry — maps file extension to LanguageSupport implementation.
// ABOUTME: Returns None for unknown extensions; they are silently skipped.

use super::{go::Go, LanguageSupport};

/// Return the language support for a given file extension, or None if unsupported.
pub fn for_extension(ext: &str) -> Option<Box<dyn LanguageSupport>> {
    match ext {
        "go" => Some(Box::new(Go)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_is_registered() {
        assert!(for_extension("go").is_some());
    }

    #[test]
    fn unknown_returns_none() {
        assert!(for_extension("xyz").is_none());
        assert!(for_extension("").is_none());
    }
}
