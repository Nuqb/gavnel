//! Offline UI localization. English source keys remain stable across regional catalogs.
mod catalog;
#[cfg(test)]
mod tests;

mod languages;
pub use languages::Language;

use std::cell::Cell;

thread_local! { static CURRENT: Cell<Language> = const { Cell::new(Language::EnglishUs) }; }

/// Restores the previous language even if rendering unwinds. Windows and tests
/// do not share mutable process-wide language state.
pub fn with_language<T>(language: Language, render: impl FnOnce() -> T) -> T {
    struct Reset(Language);
    impl Drop for Reset {
        fn drop(&mut self) {
            CURRENT.with(|current| current.set(self.0));
        }
    }
    let _reset = Reset(CURRENT.with(|current| current.replace(language)));
    render()
}

/// Translate only application-owned text; never pass paths, device names,
/// executable names, user-entered text, or shortcut tokens through this function.
pub fn tr(source: &str) -> &str {
    CURRENT
        .with(|language| catalog::lookup(language.get(), source))
        .unwrap_or(source)
}

/// Named placeholders let translators reorder whole messages without changing
/// variables. Values are inserted once, so braces inside a filename stay literal.
pub fn message(source: &str, values: &[(&str, &str)]) -> String {
    let template = tr(source);
    let mut result = String::with_capacity(template.len());
    let mut remaining = template;
    while let Some(start) = remaining.find('{') {
        result.push_str(&remaining[..start]);
        let Some(end) = remaining[start..].find('}') else {
            result.push_str(&remaining[start..]);
            return result;
        };
        let key = &remaining[start + 1..start + end];
        if let Some((_, value)) = values.iter().find(|(name, _)| *name == key) {
            result.push_str(value);
        } else {
            result.push_str(&remaining[start..=start + end]);
        }
        remaining = &remaining[start + end + 1..];
    }
    result.push_str(remaining);
    result
}

pub fn current_language() -> Language {
    CURRENT.with(Cell::get)
}

pub fn is_rtl() -> bool {
    current_language().is_rtl()
}
