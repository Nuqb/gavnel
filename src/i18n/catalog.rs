use super::{Language, languages::BUNDLED};
use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};
type Catalog = HashMap<String, String>;
// Parse only selected catalogs; adding locales does not parse them all at startup.
static CATALOGS: LazyLock<Vec<OnceLock<Catalog>>> =
    LazyLock::new(|| (0..BUNDLED.len()).map(|_| OnceLock::new()).collect());

pub(super) fn entries(language: Language) -> &'static Catalog {
    CATALOGS[language.ordinal()].get_or_init(|| {
        serde_json::from_str(BUNDLED[language.ordinal()]).expect("validated bundled catalog")
    })
}

pub(super) fn lookup(language: Language, key: &str) -> Option<&'static str> {
    let base = match language {
        Language::EnglishGb => Language::EnglishUs,
        Language::SpanishMexico => Language::SpanishSpain,
        Language::PortuguesePortugal => Language::PortugueseBrazil,
        value => value,
    };
    entries(language)
        .get(key)
        .or_else(|| entries(base).get(key))
        .map(String::as_str)
}
