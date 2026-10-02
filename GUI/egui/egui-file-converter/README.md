# File Converter App

Rust 기반 플러그인 아키텍처를 사용한 파일 확장자 변환 데스크탑 애플리케이션입니다. egui를 사용한 크로스 플랫폼 GUI와 sqlite 기반 데이터 관리를 제공합니다.

## 주요 기능

- 🔌 **플러그인 기반 아키텍처**: 동적 플러그인 로딩으로 새로운 파일 형식 지원 추가
- 🖥️ **크로스 플랫폼 GUI**: egui를 사용한 직관적인 사용자 인터페이스
- 📊 **변환 이력 관리**: sqlite를 통한 변환 작업 이력 저장 및 조회
- ⚙️ **사용자 설정**: 테마, 기본 출력 디렉토리 등 커스터마이징 가능
- 📦 **일괄 변환**: 여러 파일을 한 번에 변환
- 🔄 **비동기 처리**: UI 블로킹 없이 백그라운드에서 변환 실행

## 프로젝트 구조

```
file-converter-app/
├── Cargo.toml                 # Workspace 루트
├── plugin-manager/            # 핵심 비즈니스 로직
│   ├── src/
│   │   ├── engine.rs          # 변환 엔진
│   │   ├── registry.rs        # 플러그인 레지스트리
│   │   ├── loader.rs          # 동적 플러그인 로더
│   │   └── error.rs           # 에러 타입 정의
│   └── tests/
│       └── integration_test.rs # 통합 테스트
├── plugin-interface/          # 플러그인 트레이트 정의
│   └── src/
│       └── lib.rs             # Plugin 트레이트 및 관련 타입
├── database-manager/          # sqlite 데이터 관리
│   └── src/
│       ├── schema.rs          # 데이터베이스 스키마
│       ├── history.rs         # 변환 이력 관리
│       └── settings.rs        # 사용자 설정 관리
├── app-gui/                   # egui 기반 UI
│   └── src/
│       ├── main.rs            # 애플리케이션 진입점
│       └── app.rs             # GUI 애플리케이션 로직
└── plugins/                   # 플러그인 구현체들
    └── text-converter/        # 텍스트 인코딩 변환 플러그인
        ├── src/
        │   └── lib.rs
        └── README.md          # 플러그인 개발 가이드
```

## 시작하기

### 필수 요구사항

- Rust nightly (`rust-toolchain.toml`에 지정된 툴체인)
- Cargo

### 빌드 방법

```bash
# 전체 workspace 빌드
cargo build --release

# 개발 모드 빌드
cargo build

# 특정 크레이트만 빌드
cargo build -p app-gui
cargo build -p plugin-manager
```

### 실행 방법

```bash
# GUI 애플리케이션 실행
cargo run -p app-gui --release

# 개발 모드로 실행
cargo run -p app-gui
```

### 테스트 실행

```bash
# 전체 테스트 실행
cargo test --workspace

# 특정 크레이트 테스트
cargo test -p plugin-manager

# 통합 테스트만 실행
cargo test -p plugin-manager --test integration_test
```

## 사용 방법

### 기본 사용법

1. **애플리케이션 실행**: `cargo run -p app-gui`
2. **파일 선택**: "파일 선택..." 버튼을 클릭하여 변환할 파일 선택
3. **출력 형식 선택**: 드롭다운 메뉴에서 원하는 출력 형식 선택
4. **변환 실행**: "변환 시작" 버튼 클릭
5. **결과 확인**: 진행률 바를 통해 변환 진행 상태 확인

### 일괄 변환

파일 선택 다이얼로그에서 여러 파일을 선택하면 자동으로 일괄 변환 모드로 전환됩니다. 각 파일은 순차적으로 처리되며, 하나의 파일에서 오류가 발생해도 나머지 파일의 변환은 계속 진행됩니다.

### 변환 이력 조회

"이력" 탭에서 최근 100개의 변환 작업을 확인할 수 있습니다. 각 항목을 클릭하면 상세 정보를 볼 수 있습니다.

### 설정 변경

"설정" 탭에서 다음 항목을 변경할 수 있습니다:
- 기본 출력 디렉토리
- 테마 (Light/Dark/System)
- 언어

## 플러그인 시스템

### 플러그인 아키텍처

이 애플리케이션은 동적 플러그인 로딩을 지원합니다. 플러그인은 런타임에 로드되며, `Plugin` 트레이트를 구현하여 새로운 파일 형식 변환 기능을 추가할 수 있습니다.

### 플러그인 사용 방법

```rust
use plugin_manager::{PluginLoader, PluginRegistry};
use std::sync::Arc;

// 플러그인 레지스트리 생성
let registry = Arc::new(PluginRegistry::new());

// 플러그인 로더 생성
let mut loader = PluginLoader::new("./plugins");

// 모든 플러그인 로드 및 등록
let results = loader.load_all_plugins(&registry);

// 로드 결과 확인
for info in results {
    if info.success {
        println!("✓ Loaded: {} v{}", info.name, info.version);
    } else {
        println!("✗ Failed: {:?} - {}", 
                 info.file_path.display(), 
                 info.error_message.unwrap_or_default());
    }
}
```

### 플러그인 개발

새로운 플러그인을 개발하려면 `Plugin` 트레이트를 구현해야 합니다. 자세한 내용은 [플러그인 개발 가이드](plugins/text-converter/README.md)를 참조하세요.

#### 간단한 플러그인 예제

```rust
use plugin_interface::*;
use std::path::Path;

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "My Plugin".to_string(),
            version: "0.1.0".to_string(),
            author: "Your Name".to_string(),
            description: "My custom converter".to_string(),
        }
    }

    fn supported_input_formats(&self) -> Vec<FileFormat> {
        vec![FileFormat {
            extension: "txt".to_string(),
            mime_type: "text/plain".to_string(),
            description: "Plain Text".to_string(),
        }]
    }

    fn supported_output_formats(&self) -> Vec<FileFormat> {
        vec![FileFormat {
            extension: "md".to_string(),
            mime_type: "text/markdown".to_string(),
            description: "Markdown".to_string(),
        }]
    }

    fn can_convert(&self, from: &FileFormat, to: &FileFormat) -> bool {
        from.extension == "txt" && to.extension == "md"
    }

    fn convert(
        &self,
        input_path: &Path,
        output_format: &FileFormat,
        options: &ConversionOptions,
    ) -> Result<ConversionResult, Box<dyn std::error::Error>> {
        // 변환 로직 구현
        todo!()
    }
}

#[no_mangle]
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(MyPlugin)
}
```

## 아키텍처

### 컴포넌트 다이어그램

```
┌─────────────────────────────────────────────────────────┐
│                      GUI Module                         │
│                      (egui)                             │
└────────────┬────────────────────────────┬───────────────┘
             │                            │
             │ 변환 요청                   │ 이력/설정 조회
             ▼                            ▼
┌─────────────────────────┐    ┌──────────────────────────┐
│   Conversion Engine     │    │   Database Manager       │
│   (Core System)         │───▶│   (sqlite)               │
└────────────┬────────────┘    └──────────────────────────┘
             │
             │ 플러그인 조회/실행
             ▼
┌─────────────────────────┐
│   Plugin Registry       │
└────────────┬────────────┘
             │
             │ 등록
             ▼
┌─────────────────────────┐
│   Plugin Interface      │
│   (Trait)               │
└────────────┬────────────┘
             │
             │ 구현
             ▼
┌─────────────────────────────────────────┐
│  Plugins (Text, Image, etc.)            │
└─────────────────────────────────────────┘
```

### 데이터 흐름

1. 사용자가 GUI에서 파일 선택 및 변환 요청
2. GUI가 Core System의 ConversionEngine에 변환 요청
3. ConversionEngine이 PluginRegistry에서 적절한 플러그인 조회
4. 플러그인이 파일 변환 실행
5. 변환 결과가 Database에 저장
6. GUI에 결과 표시

## 에러 처리

애플리케이션은 다음과 같은 에러 상황을 처리합니다:

- **파일 읽기/쓰기 오류**: 원본 파일 보존, 부분 출력 파일 삭제
- **플러그인 로드 실패**: 오류 로깅, 사용자 알림, 앱 실행 계속
- **변환 실패**: 사용자 친화적인 에러 메시지 표시
- **일괄 변환 중 오류**: 실패한 파일 건너뛰기, 나머지 계속 처리

## 성능 고려사항

- **비동기 처리**: 대용량 파일 변환 시 UI 블로킹 방지
- **스레드 분리**: 변환 작업을 별도 스레드에서 실행
- **메모리 효율**: 스트리밍 방식으로 대용량 파일 처리 (플러그인 구현에 따라 다름)

## 개발 가이드

### 코드 스타일

```bash
# 코드 포맷팅
cargo fmt --all

# Clippy 린트 실행
cargo clippy --all-targets --all-features
```

### 새로운 크레이트 추가

1. `Cargo.toml`의 `members` 배열에 크레이트 경로 추가
2. 크레이트 디렉토리 생성 및 `Cargo.toml` 작성
3. workspace 의존성 활용: `dependency = { workspace = true }`

### 디버깅

```bash
# 로그 레벨 설정하여 실행
RUST_LOG=debug cargo run -p app-gui

# 특정 모듈만 로깅
RUST_LOG=plugin_manager=debug cargo run -p app-gui
```

## 라이선스

이 프로젝트는 MIT 라이선스 하에 배포됩니다.

## 기여

버그 리포트, 기능 제안, 풀 리퀘스트를 환영합니다!

## 문서

- [요구사항 문서](.kiro/specs/file-converter-app/requirements.md)
- [설계 문서](.kiro/specs/file-converter-app/design.md)
- [구현 계획](.kiro/specs/file-converter-app/tasks.md)
- [플러그인 개발 가이드](plugins/text-converter/README.md)
