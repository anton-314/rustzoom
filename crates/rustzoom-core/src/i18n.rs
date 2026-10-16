//! UI texts in English and German.
//!
//! The `texts!` table defines every text in both languages at once, so a missing translation
//! cannot compile.

use crate::config::Language;

macro_rules! texts {
    ($($name:ident => $en:literal, $de:literal;)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Text {
            $($name),*
        }

        impl Text {
            pub const ALL: &[Text] = &[$(Text::$name),*];
        }

        pub fn text(language: Language, text: Text) -> &'static str {
            match (language, text) {
                $(
                    (Language::English, Text::$name) => $en,
                    (Language::German, Text::$name) => $de,
                )*
            }
        }
    };
}

texts! {
    ZoomEnabled => "Zoom enabled", "Zoom aktiv";
    ZoomKey => "Zoom key", "Zoom-Taste";
    Change => "Change…", "Ändern…";
    PressKey => "Press a key or mouse button… (Esc cancels)",
        "Taste oder Maustaste drücken… (Esc bricht ab)";
    Mode => "Mode", "Modus";
    ModeHold => "Hold", "Halten";
    ModeToggle => "Toggle", "Umschalten";
    Levels => "Zoom levels", "Zoomstufen";
    AddLevel => "Add level", "Stufe hinzufügen";
    RemoveLevel => "Remove", "Entfernen";
    CycleKey => "Next level key", "Taste für nächste Stufe";
    NoKey => "None", "Keine";
    Clear => "Clear", "Entfernen";
    ScopeSize => "Scope size", "Kreisgröße";
    BorderWidth => "Border width", "Randstärke";
    Language => "Language", "Sprache";
    LanguageSystem => "Windows default", "Windows-Standard";
    BorderlessHint => "Rust must run in \"Borderless\" display mode.",
        "Rust muss im Anzeigemodus „Randlos“ laufen.";
    MinimizeHint => "Minimize to keep zooming, close to quit.",
        "Minimieren: Zoom läuft weiter. Schließen: Programm beenden.";
}

/// Each language's name in itself, for the language picker.
pub fn language_name(language: Language) -> &'static str {
    match language {
        Language::English => "English",
        Language::German => "Deutsch",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_text_is_present() {
        for &t in Text::ALL {
            for language in [Language::English, Language::German] {
                assert!(!text(language, t).trim().is_empty(), "{t:?} {language:?}");
            }
        }
    }

    #[test]
    fn german_is_actually_translated() {
        for &t in Text::ALL {
            assert_ne!(
                text(Language::English, t),
                text(Language::German, t),
                "{t:?} is identical in both languages"
            );
        }
    }

    #[test]
    fn language_names() {
        assert_eq!(language_name(Language::German), "Deutsch");
        assert_eq!(language_name(Language::English), "English");
    }
}
