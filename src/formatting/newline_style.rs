use crate::NewlineStyle;

/// Apply this newline style to the formatted text. When the style is set
/// to `Auto`, the `raw_input_text` is used to detect the existing line
/// endings.
///
/// If the style is set to `Auto` and `raw_input_text` contains no
/// newlines, the `Native` style will be used.
pub(crate) fn apply_newline_style(
    newline_style: NewlineStyle,
    formatted_text: &mut String,
    raw_input_text: &str,
) {
    match effective_newline_style(newline_style, raw_input_text) {
        EffectiveNewlineStyle::Windows => {
            let mut transformed = String::with_capacity(2 * formatted_text.capacity());
            for c in formatted_text.chars() {
                match c {
                    '\n' => transformed.push_str("\r\n"),
                    '\r' => continue,
                    c => transformed.push(c),
                }
            }
            *formatted_text = transformed;
        }
        EffectiveNewlineStyle::Unix => {}
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum EffectiveNewlineStyle {
    Windows,
    Unix,
}

fn effective_newline_style(
    newline_style: NewlineStyle,
    raw_input_text: &str,
) -> EffectiveNewlineStyle {
    match newline_style {
        NewlineStyle::Auto => auto_detect_newline_style(raw_input_text),
        NewlineStyle::Native => native_newline_style(),
        NewlineStyle::Windows => EffectiveNewlineStyle::Windows,
        NewlineStyle::Unix => EffectiveNewlineStyle::Unix,
    }
}

fn auto_detect_newline_style(raw_input_text: &str) -> EffectiveNewlineStyle {
    if let Some(pos) = raw_input_text.find('\n') {
        let pos = pos.saturating_sub(1);
        if let Some('\r') = raw_input_text.chars().nth(pos) {
            EffectiveNewlineStyle::Windows
        } else {
            EffectiveNewlineStyle::Unix
        }
    } else {
        native_newline_style()
    }
}

fn native_newline_style() -> EffectiveNewlineStyle {
    if cfg!(windows) {
        EffectiveNewlineStyle::Windows
    } else {
        EffectiveNewlineStyle::Unix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_newline_style_auto_detect() {
        let lf = "One\nTwo\nThree";
        let crlf = "One\r\nTwo\r\nThree";
        let none = "One Two Three";

        assert_eq!(EffectiveNewlineStyle::Unix, auto_detect_newline_style(lf));
        assert_eq!(
            EffectiveNewlineStyle::Windows,
            auto_detect_newline_style(crlf)
        );

        if cfg!(windows) {
            assert_eq!(
                EffectiveNewlineStyle::Windows,
                auto_detect_newline_style(none)
            );
        } else {
            assert_eq!(EffectiveNewlineStyle::Unix, auto_detect_newline_style(none));
        }
    }

    #[test]
    fn test_newline_style_auto_apply() {
        let auto = NewlineStyle::Auto;

        let formatted_text = "One\nTwo\nThree";
        let raw_input_text = "One\nTwo\nThree";

        let mut out = String::from(formatted_text);
        apply_newline_style(auto, &mut out, raw_input_text);
        assert_eq!("One\nTwo\nThree", &out, "auto should detect 'lf'");

        let formatted_text = "One\nTwo\nThree";
        let raw_input_text = "One\r\nTwo\r\nThree";

        let mut out = String::from(formatted_text);
        apply_newline_style(auto, &mut out, raw_input_text);
        assert_eq!("One\r\nTwo\r\nThree", &out, "auto should detect 'crlf'");

        #[cfg(not(windows))]
        {
            let formatted_text = "One\nTwo\nThree";
            let raw_input_text = "One Two Three";

            let mut out = String::from(formatted_text);
            apply_newline_style(auto, &mut out, raw_input_text);
            assert_eq!(
                "One\nTwo\nThree", &out,
                "auto-native-unix should detect 'lf'"
            );
        }

        #[cfg(windows)]
        {
            let formatted_text = "One\nTwo\nThree";
            let raw_input_text = "One Two Three";

            let mut out = String::from(formatted_text);
            apply_newline_style(auto, &mut out, raw_input_text);
            assert_eq!(
                "One\r\nTwo\r\nThree", &out,
                "auto-native-windows should detect 'crlf'"
            );
        }
    }
}
