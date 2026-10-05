# SolidJS Frontend - Azure Bicep Lab

이 디렉토리는 Azure Bicep Lab 프로젝트의 SolidJS 프론트엔드 부분입니다. Rust(Actix-web) 백엔드와 통합되어 하나의 애플리케이션으로 배포됩니다.

## 기술 스택

- **Framework**: SolidJS 1.9.15 with TypeScript 7.0.2
- **Build Tool**: Vite 8.3.2
- **Compiler**: Babel (`babel-preset-solid` via `vite-plugin-solid`)
- **Package Manager**: pnpm 12.9.1

빌드는 TypeScript 7.0.2를 사용합니다. typescript-eslint의 API 호환성을 위해 `typescript`는 `@typescript/typescript6` 6.0.2 별칭으로 설치하고, `@typescript/native` 별칭에서 TypeScript 7의 `tsc`를 제공합니다. [Microsoft 공식 병행 설치 안내](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/#running-side-by-side-with-typescript-6.0)를 따릅니다.

Node.js는 `^20.19.0 || ^22.13.0 || >=24`가 필요합니다.

Solid 시그널과 `<For>`로 TODO 상태 및 목록을 관리합니다. 입력은 `onInput`으로 처리하고, 최초 요청은 `onMount`에서 실행하며 `onCleanup`에서 취소합니다. 개발 프록시는 Rust 백엔드의 `http://localhost:8000`을 사용합니다.

## 프로젝트 구조

```
src/frontend/
├── src/
│   ├── App.tsx           # 메인 SolidJS 컴포넌트
│   ├── App.css           # 스타일시트
│   └── main.tsx          # 애플리케이션 진입점
├── public/               # 정적 파일
├── package.json          # Node.js 의존성
├── vite.config.ts        # Vite 설정 (백엔드 통합)
└── tsconfig.json         # TypeScript 설정
```

## 개발 및 빌드

### 개발 서버 실행

```bash
# 의존성 설치
pnpm install

# 개발 서버 시작 (http://localhost:5173)
pnpm run dev
```

### 프로덕션 빌드

```bash
# 빌드 (결과물은 ../backend/wwwroot에 생성)
pnpm run build

# 빌드 결과 미리보기
pnpm run preview
```

### 검증

```bash
pnpm run lint
pnpm test
```

Vitest와 jsdom으로 TODO 추가·수정·취소·완료 전환·새로고침·삭제, 초기 요청 취소와 네트워크 실패 후 로딩 상태를 검증합니다.

## Vite 설정 특징

`vite.config.ts`에서 다음과 같이 설정되어 있습니다:

- **빌드 출력**: `../backend/wwwroot`로 설정하여 Rust 백엔드와 통합
- **개발 프록시**: `/api` 요청을 백엔드 서버로 프록시
- **Solid 컴파일러**: `vite-plugin-solid`로 JSX를 DOM 업데이트 코드로 변환

## API 통합

프론트엔드는 Rust(Actix-web) 백엔드 API와 통신합니다:

- `GET /api/todos` - TODO 목록 조회
- `POST /api/todos` - TODO 생성
- `PUT /api/todos/{id}` - TODO 수정
- `DELETE /api/todos/{id}` - TODO 삭제

개발 모드에서는 Vite 프록시를 통해, 프로덕션에서는 동일한 도메인에서 API에 접근합니다.

## 배포

이 SolidJS 앱은 빌드 시 자동으로 Rust 백엔드의 `wwwroot` 디렉토리에 배포되어 하나의 통합된 애플리케이션으로 동작합니다.
