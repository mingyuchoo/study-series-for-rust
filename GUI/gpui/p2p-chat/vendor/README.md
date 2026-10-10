# GPUI 의존성 호환성 패치

GPUI 0.2.2의 간접 의존성을 crates.io 배포 소스 그대로 보관하고,
루트 `Cargo.toml`의 `[patch.crates-io]`로 아래 수정만 적용합니다.

- [block 0.1.6](https://crates.io/crates/block/0.1.6) (MIT): 외부 정적 변수의
  빈 enum 타입을 `c_void`로 바꾸고, 생략된 C ABI를 명시합니다.
- [proc-macro-error2 2.0.1](https://crates.io/crates/proc-macro-error2/2.0.1)
  (MIT OR Apache-2.0): 재공개하는 `proc_macro`의 `extern crate` 선언을
  `pub`으로 바꿉니다. 원본 라이선스 파일을 함께 보관합니다.

`block`의 외부 정적 변수는 값을 읽지 않고 원시 주소만 사용합니다.
GPUI 의존성에서 이 문제가 해결되면 해당 패치와 보관 소스를 제거합니다.
검증 명령은 `./scripts/run.sh --skip-run`입니다.
