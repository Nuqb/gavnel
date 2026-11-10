/// Keep stored shortcut names parseable while showing familiar key symbols.
pub fn display(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|part| match part.trim().to_ascii_lowercase().as_str() {
            "backslash" | "intlbackslash" => "\\",
            "slash" => "/",
            "backquote" => "`",
            "quote" => "'",
            "semicolon" => ";",
            "bracketleft" | "openbracket" => "[",
            "bracketright" | "closebracket" => "]",
            "minus" => "-",
            "equal" | "equals" => "=",
            "comma" => ",",
            "period" => ".",
            _ => part,
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::display;
    #[test]
    fn punctuation_uses_symbols_and_preserves_modifiers() {
        assert_eq!(display("Backslash"), "\\");
        assert_eq!(display("Control+Shift+Backslash"), "Control+Shift+\\");
        assert_eq!(display("Alt+Semicolon"), "Alt+;");
        assert_eq!(display("F8"), "F8");
        assert_eq!(display("A"), "A");
        assert_eq!(display(""), "");
    }
}
