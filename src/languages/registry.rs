// ABOUTME: Language registry — maps file extension to LanguageSupport implementation.
// ABOUTME: Returns None for unknown extensions; they are silently skipped.

use super::{LanguageSupport, go::Go};

fn all() -> Vec<Box<dyn LanguageSupport>> {
    vec![Box::new(Go)]
}

/// Return the language support for a given file extension, or None if unsupported.
pub fn for_extension(ext: &str) -> Option<Box<dyn LanguageSupport>> {
    all()
        .into_iter()
        .find(|lang| lang.extensions().contains(&ext))
}
