use super::*;
use std::collections::BTreeSet;

fn placeholders(value: &str) -> BTreeSet<&str> {
    value
        .split('{')
        .skip(1)
        .filter_map(|part| part.split_once('}').map(|(name, _)| name))
        .collect()
}

#[test]
fn every_language_has_complete_nonempty_catalog_and_identical_placeholders() {
    let source = catalog::entries(Language::EnglishUs);
    assert!(source.len() >= 290);
    for language in Language::ALL {
        for (key, english) in source {
            let value = catalog::lookup(language, key)
                .unwrap_or_else(|| panic!("{language:?}: missing {key}"));
            assert!(!value.trim().is_empty(), "{language:?}: {key}");
            assert_eq!(
                placeholders(english),
                placeholders(value),
                "{language:?}: {key}"
            );
            assert!(!value.contains('\u{fffd}'), "{language:?}: {key}");
        }
        for key in catalog::entries(language).keys() {
            assert!(source.contains_key(key), "{language:?}: unknown key {key}");
        }
    }
}

#[test]
fn regional_variants_use_their_own_terms_and_native_names() {
    with_language(Language::SpanishSpain, || {
        assert_eq!(tr("PC audio"), "Audio del ordenador")
    });
    with_language(Language::SpanishMexico, || {
        assert_eq!(tr("PC audio"), "Audio de la computadora")
    });
    with_language(Language::PortugueseBrazil, || {
        assert_eq!(tr("Whole screen"), "Tela inteira")
    });
    with_language(Language::PortuguesePortugal, || {
        assert_eq!(tr("Whole screen"), "Ecrã inteiro")
    });
    let names: BTreeSet<_> = Language::ALL
        .iter()
        .map(|language| language.name())
        .collect();
    assert_eq!(names.len(), Language::ALL.len());
}

#[test]
fn named_values_are_not_retranslated_and_scopes_restore_after_unwinding() {
    with_language(Language::German, || {
        assert_eq!(
            message("Screen {number}", &[("number", "{user-data}")]),
            "Bildschirm {user-data}"
        );
        assert_eq!(tr("C:\\clips\\my game.mp4"), "C:\\clips\\my game.mp4");
        let _ = std::panic::catch_unwind(|| with_language(Language::French, || panic!("test")));
        assert_eq!(tr("Settings"), "Einstellungen");
    });
    assert_eq!(tr("Settings"), "Settings");
    assert_eq!(message("unclosed {value", &[]), "unclosed {value");
}

#[test]
fn language_tags_roundtrip_and_unknown_or_legacy_preferences_remain_loadable() {
    for language in Language::ALL {
        let mut settings = crate::config::Settings {
            language,
            ..Default::default()
        };
        let json = serde_json::to_value(&settings).unwrap();
        settings = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(settings.language, language);
        assert!(!settings.recording_changed(&crate::config::Settings::default()));
    }
    let mut json = serde_json::to_value(crate::config::Settings::default()).unwrap();
    json["language"] = "future-language".into();
    assert_eq!(
        serde_json::from_value::<crate::config::Settings>(json.clone())
            .unwrap()
            .language,
        Language::EnglishUs
    );
    json.as_object_mut().unwrap().remove("language");
    assert_eq!(
        serde_json::from_value::<crate::config::Settings>(json)
            .unwrap()
            .language,
        Language::EnglishUs
    );
}

#[test]
fn global_coverage_has_unique_tags_and_keeps_failed_drafts_unselectable() {
    let tags: BTreeSet<_> = Language::ALL
        .iter()
        .map(|language| language.code())
        .collect();
    assert_eq!(tags.len(), 100);
    assert_eq!(
        Language::ALL
            .iter()
            .filter(|language| language.is_selectable())
            .count(),
        86
    );
    for language in [
        Language::Arabic,
        Language::Hebrew,
        Language::Persian,
        Language::Urdu,
    ] {
        assert!(language.is_rtl() && language.is_selectable());
    }
    for language in [Language::Lingala, Language::Yoruba, Language::Maori] {
        assert!(!language.is_selectable());
    }
    assert_ne!(
        catalog::lookup(Language::ChineseSimplified, "Preferences"),
        catalog::lookup(Language::ChineseTraditional, "Preferences")
    );
    for language in [Language::SerbianLatin, Language::Uzbek] {
        for text in catalog::entries(language).values() {
            assert!(
                !text.chars().any(|c| ('\u{0400}'..='\u{04ff}').contains(&c)),
                "{language:?}: {text}"
            );
        }
    }
}

#[test]
fn catalog_review_manifest_matches_selectable_languages() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../assets/localization-status.json")).unwrap();
    let locales = manifest["locales"].as_array().unwrap();
    assert_eq!(locales.len(), Language::ALL.len());
    for language in Language::ALL {
        let entry = locales
            .iter()
            .find(|entry| entry["code"] == language.code())
            .unwrap();
        assert_eq!(entry["selectable"], language.is_selectable());
    }
}
