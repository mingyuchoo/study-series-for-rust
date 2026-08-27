//! Small output helpers. Plain text, no colour dependency.

pub const OK: &str = "✓";
pub const ERR: &str = "✗";
pub const ARROW: &str = "→";

pub fn heading(text: &str) {
    println!("{text}");
}

pub fn field(label: &str, value: impl std::fmt::Display) {
    println!("  {label:<12} {value}");
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}
