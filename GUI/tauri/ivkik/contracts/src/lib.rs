//! 백엔드(Tauri 커맨드)와 프런트엔드(wasm)가 공유하는 와이어 계약.
//!
//! 이 크레이트가 단일 정의가 되어 양쪽 DTO가 어긋나는 것을 컴파일
//! 타임에 막는다. 어떤 타겟에서도 빌드되도록 의존성은 serde뿐이다.

mod dto;
mod error;
mod item;

pub use dto::*;
pub use error::*;
pub use item::*;
