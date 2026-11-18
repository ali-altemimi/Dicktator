//! Turning raw Whisper output into draft text: spoken commands and noise cleanup.

use regex::Regex;
use std::sync::LazyLock;

/// What a transcribed clip means for the draft.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Add this text to the draft.
    Append(String),
    /// The clip was "scratch that": remove the previous chunk.
    Undo,
    /// Nothing worth keeping (silence, noise markers).
    Nothing,
}

// Whisper adds its own punctuation around spoken commands, so swallow it. The
// period *before* "new paragraph" is kept: it ends the previous sentence.
static NEW_PARAGRAPH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[\s,]*\bnew paragraph\b[\s,.!?]*").unwrap());
static NEW_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[\s,]*\bnew line\b[\s,.!?]*").unwrap());
static SCRATCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\W*(scratch that|delete that|undo that)\W*$").unwrap());
// Non-speech markers Whisper emits for silence or background sound:
// [BLANK_AUDIO], (music), [ Silence ], *coughs* ...
static NOISE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]*\]|\([^)]*\)|\*[^*]*\*").unwrap());

pub fn process(raw: &str) -> Outcome {
    let text = NOISE.replace_all(raw, "");
    let text = text.trim();
    if text.is_empty() || !text.chars().any(char::is_alphanumeric) {
        return Outcome::Nothing;
    }
    if SCRATCH.is_match(text) {
        return Outcome::Undo;
    }
    let text = NEW_PARAGRAPH.replace_all(text, "\n\n");
    let text = NEW_LINE.replace_all(&text, "\n");
    Outcome::Append(text.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn append(s: &str) -> Outcome {
        Outcome::Append(s.into())
    }

    #[test]
    fn plain_text_passes_through() {
        assert_eq!(process(" Hello world. "), append("Hello world."));
    }

    #[test]
    fn new_paragraph_keeps_the_sentence_end() {
        assert_eq!(
            process("Sleep matters. New paragraph. In section two, we discuss it."),
            append("Sleep matters.\n\nIn section two, we discuss it.")
        );
        assert_eq!(process("first, new line, second"), append("first\nsecond"));
    }

    #[test]
    fn scratch_that_only_on_its_own() {
        assert_eq!(process("Scratch that."), Outcome::Undo);
        assert_eq!(process("delete that!"), Outcome::Undo);
        assert_eq!(
            process("Scratch that idea about mice."),
            append("Scratch that idea about mice.")
        );
    }

    #[test]
    fn noise_markers_are_dropped() {
        assert_eq!(process("[BLANK_AUDIO]"), Outcome::Nothing);
        assert_eq!(process(" (music) "), Outcome::Nothing);
        assert_eq!(process("..."), Outcome::Nothing);
        assert_eq!(process("Hello [BLANK_AUDIO] there"), append("Hello  there"));
    }
}
