mod item;
mod measurement;
mod revision;

// 단계·상태·집계 방식은 contracts가 단일 정의를 갖는 공유 커널이다.
// 와이어 표현과 도메인 규칙(계층·집계)이 항상 같아야 하는 값 타입이라
// 중복 정의 대신 재사용한다.
pub use contracts::{ItemKind,
                    ItemStatus,
                    KpiAggregation};
pub use item::*;
pub use measurement::*;
pub use revision::*;
