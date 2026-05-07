//! Terminal markdown rendering utilities.
//!
//! Converts markdown text to styled terminal output.

/// Simple markdown-to-terminal renderer.
/// Handles: bold, italic, code blocks, inline code, headers, lists.
pub fn render_markdown(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut in_code_block = false;

    for line in text.lines() {
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            if in_code_block {
                output.push_str("┌─── code ───\n");
            } else {
                output.push_str("└────────────\n");
            }
            continue;
        }

        if in_code_block {
            output.push_str("│ ");
            output.push_str(line);
            output.push('\n');
            continue;
        }

        // Headers
        if let Some(rest) = line.strip_prefix("### ") {
            output.push_str(&format!("  {} {}\n", "###", rest));
        } else if let Some(rest) = line.strip_prefix("## ") {
            output.push_str(&format!(" {} {}\n", "##", rest));
        } else if let Some(rest) = line.strip_prefix("# ") {
            output.push_str(&format!("{} {}\n", "#", rest));
        } else if let Some(rest) = line.strip_prefix("- ") {
            output.push_str(&format!("  * {}\n", rest));
        } else if let Some(rest) = line.strip_prefix("* ") {
            output.push_str(&format!("  * {}\n", rest));
        } else {
            // Inline formatting
            let formatted = render_inline(line);
            output.push_str(&formatted);
            output.push('\n');
        }
    }

    output
}

/// Render inline markdown: **bold**, *italic*, `code`.
fn render_inline(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '`' {
            // Inline code
            if let Some(end) = find_closing(&chars, i + 1, '`') {
                result.push('[');
                result.extend(&chars[i + 1..end]);
                result.push(']');
                i = end + 1;
                continue;
            }
        }

        if i + 2 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            // Bold
            if let Some(end) = find_double_closing(&chars, i + 2, '*') {
                result.push_str("**");
                result.extend(&chars[i + 2..end]);
                result.push_str("**");
                i = end + 2;
                continue;
            }
        }

        if chars[i] == '*' && (i + 1 < chars.len() && chars[i + 1] != '*') {
            // Italic
            if let Some(end) = find_closing(&chars, i + 1, '*') {
                result.push('_');
                result.extend(&chars[i + 1..end]);
                result.push('_');
                i = end + 1;
                continue;
            }
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Find closing delimiter in char slice.
fn find_closing(chars: &[char], start: usize, delimiter: char) -> Option<usize> {
    chars
        .iter()
        .position(|&c| c == delimiter)
        .and_then(|pos| {
            let abs = pos;
            if abs >= start {
                Some(abs)
            } else {
                None
            }
        })
        .or_else(|| (start..chars.len()).find(|&i| chars[i] == delimiter))
}

/// Find closing double delimiter (e.g., **).
fn find_double_closing(chars: &[char], start: usize, delimiter: char) -> Option<usize> {
    (start..chars.len().saturating_sub(1))
        .find(|&i| chars[i] == delimiter && chars[i + 1] == delimiter)
}

/// Display width of text accounting for wide chars (CJK, emoji).
pub fn display_width(text: &str) -> usize {
    use unicode_width::UnicodeWidthStr;
    UnicodeWidthStr::width(text)
}

/// Word-wrap text to fit within given width.
pub fn word_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut current_line = String::new();

        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if display_width(&current_line) + 1 + display_width(word) <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_markdown_plain() {
        let result = render_markdown("Hello world");
        assert_eq!(result.trim(), "Hello world");
    }

    #[test]
    fn test_render_markdown_headers() {
        let result = render_markdown("# Title\n## Subtitle\n### Section");
        assert!(result.contains("Title"));
        assert!(result.contains("Subtitle"));
        assert!(result.contains("Section"));
    }

    #[test]
    fn test_render_markdown_code_block() {
        let result = render_markdown("```rust\nfn main() {}\n```");
        assert!(result.contains("code"));
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn test_render_markdown_list() {
        let result = render_markdown("- item 1\n- item 2\n* item 3");
        assert!(result.contains("* item 1"));
        assert!(result.contains("* item 2"));
        assert!(result.contains("* item 3"));
    }

    #[test]
    fn test_render_inline_code() {
        let result = render_inline("Use `cargo build` to compile");
        assert!(result.contains("[cargo build]"));
    }

    #[test]
    fn test_render_inline_bold() {
        let result = render_inline("This is **bold** text");
        assert!(result.contains("**bold**"));
    }

    #[test]
    fn test_render_inline_italic() {
        let result = render_inline("This is *italic* text");
        assert!(result.contains("_italic_"));
    }

    #[test]
    fn test_word_wrap_short() {
        let lines = word_wrap("Hello", 80);
        assert_eq!(lines, vec!["Hello"]);
    }

    #[test]
    fn test_word_wrap_long() {
        let lines = word_wrap("The quick brown fox jumps over the lazy dog", 20);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(display_width(line) <= 20);
        }
    }

    #[test]
    fn test_word_wrap_preserves_newlines() {
        let lines = word_wrap("Line 1\nLine 2\nLine 3", 80);
        assert_eq!(lines, vec!["Line 1", "Line 2", "Line 3"]);
    }

    #[test]
    fn test_word_wrap_empty() {
        let lines = word_wrap("", 80);
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn test_word_wrap_zero_width() {
        let lines = word_wrap("Hello world", 0);
        assert_eq!(lines, vec!["Hello world"]);
    }

    #[test]
    fn test_display_width() {
        assert_eq!(display_width("Hello"), 5);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn test_render_markdown_empty() {
        let result = render_markdown("");
        assert!(result.is_empty() || result.trim().is_empty());
    }

    #[test]
    fn test_render_inline_no_formatting() {
        let result = render_inline("Plain text here");
        assert_eq!(result, "Plain text here");
    }

    #[test]
    fn test_render_inline_unclosed_backtick() {
        let result = render_inline("Open `code without close");
        assert_eq!(result, "Open `code without close");
    }
}
