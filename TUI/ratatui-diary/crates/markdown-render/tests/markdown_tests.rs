use markdown_render::{render_to_text,
                      to_ratatui_color};
use ratatui::style::Color;
use termimad::crossterm::style::Color as TermColor;

#[test]
fn test_render_empty_string() {
    let result = render_to_text("");
    assert!(!result.lines.is_empty());
    assert_eq!(result.lines.len(), 1);
}

#[test]
fn test_render_plain_text() {
    let result = render_to_text("Hello, World!");
    assert!(result.lines.len() > 0);
    assert_eq!(result.lines[0].spans.len(), 1);
}

#[test]
fn test_render_headers() {
    let markdown = "# Header 1\n## Header 2\n### Header 3";
    let result = render_to_text(markdown);
    assert_eq!(result.lines.len(), 3);
}

#[test]
fn test_render_bold_italic() {
    let markdown = "**bold** and *italic*";
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 0);
}

#[test]
fn test_render_code_block() {
    let markdown = "```rust\nfn main() {}\n```";
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 0);
}

#[test]
fn test_render_list() {
    let markdown = "* Item 1\n* Item 2\n* Item 3";
    let result = render_to_text(markdown);
    assert!(result.lines.len() >= 3);
}

#[test]
fn test_render_complex_markdown() {
    let markdown = r#"# Diary Entry

Today I learned about **Rust** and *Ratatui*.

## Code Example

```rust
fn main() {
    println!("Hello, World!");
}
```

## List of Tasks

* Write code
* Test code
* Commit changes
"#;
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 5);
}

#[test]
fn test_render_blockquote() {
    let markdown = "> This is a quote\n> Multiple lines";
    let result = render_to_text(markdown);
    assert!(result.lines.len() >= 2);
}

#[test]
fn test_render_horizontal_rule() {
    let markdown = "Before\n\n---\n\nAfter";
    let result = render_to_text(markdown);
    assert!(result.lines.len() >= 3);
}

#[test]
fn test_render_table() {
    let markdown = "| Col1 | Col2 |\n|------|------|\n| A    | B    |";
    let result = render_to_text(markdown);
    assert!(result.lines.len() >= 3);
}

#[test]
fn test_render_links() {
    let markdown = "[Link Text](https://example.com)";
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 0);
}

// 추가 테스트: Skin colors 및 인라인 코드 테스트
#[test]
fn test_render_inline_code() {
    let markdown = "This is `inline code` in text";
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 0);
}

#[test]
fn test_render_strikethrough() {
    let markdown = "~~strikethrough text~~";
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 0);
}

#[test]
fn test_render_mixed_formatting() {
    let markdown = "**bold `code`** and *italic* and `more code`";
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 0);
}

#[test]
fn test_render_multiline_text() {
    let markdown = "Line 1\nLine 2\nLine 3";
    let result = render_to_text(markdown);
    assert_eq!(result.lines.len(), 3);
}

#[test]
fn test_render_long_line() {
    let long_text = "a".repeat(500);
    let result = render_to_text(&long_text);
    assert!(result.lines.len() > 0);
}

#[test]
fn test_render_unicode_text() {
    let markdown = "안녕하세요 Rust 世界";
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 0);
}

#[test]
fn test_render_special_characters() {
    let markdown = "Special chars: !@#$%^&*()_+-=[]{}|;':\",./<>?";
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 0);
}

#[test]
fn test_render_mixed_lists() {
    let markdown = "* Item 1\n* **bold item**\n* `code item`";
    let result = render_to_text(markdown);
    assert!(result.lines.len() >= 3);
}

#[test]
fn test_render_nested_formatting() {
    let markdown = "***bold italic***";
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 0);
}

#[test]
fn test_render_paragraph_breaks() {
    let markdown = "Paragraph 1\n\n\nParagraph 2\n\n\n\nParagraph 3";
    let result = render_to_text(markdown);
    assert!(result.lines.len() >= 3);
}

#[test]
fn test_render_indented_text() {
    let markdown = "    Indented line 1\n    Indented line 2";
    let result = render_to_text(markdown);
    assert!(result.lines.len() >= 2);
}

#[test]
fn test_render_mixed_header_and_text() {
    let markdown = "# Header\n\nSome text\n\n## Subheader\n\nMore text";
    let result = render_to_text(markdown);
    assert!(result.lines.len() >= 4);
}

#[test]
fn test_render_code_block_multiple_languages() {
    let markdown = r#"```python
print("Hello")
```

```javascript
console.log("Hello");
```"#;
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 2);
}

#[test]
fn test_render_empty_code_block() {
    let markdown = "```\ncode\n```";
    let result = render_to_text(markdown);
    assert!(result.lines.len() > 0);
}

#[test]
fn test_render_numbered_list() {
    let markdown = "1. First\n2. Second\n3. Third";
    let result = render_to_text(markdown);
    assert!(result.lines.len() >= 3);
}

// Color conversion tests
#[test]
fn test_to_ratatui_color_black() {
    let color = to_ratatui_color(TermColor::Black);
    assert_eq!(color, Color::Black);
}

#[test]
fn test_to_ratatui_color_red() {
    let color = to_ratatui_color(TermColor::Red);
    assert_eq!(color, Color::Red);
}

#[test]
fn test_to_ratatui_color_dark_red() {
    let color = to_ratatui_color(TermColor::DarkRed);
    assert_eq!(color, Color::Red);
}

#[test]
fn test_to_ratatui_color_green() {
    let color = to_ratatui_color(TermColor::Green);
    assert_eq!(color, Color::Green);
}

#[test]
fn test_to_ratatui_color_dark_green() {
    let color = to_ratatui_color(TermColor::DarkGreen);
    assert_eq!(color, Color::Green);
}

#[test]
fn test_to_ratatui_color_yellow() {
    let color = to_ratatui_color(TermColor::Yellow);
    assert_eq!(color, Color::Yellow);
}

#[test]
fn test_to_ratatui_color_dark_yellow() {
    let color = to_ratatui_color(TermColor::DarkYellow);
    assert_eq!(color, Color::Yellow);
}

#[test]
fn test_to_ratatui_color_blue() {
    let color = to_ratatui_color(TermColor::Blue);
    assert_eq!(color, Color::Blue);
}

#[test]
fn test_to_ratatui_color_dark_blue() {
    let color = to_ratatui_color(TermColor::DarkBlue);
    assert_eq!(color, Color::Blue);
}

#[test]
fn test_to_ratatui_color_magenta() {
    let color = to_ratatui_color(TermColor::Magenta);
    assert_eq!(color, Color::Magenta);
}

#[test]
fn test_to_ratatui_color_dark_magenta() {
    let color = to_ratatui_color(TermColor::DarkMagenta);
    assert_eq!(color, Color::Magenta);
}

#[test]
fn test_to_ratatui_color_cyan() {
    let color = to_ratatui_color(TermColor::Cyan);
    assert_eq!(color, Color::Cyan);
}

#[test]
fn test_to_ratatui_color_dark_cyan() {
    let color = to_ratatui_color(TermColor::DarkCyan);
    assert_eq!(color, Color::Cyan);
}

#[test]
fn test_to_ratatui_color_white() {
    let color = to_ratatui_color(TermColor::White);
    assert_eq!(color, Color::White);
}

#[test]
fn test_to_ratatui_color_grey() {
    let color = to_ratatui_color(TermColor::Grey);
    assert_eq!(color, Color::Gray);
}

#[test]
fn test_to_ratatui_color_dark_grey() {
    let color = to_ratatui_color(TermColor::DarkGrey);
    assert_eq!(color, Color::DarkGray);
}

#[test]
fn test_to_ratatui_color_rgb() {
    let color = to_ratatui_color(TermColor::Rgb {
        r: 100,
        g: 150,
        b: 200,
    });
    assert_eq!(color, Color::Rgb(100, 150, 200));
}

#[test]
fn test_to_ratatui_color_ansi_value() {
    let color = to_ratatui_color(TermColor::AnsiValue(42));
    assert_eq!(color, Color::Indexed(42));
}

#[test]
fn test_to_ratatui_color_reset() {
    let color = to_ratatui_color(TermColor::Reset);
    assert_eq!(color, Color::Reset);
}
