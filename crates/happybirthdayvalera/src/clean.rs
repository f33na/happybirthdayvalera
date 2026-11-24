//! Cleaning of engine output: Whisper's sound tags and its stock phrases for silence never reach the user.

use std::sync::LazyLock;

use crate::easter::normalize;
use crate::shared::BEHAVIOR;

/// `behavior.json` → `hallucinations`, normalized.
static HALLUCINATIONS: LazyLock<Vec<String>> = LazyLock::new(|| {
    BEHAVIOR.hallucinations.iter().map(|phrase| normalize(phrase)).filter(|phrase| !phrase.is_empty()).collect()
});

/// The text of an engine result without non-speech output; empty when nothing is speech.
///
/// Removes every `[...]`, `(...)` and `*...*` span and every `♪` and `♫`, then collapses whitespace. A rest
/// without a letter or a digit is empty, and so is a rest that, normalized, is exactly a known hallucination
/// phrase. A hallucination phrase inside longer speech stays.
pub(crate) fn clean_transcript(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let close = match c {
            '[' => Some(']'),
            '(' => Some(')'),
            '*' => Some('*'),
            _ => None,
        };
        if let Some(close) = close
            && let Some(len) = chars[i + 1..].iter().position(|&d| d == close)
        {
            // The span becomes a space, so the words around it stay apart.
            out.push(' ');
            i += len + 2;
            continue;
        }
        if c != '♪' && c != '♫' {
            out.push(c);
        } else {
            out.push(' ');
        }
        i += 1;
    }
    let text = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if !text.chars().any(char::is_alphanumeric) {
        return String::new();
    }
    let normalized = normalize(&text);
    if HALLUCINATIONS.contains(&normalized) {
        return String::new();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::clean_transcript;

    #[test]
    fn non_speech_output_is_removed() {
        let cases = [
            ("[BLANK_AUDIO]", ""),
            (" [Music] ", ""),
            ("(смеется)", ""),
            ("*applause*", ""),
            ("♪ ♫ ♪", ""),
            ("♪ la la ♪", "la la"),
            ("[BLANK_AUDIO] привет", "привет"),
            ("Hello [Music] world.", "Hello world."),
            ("Hello (laughs), world", "Hello , world"),
            ("...", ""),
            ("- - -", ""),
            ("Субтитры сделал DimaTorzok", ""),
            ("Thank you for watching!", ""),
            ("Thank you.", ""),
            ("Продолжение следует...", ""),
            ("Sous-titres réalisés par la communauté d'Amara.org", ""),
            ("Thank you for watching my sister play", "Thank you for watching my sister play"),
            ("I said thank you to him.", "I said thank you to him."),
            ("Привет, мир", "Привет, мир"),
            ("A 5 [x", "A 5 [x"),
            ("2 * 3 = 6", "2 * 3 = 6"),
        ];
        for (input, expected) in cases {
            assert_eq!(clean_transcript(input), expected, "{input:?}");
        }
    }
}
