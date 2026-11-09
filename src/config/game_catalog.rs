use std::{collections::HashMap, sync::OnceLock};

#[path = "game_catalog_data.rs"]
mod data;
pub(super) use data::OFFICIAL_GAMES;

pub(super) fn official_name(executable: &str) -> Option<&'static str> {
    static INDEX: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            OFFICIAL_GAMES
                .iter()
                .map(|&(name, executable)| (executable.to_ascii_lowercase(), name))
                .collect()
        })
        .get(&executable.to_ascii_lowercase())
        .copied()
}

// Remove old built-in false matches during migration, without deleting a
// deliberately named custom profile for the same executable.
pub(super) fn retired_builtin(name: &str, executable: &str) -> bool {
    const RETIRED: &[(&str, &str)] = &[
        ("Dragon Ball: Xenoverse 2", "start.exe"),
        ("Far Cry 5", "uplayinstaller.exe"),
        ("RuneScape: Dragonwilds", "epiconlineservicesinstaller.exe"),
        ("The Elder Scrolls III: Morrowind", "moinstaller.exe"),
        (
            "Tiny Tina's Assault on Dragon Keep: A Wonderlands One-shot Adventure",
            "dotnetfx40_client_x86_x64.exe",
        ),
        ("Wuthering Waves", "client-win64-shipping.exe"),
        ("Half-Life: Alyx - Final Hours", "alyxfinalhours.exe"),
        ("Portal 2 - The Final Hours", "thefinalhoursofportal2.exe"),
        ("Satisfactory Modeler", "satisfactory_modeler.exe"),
        ("Skyrim Special Edition: Creation Kit", "skse64_loader.exe"),
    ];
    RETIRED
        .iter()
        .chain(data::RETIRED_GAMES.iter())
        .any(|&(old_name, old_exe)| name == old_name && executable.eq_ignore_ascii_case(old_exe))
}

#[cfg(test)]
mod tests {
    use super::OFFICIAL_GAMES;
    use std::collections::HashSet;

    #[test]
    fn retired_false_matches_do_not_remove_deliberate_custom_profiles() {
        assert!(super::retired_builtin("Far Cry 5", "UPLAYINSTALLER.EXE"));
        assert!(!super::retired_builtin(
            "My custom installer",
            "uplayinstaller.exe"
        ));
        assert!(!super::retired_builtin("Far Cry 5", "farcry5.exe"));
        assert!(super::retired_builtin(
            "The Elder Scrolls III: Morrowind",
            "MGEXEgui.exe"
        ));
        assert!(!super::retired_builtin(
            "My graphics utility",
            "mgexegui.exe"
        ));
    }

    #[test]
    fn index_resolves_every_alias_and_rejects_non_games() {
        for &(name, executable) in OFFICIAL_GAMES {
            assert_eq!(
                super::official_name(&executable.to_ascii_uppercase()),
                Some(name)
            );
        }
        for executable in [
            "javaw.exe",
            "game.exe",
            "start.exe",
            "uplayinstaller.exe",
            "rpgmv.exe",
            "chrome.exe",
        ] {
            assert_eq!(super::official_name(executable), None);
        }
        for (name, exe) in [
            ("Stoneshard", "stoneshard.exe"),
            ("Schedule I", "schedule i.exe"),
            ("Clair Obscur: Expedition 33", "sandfall-win64-shipping.exe"),
            ("Battlefield 3", "bf3.exe"),
        ] {
            assert_eq!(super::official_name(exe), Some(name));
        }
    }

    #[test]
    fn catalog_has_unique_safe_windows_filenames() {
        let mut seen = HashSet::new();
        for &(name, executable) in OFFICIAL_GAMES {
            assert!(!name.trim().is_empty());
            let exe = executable.to_lowercase();
            assert!(exe.ends_with(".exe"), "{exe}");
            assert!(!exe.contains(['/', '\\', ':', '*', '?', '>', '<']), "{exe}");
            assert!(seen.insert(exe.clone()), "duplicate executable: {exe}");
            assert!(
                ![
                    "java.exe",
                    "javaw.exe",
                    "game.exe",
                    "launcher.exe",
                    "client.exe"
                ]
                .contains(&exe.as_str())
            );
        }
    }
}
