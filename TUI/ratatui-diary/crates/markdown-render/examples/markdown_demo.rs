use markdown_render::render_to_text;

fn main() {
    let markdown = r#"# Markdown Demo

This is a **bold** text and this is *italic*.

## Code Example

```rust
fn main() {
    println!("Hello from Rust!");
}
```

## List

* Item 1
* Item 2
* Item 3

### Inline Code

You can use `inline_code` like this.
"#;

    let text = render_to_text(markdown);

    println!("Rendered Markdown:");
    println!("==================");
    for line in &text.lines {
        println!("{}", line);
    }
    println!("\nTotal lines: {}", text.lines.len());
}
