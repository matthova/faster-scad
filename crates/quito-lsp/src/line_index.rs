//! Byte-offset ↔ LSP [`Position`] mapping.
//!
//! The engine reports spans as byte offsets into the source, while LSP positions
//! are `(line, character)` where `character` counts **UTF-16 code units** (the
//! default `PositionEncodingKind`). This module bridges the two.

use tower_lsp::lsp_types::{Position, Range};

/// A precomputed index of line-start byte offsets for one document.
pub struct LineIndex {
    text: String,
    /// Byte offset of the start of each line (line 0 starts at 0).
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        LineIndex {
            text: text.to_string(),
            line_starts,
        }
    }

    /// Convert a byte offset into a [`Position`]. Offsets past the end clamp to
    /// the document end; offsets that land inside a multi-byte char snap down to
    /// its start.
    pub fn position(&self, byte: usize) -> Position {
        let byte = byte.min(self.text.len());
        // Find the last line whose start is <= byte.
        let line = match self.line_starts.binary_search(&byte) {
            Ok(l) => l,
            Err(next) => next - 1,
        };
        let line_start = self.line_starts[line];
        // Count UTF-16 units from the line start to `byte`, snapping to a char
        // boundary so we never slice mid-codepoint.
        let mut boundary = byte;
        while boundary > line_start && !self.text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        let col16: u32 = self.text[line_start..boundary]
            .chars()
            .map(|c| c.len_utf16() as u32)
            .sum();
        Position {
            line: line as u32,
            character: col16,
        }
    }

    /// Convert a byte span into an LSP [`Range`].
    pub fn range(&self, span: std::ops::Range<usize>) -> Range {
        Range {
            start: self.position(span.start),
            end: self.position(span.end),
        }
    }

    /// Convert a [`Position`] back to a byte offset (inverse of [`position`]).
    /// Out-of-range lines/characters clamp to the document.
    pub fn offset(&self, pos: Position) -> usize {
        let line = pos.line as usize;
        if line >= self.line_starts.len() {
            return self.text.len();
        }
        let line_start = self.line_starts[line];
        let line_end = self
            .line_starts
            .get(line + 1)
            .map(|&s| s.saturating_sub(1)) // drop the '\n'
            .unwrap_or(self.text.len());
        // Walk UTF-16 units across the line to find the byte offset.
        let mut remaining = pos.character;
        let mut byte = line_start;
        for c in self.text[line_start..line_end].chars() {
            if remaining == 0 {
                break;
            }
            let w = c.len_utf16() as u32;
            if remaining < w {
                break;
            }
            remaining -= w;
            byte += c.len_utf8();
        }
        byte
    }

    /// The identifier (`[A-Za-z0-9_$]+`, not starting with a digit) surrounding a
    /// byte offset, if any. Used to resolve the symbol under the cursor for hover.
    pub fn word_at(&self, byte: usize) -> Option<(String, std::ops::Range<usize>)> {
        let bytes = self.text.as_bytes();
        let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
        if byte > bytes.len() {
            return None;
        }
        // Expand left, then right, from the cursor.
        let mut start = byte;
        while start > 0 && is_word(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = byte;
        while end < bytes.len() && is_word(bytes[end]) {
            end += 1;
        }
        if start == end {
            return None;
        }
        let word = self.text[start..end].to_string();
        // Reject a bare number.
        if word.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return None;
        }
        Some((word, start..end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_maps_lines_and_columns() {
        let idx = LineIndex::new("cube(1);\ntranslate([0,0,1]);");
        // Start of line 0.
        assert_eq!(idx.position(0), Position::new(0, 0));
        // Start of line 1 (after the '\n' at byte 8).
        assert_eq!(idx.position(9), Position::new(1, 0));
        // "translate" ends at column 9 on line 1.
        assert_eq!(idx.position(9 + 9), Position::new(1, 9));
    }

    #[test]
    fn utf16_columns_count_code_units() {
        // '𝄞' (U+1D11E) is one char but two UTF-16 units and 4 UTF-8 bytes.
        let idx = LineIndex::new("x=\"𝄞\";");
        // After the emoji-like char: byte offset 3 (x=") + 4 = 7 → column 3+2=5.
        let pos = idx.position(3 + 4);
        assert_eq!(pos, Position::new(0, 5));
    }

    #[test]
    fn word_at_finds_identifier() {
        let idx = LineIndex::new("translate([0,0,1]) cube(2);");
        let (w, _) = idx.word_at(3).unwrap();
        assert_eq!(w, "translate");
        let (w, _) = idx.word_at(20).unwrap();
        assert_eq!(w, "cube");
        // Inside the number literal → not a word.
        assert!(idx.word_at(11).is_none());
    }

    #[test]
    fn offset_round_trips_with_position() {
        let src = "cube(1);\ntranslate([0,0,1]);";
        let idx = LineIndex::new(src);
        for byte in [0usize, 5, 9, 18] {
            let pos = idx.position(byte);
            assert_eq!(idx.offset(pos), byte, "byte {byte}");
        }
    }
}
