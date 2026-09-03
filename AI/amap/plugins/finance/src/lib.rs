//! Finance comparator plugin (WASM). Compares monetary values that may be numbers or formatted
//! strings ("1,234.50", "₩1,234", "1234.5 KRW") with half-up rounding to 2 decimals.
//!
//! ABI (see `amap-comparator::wasm`): `alloc`, `compare(ep, el, ap, al) -> i32`, `message_ptr/len`.
use std::cell::RefCell;

thread_local! {
    static MESSAGE: RefCell<String> = const { RefCell::new(String::new()) };
}

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    let mut buf = Vec::<u8>::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as i32
}

fn parse_money(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => {
            let cleaned: String = s.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
            cleaned.parse().ok()
        }
        _ => None,
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0 + if x >= 0.0 { 0.5 } else { -0.5 }).trunc() / 100.0
}

/// # Safety
/// Pointers must come from `alloc` and hold valid UTF-8 JSON of the given lengths.
#[no_mangle]
pub unsafe extern "C" fn compare(ep: i32, el: i32, ap: i32, al: i32) -> i32 {
    let e = std::slice::from_raw_parts(ep as *const u8, el as usize);
    let a = std::slice::from_raw_parts(ap as *const u8, al as usize);
    let (Ok(ev), Ok(av)) = (serde_json::from_slice::<serde_json::Value>(e), serde_json::from_slice::<serde_json::Value>(a)) else {
        return -1;
    };
    match (parse_money(&ev), parse_money(&av)) {
        (Some(x), Some(y)) => {
            if round2(x) == round2(y) {
                0
            } else {
                MESSAGE.with(|m| *m.borrow_mut() = format!("monetary values differ: {x} vs {y}"));
                1
            }
        }
        _ => {
            MESSAGE.with(|m| *m.borrow_mut() = "not monetary".to_string());
            -2
        }
    }
}

#[no_mangle]
pub extern "C" fn message_ptr() -> i32 {
    MESSAGE.with(|m| m.borrow().as_ptr() as i32)
}

#[no_mangle]
pub extern "C" fn message_len() -> i32 {
    MESSAGE.with(|m| m.borrow().len() as i32)
}
