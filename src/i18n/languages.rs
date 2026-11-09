//! Language tags and native names. Regional variants are explicit, never country flags.
use serde::{Deserialize, Serialize};

macro_rules! languages {
    ($( $variant:ident => ($code:literal, $name:literal, $english:literal, $script:literal, $rtl:literal) ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum Language {
            $( #[serde(rename = $code)] $variant, )+
            #[default]
            #[serde(rename = "en-US", other)]
            EnglishUs,
        }
        impl Language {
            pub const ALL: [Self; 1 + [$(stringify!($variant)),+].len()] = [Self::EnglishUs, $(Self::$variant),+];
            pub const fn code(self) -> &'static str { match self { Self::EnglishUs => "en-US", $(Self::$variant => $code),+ } }
            pub const fn name(self) -> &'static str { match self { Self::EnglishUs => "English (United States)", $(Self::$variant => $name),+ } }
            pub const fn english_name(self) -> &'static str { match self { Self::EnglishUs => "English (United States)", $(Self::$variant => $english),+ } }
            pub const fn is_rtl(self) -> bool { match self { Self::EnglishUs => false, $(Self::$variant => $rtl),+ } }
            pub(super) const fn ordinal(self) -> usize { self as usize }
        }
        pub(super) const BUNDLED: &[&str] = &[
            $(include_str!(concat!("../../assets/locales/", $code, ".json")),)+
            include_str!("../../assets/locales/en-US.json"),
        ];
    };
}

languages! {
    EnglishGb => ("en-GB", "English (United Kingdom)", "English (United Kingdom)", "latin", false),
    SpanishSpain => ("es-ES", "Español (España)", "Spanish (Spain)", "latin", false),
    SpanishMexico => ("es-MX", "Español (México)", "Spanish (Mexico)", "latin", false),
    PortugueseBrazil => ("pt-BR", "Português (Brasil)", "Portuguese (Brazil)", "latin", false),
    PortuguesePortugal => ("pt-PT", "Português (Portugal)", "Portuguese (Portugal)", "latin", false),
    French => ("fr-FR", "Français", "French", "latin", false),
    German => ("de-DE", "Deutsch", "German", "latin", false),
    Italian => ("it-IT", "Italiano", "Italian", "latin", false),
    Dutch => ("nl-NL", "Nederlands", "Dutch", "latin", false),
    Polish => ("pl-PL", "Polski", "Polish", "latin", false),
    Turkish => ("tr-TR", "Türkçe", "Turkish", "latin", false),
    Ukrainian => ("uk-UA", "Українська", "Ukrainian", "cyrillic", false),
    Russian => ("ru-RU", "Русский", "Russian", "cyrillic", false),
    Afrikaans => ("af-ZA", "Afrikaans", "Afrikaans", "latin", false),
    Amharic => ("am-ET", "አማርኛ", "Amharic", "ethiopic", false),
    Arabic => ("ar", "العربية", "Arabic", "arabic", true),
    Armenian => ("hy-AM", "Հայերեն", "Armenian", "armenian", false),
    Azerbaijani => ("az-AZ", "Azərbaycanca", "Azerbaijani", "latin", false),
    Belarusian => ("be-BY", "Беларуская", "Belarusian", "cyrillic", false),
    Bulgarian => ("bg-BG", "Български", "Bulgarian", "cyrillic", false),
    Bengali => ("bn-BD", "বাংলা", "Bengali", "indic", false),
    Bosnian => ("bs-BA", "Bosanski", "Bosnian", "latin", false),
    Catalan => ("ca-ES", "Català", "Catalan", "latin", false),
    Czech => ("cs-CZ", "Čeština", "Czech", "latin", false),
    Welsh => ("cy-GB", "Cymraeg", "Welsh", "latin", false),
    Danish => ("da-DK", "Dansk", "Danish", "latin", false),
    Greek => ("el-GR", "Ελληνικά", "Greek", "greek", false),
    Estonian => ("et-EE", "Eesti", "Estonian", "latin", false),
    Persian => ("fa-IR", "فارسی", "Persian", "arabic", true),
    Finnish => ("fi-FI", "Suomi", "Finnish", "latin", false),
    Filipino => ("fil-PH", "Filipino", "Filipino", "latin", false),
    Irish => ("ga-IE", "Gaeilge", "Irish", "latin", false),
    Galician => ("gl-ES", "Galego", "Galician", "latin", false),
    Gujarati => ("gu-IN", "ગુજરાતી", "Gujarati", "indic", false),
    Hausa => ("ha-NG", "Hausa", "Hausa", "latin", false),
    Hebrew => ("he-IL", "עברית", "Hebrew", "hebrew", true),
    Hindi => ("hi-IN", "हिन्दी", "Hindi", "indic", false),
    Croatian => ("hr-HR", "Hrvatski", "Croatian", "latin", false),
    HaitianCreole => ("ht-HT", "Kreyòl ayisyen", "Haitian Creole", "latin", false),
    Hungarian => ("hu-HU", "Magyar", "Hungarian", "latin", false),
    Indonesian => ("id-ID", "Bahasa Indonesia", "Indonesian", "latin", false),
    Igbo => ("ig-NG", "Igbo", "Igbo", "latin", false),
    Icelandic => ("is-IS", "Íslenska", "Icelandic", "latin", false),
    Japanese => ("ja-JP", "日本語", "Japanese", "cjk", false),
    Georgian => ("ka-GE", "ქართული", "Georgian", "georgian", false),
    Kazakh => ("kk-KZ", "Қазақша", "Kazakh", "cyrillic", false),
    Khmer => ("km-KH", "ខ្មែរ", "Khmer", "khmer", false),
    Kannada => ("kn-IN", "ಕನ್ನಡ", "Kannada", "indic", false),
    Korean => ("ko-KR", "한국어", "Korean", "cjk", false),
    Luxembourgish => ("lb-LU", "Lëtzebuergesch", "Luxembourgish", "latin", false),
    Luganda => ("lg-UG", "Luganda", "Luganda", "latin", false),
    Lingala => ("ln-CD", "Lingála", "Lingala", "latin", false),
    Lao => ("lo-LA", "ລາວ", "Lao", "lao", false),
    Lithuanian => ("lt-LT", "Lietuvių", "Lithuanian", "latin", false),
    Latvian => ("lv-LV", "Latviešu", "Latvian", "latin", false),
    Malagasy => ("mg-MG", "Malagasy", "Malagasy", "latin", false),
    Macedonian => ("mk-MK", "Македонски", "Macedonian", "cyrillic", false),
    Malayalam => ("ml-IN", "മലയാളം", "Malayalam", "indic", false),
    Mongolian => ("mn-MN", "Монгол", "Mongolian", "cyrillic", false),
    Marathi => ("mr-IN", "मराठी", "Marathi", "indic", false),
    Malay => ("ms-MY", "Bahasa Melayu", "Malay", "latin", false),
    Burmese => ("my-MM", "မြန်မာ", "Burmese", "myanmar", false),
    Nepali => ("ne-NP", "नेपाली", "Nepali", "indic", false),
    NorwegianBokmal => ("nb-NO", "Norsk bokmål", "Norwegian Bokmål", "latin", false),
    NorthernSotho => ("nso-ZA", "Sesotho sa Leboa", "Northern Sotho", "latin", false),
    Odia => ("or-IN", "ଓଡ଼ିଆ", "Odia", "indic", false),
    PunjabiGurmukhi => ("pa-IN", "ਪੰਜਾਬੀ", "Punjabi (Gurmukhi)", "indic", false),
    Pashto => ("ps-AF", "پښتو", "Pashto", "arabic", true),
    Romanian => ("ro-RO", "Română", "Romanian", "latin", false),
    Sindhi => ("sd-PK", "سنڌي", "Sindhi", "arabic", true),
    Sinhala => ("si-LK", "සිංහල", "Sinhala", "indic", false),
    Slovak => ("sk-SK", "Slovenčina", "Slovak", "latin", false),
    Slovenian => ("sl-SI", "Slovenščina", "Slovenian", "latin", false),
    Somali => ("so-SO", "Soomaali", "Somali", "latin", false),
    Albanian => ("sq-AL", "Shqip", "Albanian", "latin", false),
    SerbianCyrillic => ("sr-Cyrl-RS", "Српски (ћирилица)", "Serbian (Cyrillic)", "cyrillic", false),
    Swedish => ("sv-SE", "Svenska", "Swedish", "latin", false),
    Swahili => ("sw-KE", "Kiswahili", "Swahili", "latin", false),
    Tamil => ("ta-IN", "தமிழ்", "Tamil", "indic", false),
    Thai => ("th-TH", "ไทย", "Thai", "thai", false),
    Tswana => ("tn-BW", "Setswana", "Tswana", "latin", false),
    Urdu => ("ur-PK", "اردو", "Urdu", "arabic", true),
    Uzbek => ("uz-UZ", "Oʻzbekcha", "Uzbek", "latin", false),
    Vietnamese => ("vi-VN", "Tiếng Việt", "Vietnamese", "latin", false),
    Wolof => ("wo-SN", "Wolof", "Wolof", "latin", false),
    Xhosa => ("xh-ZA", "isiXhosa", "Xhosa", "latin", false),
    Yoruba => ("yo-NG", "Yorùbá", "Yoruba", "latin", false),
    ChineseSimplified => ("zh-CN", "简体中文", "Chinese (Simplified)", "cjk", false),
    Zulu => ("zu-ZA", "isiZulu", "Zulu", "latin", false),
    Telugu => ("te-IN", "తెలుగు", "Telugu", "indic", false),
    Assamese => ("as-IN", "অসমীয়া", "Assamese", "indic", false),
    Maltese => ("mt-MT", "Malti", "Maltese", "latin", false),
    NorwegianNynorsk => ("nn-NO", "Norsk nynorsk", "Norwegian Nynorsk", "latin", false),
    Maori => ("mi-NZ", "Māori", "Māori", "latin", false),
    Fijian => ("fj-FJ", "Vosa Vakaviti", "Fijian", "latin", false),
    Samoan => ("sm-WS", "Gagana Samoa", "Samoan", "latin", false),
    Tongan => ("to-TO", "Lea faka-Tonga", "Tongan", "latin", false),
    ChineseTraditional => ("zh-TW", "繁體中文（台灣）", "Chinese (Traditional, Taiwan)", "cjk", false),
    SerbianLatin => ("sr-Latn-RS", "Srpski (latinica)", "Serbian (Latin)", "latin", false),
}

impl Language {
    pub fn is_selectable(self) -> bool {
        !matches!(
            self,
            Self::Hausa
                | Self::Igbo
                | Self::Luganda
                | Self::Lingala
                | Self::NorthernSotho
                | Self::Tswana
                | Self::Wolof
                | Self::Xhosa
                | Self::Yoruba
                | Self::Zulu
                | Self::Maori
                | Self::Fijian
                | Self::Samoan
                | Self::Tongan
        )
    }
}
