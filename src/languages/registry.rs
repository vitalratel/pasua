// ABOUTME: Language registry — maps file extension to LanguageSupport implementation.
// ABOUTME: Returns None for unknown extensions; they are silently skipped.

use super::{
    LanguageSupport,
    elixir::Elixir,
    gleam::Gleam,
    go::Go,
    python::Python,
    rust::Rust,
    typescript::{Tsx, TypeScript},
};

fn all() -> Vec<Box<dyn LanguageSupport>> {
    vec![
        Box::new(Go),
        Box::new(Rust),
        Box::new(Elixir),
        Box::new(Gleam),
        Box::new(Python),
        Box::new(TypeScript),
        Box::new(Tsx),
    ]
}

/// Return the language support for a given file extension, or None if unsupported.
pub fn for_extension(ext: &str) -> Option<Box<dyn LanguageSupport>> {
    all()
        .into_iter()
        .find(|lang| lang.extensions().contains(&ext))
}
