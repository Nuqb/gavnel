# Languages and translation maintenance

Gavnel bundles its translations locally; language selection never contacts a
translation service. Choose a language on the welcome screen or in **Settings →
General → Language**. Changes appear without restarting. Finishing setup saves
the draft; skipping saves only the selected language and the completion marker.
Setup preview never saves either.

## Available locales

The picker offers **86 language and regional choices**, with **100 catalogs** in
source control. Fourteen drafts are withheld because meaning checks found wrong
language, unrelated terminology, or substantial untranslated text. This is broad
coverage, not a claim to cover every official language or dialect.

Native names, English names, and BCP 47 tags are searchable. Regional choices
include Spanish (Spain/Mexico), Portuguese (Brazil/Portugal), US/UK English,
Simplified Chinese/Traditional Chinese (Taiwan), and Serbian Cyrillic/Latin.
The exact list, availability, generation provenance, and review notes live in
`assets/localization-status.json`. Country flags are not used for languages.

Bundled SIL Open Font License Noto fonts cover Arabic, Hebrew, Indic, Southeast
Asian, Ethiopic, Armenian, Georgian, and East Asian writing systems. Regional
CJK faces preserve local character forms. Segoe UI remains the Latin UI font
when installed. Fonts are selected by script; catalogs load on demand. The native save
notification also registers the selected bundled font privately for this process;
this never installs fonts system-wide.

The approved egui upgrade enables complex-script shaping. A pinned upstream
right-to-left patch handles mixed-direction text; onboarding navigation and the
main navigation rail mirror for RTL locales. Shortcuts, paths, and user input
remain literal. Keep testing text input, selection, and menus as well as labels.

## Catalogs and context

`assets/locales/en-US.json` contains the English source messages, which serve as
lookup keys. Other JSON files contain whole-message translations. British English
inherits US English, Mexican Spanish inherits Spain Spanish, and Portugal
Portuguese inherits Brazilian Portuguese; regional files override complete
messages. Missing unknown keys display their original English text. Unknown saved
language tags fall back to English without invalidating other settings.

Treat regional catalogs as independent editorial choices. For example, Mexican
Spanish uses *computadora* where Spain Spanish uses *ordenador*. Portugal
Portuguese uses *ecrã* and *ficheiro*, compared with Brazilian *tela* and *arquivo*.
Never use runtime word replacement or assume that two regions differ only in
spelling. Pay attention to articles, gender, verb agreement, tone, and references
to other translated controls. The [Unicode CLDR guidance on regional
variants](https://cldr.unicode.org/translation/translation-guide-general/default-content)
is a useful reference.

Here, **clip** means a saved recent gameplay excerpt, **long recording** is an
explicitly started recording, **bookmark** marks a timestamp within a recording,
and **capture source** is the game, window, or screen being captured. **Replay
length** means how far back a clip saves, not playback speed. **Tray** means the
Windows notification area. **Noise suppression** filters microphone noise; it is
not a volume control. Keep those distinctions in every locale.

Use concise, friendly UI phrasing. Do not translate Gavnel, game names, codec
names, device names, executable filenames, paths, user input, or shortcut tokens
such as `Control+F8`. FPS, Mbps, resolution labels, and duration timestamps remain
technical notation. Dates in clip cards use unambiguous `YYYY-MM-DD` notation.
Detailed Windows, driver, and encoder diagnostics retain their original wording.

## Adding or changing messages

1. Add the complete message to the English catalog and every base catalog.
   Keep draft catalogs complete too, but do not expose them to users until corrected.
2. Review regional variants and add complete-message overrides where needed.
3. Use `i18n::tr` for fixed app-owned text and `i18n::message` with named values
   for dynamic text. Translators may reorder `{name}` placeholders, but must not
   rename, remove, duplicate, or translate them. Values are inserted once.
4. Keep persistent IDs, settings values, and hotkey parsing independent of text.
   A locale change must never restart recording or rewrite source/profile names.
5. Run `python scripts/check-locales.py`, `cargo test --locked`, and strict Clippy.
   The tests check completeness, placeholders, saved settings, regional wording,
   language switching, glyphs, and all onboarding steps at minimum window size.
6. Use **Preview setup** for an isolated check. The ignored
   `export_localized_onboarding_previews` test exports representative localized
   screens without recording, device access, or preference writes.

## Review status

All selectable catalogs are **initial translations, pending independent
native-speaker review**. The expansion uses offline MADLAD-400 drafts with
contextual corrections and explicit action/state labels. Automated checks cover
keys, placeholders, encoding, fonts, persistence, navigation, and layout; they do
not prove cultural or linguistic correctness. No translator, model, or network
service is included at runtime.

Hausa, Igbo, Luganda, Lingala, Northern Sotho, Tswana, Wolof, Xhosa, Yoruba, Zulu,
Māori, Fijian, Samoan, and Tongan remain draft-only. Correct their whole messages
with qualified speakers before changing `Language::is_selectable`. Related
languages are not interchangeable: a French, Swedish, or English result must not
be accepted as a translation just because a model produced it.

Review Traditional Chinese conversion, Serbian/Uzbek transliteration, technical
recording vocabulary, gender, tone, and all protected-slot repairs in particular.
Record reviewer, locale, date, and scope in the status manifest as reviews occur.
Do not label a catalog verified merely because it passes tests.
