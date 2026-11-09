//! Bundled OFL fonts cover scripts independently of installed Windows language packs.
use eframe::egui;
use std::sync::LazyLock;
use std::{path::PathBuf, sync::Arc};

mod native;

type Font = (&'static str, Arc<egui::FontData>);

static FALLBACKS: LazyLock<Vec<Font>> = LazyLock::new(|| {
    let mut fonts = Vec::new();
    macro_rules! font {
        ($name:literal) => {
            fonts.push((
                $name,
                Arc::new(egui::FontData::from_static(include_bytes!(concat!(
                    "../assets/fonts/",
                    $name,
                    ".ttf"
                )))),
            ));
        };
    }
    font!("notosansarabic");
    font!("notosanshebrew");
    font!("notosansdevanagari");
    font!("notosansbengali");
    font!("notosansgujarati");
    font!("notosansgurmukhi");
    font!("notosanskannada");
    font!("notosansmalayalam");
    font!("notosansoriya");
    font!("notosanstamil");
    font!("notosanstelugu");
    font!("notosanssinhala");
    font!("notosanskhmer");
    font!("notosanslao");
    font!("notosansthai");
    font!("notosansmyanmar");
    font!("notosansethiopic");
    font!("notosansarmenian");
    font!("notosansgeorgian");
    let cjk = egui::FontData::from_static(include_bytes!("../assets/fonts/notosanscjk.ttc"));
    // Noto's shared collection preserves region-specific Han character forms.
    for (name, index) in [
        ("noto-ja", 0),
        ("noto-ko", 1),
        ("noto-zh-CN", 2),
        ("noto-zh-TW", 3),
    ] {
        let mut face = cjk.clone();
        face.index = index;
        fonts.push((name, Arc::new(face)));
    }
    fonts
});

static WINDOWS_UI: LazyLock<Vec<Font>> = LazyLock::new(|| {
    let root = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
        .join("Fonts");
    [
        ("Gavnel UI", "segoeui.ttf"),
        ("Gavnel Semibold", "seguisb.ttf"),
    ]
    .into_iter()
    .filter_map(|(name, file)| {
        std::fs::read(root.join(file))
            .ok()
            .map(|bytes| (name, Arc::new(egui::FontData::from_owned(bytes))))
    })
    .collect()
});

pub(crate) fn definitions() -> egui::FontDefinitions {
    definitions_for("latin")
}

fn profile(code: &str) -> &'static str {
    match code.split('-').next().unwrap_or(code) {
        "ar" | "fa" | "ps" | "sd" | "ur" => "notosansarabic",
        "he" => "notosanshebrew",
        "hi" | "mr" | "ne" => "notosansdevanagari",
        "bn" | "as" => "notosansbengali",
        "gu" => "notosansgujarati",
        "pa" => "notosansgurmukhi",
        "kn" => "notosanskannada",
        "ml" => "notosansmalayalam",
        "or" => "notosansoriya",
        "ta" => "notosanstamil",
        "te" => "notosanstelugu",
        "si" => "notosanssinhala",
        "km" => "notosanskhmer",
        "lo" => "notosanslao",
        "th" => "notosansthai",
        "my" => "notosansmyanmar",
        "am" => "notosansethiopic",
        "hy" => "notosansarmenian",
        "ka" => "notosansgeorgian",
        "ja" => "noto-ja",
        "ko" => "noto-ko",
        "zh" if code == "zh-TW" => "noto-zh-TW",
        "zh" => "noto-zh-CN",
        _ => "latin",
    }
}

pub(crate) fn ensure_language(ctx: &egui::Context, code: &str) {
    let desired = profile(code);
    let id = egui::Id::unique("gavnel-font-profile");
    if ctx.data(|data| data.get_temp::<String>(id)).as_deref() != Some(desired) {
        ctx.set_fonts(definitions_for(desired));
        ctx.data_mut(|data| data.insert_temp(id, desired.to_owned()));
        ctx.request_repaint();
    }
}

fn definitions_for(preferred: &str) -> egui::FontDefinitions {
    let mut definitions = egui::FontDefinitions::default();
    for (name, font) in FALLBACKS.iter().chain(WINDOWS_UI.iter()) {
        definitions.font_data.insert((*name).into(), font.clone());
    }
    let mut normal = definitions.families[&egui::FontFamily::Proportional].clone();
    if definitions.font_data.contains_key("Gavnel UI") {
        normal.insert(0, "Gavnel UI".into());
    }
    normal.extend(FALLBACKS.iter().map(|(name, _)| (*name).into()));
    let mut strong = normal.clone();
    if definitions.font_data.contains_key("Gavnel Semibold") {
        strong.insert(0, "Gavnel Semibold".into());
    }
    if definitions.font_data.contains_key(preferred) {
        normal.retain(|name| name != preferred);
        strong.retain(|name| name != preferred);
        normal.insert(0, preferred.into());
        strong.insert(0, preferred.into());
    }
    definitions
        .families
        .insert(egui::FontFamily::Proportional, normal);
    definitions
        .families
        .insert(egui::FontFamily::Name("GavnelSemibold".into()), strong);
    definitions
}

#[cfg(test)]
mod tests;

/// GDI notifications need their own process-private font registration.
/// No fonts are installed or made available to other applications.
pub(crate) fn native_family(code: &str) -> &'static str {
    let preferred = profile(code);
    let family = match preferred {
        "notosansarabic" => "Noto Sans Arabic",
        "notosanshebrew" => "Noto Sans Hebrew",
        "notosansdevanagari" => "Noto Sans Devanagari",
        "notosansbengali" => "Noto Sans Bengali",
        "notosansgujarati" => "Noto Sans Gujarati",
        "notosansgurmukhi" => "Noto Sans Gurmukhi",
        "notosanskannada" => "Noto Sans Kannada",
        "notosansmalayalam" => "Noto Sans Malayalam",
        "notosansoriya" => "Noto Sans Oriya",
        "notosanstamil" => "Noto Sans Tamil",
        "notosanstelugu" => "Noto Sans Telugu",
        "notosanssinhala" => "Noto Sans Sinhala",
        "notosanskhmer" => "Noto Sans Khmer",
        "notosanslao" => "Noto Sans Lao",
        "notosansthai" => "Noto Sans Thai",
        "notosansmyanmar" => "Noto Sans Myanmar",
        "notosansethiopic" => "Noto Sans Ethiopic",
        "notosansarmenian" => "Noto Sans Armenian",
        "notosansgeorgian" => "Noto Sans Georgian",
        "noto-ja" => "Noto Sans CJK JP",
        "noto-ko" => "Noto Sans CJK KR",
        "noto-zh-CN" => "Noto Sans CJK SC",
        "noto-zh-TW" => "Noto Sans CJK TC",
        _ => return "Segoe UI",
    };
    // Every CJK face shares the same collection and registration.
    let asset = if preferred.starts_with("noto-") {
        "noto-ja"
    } else {
        preferred
    };
    let font = FALLBACKS
        .iter()
        .find(|(name, _)| *name == asset)
        .expect("bundled script font");
    if native::ensure(asset, font.1.bytes()) {
        family
    } else {
        "Segoe UI"
    }
}
