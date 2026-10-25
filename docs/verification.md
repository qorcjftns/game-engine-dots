# 개발 기반과 계획 검증 기록

기준일: 2026-10-25. macOS ARM64 환경입니다.
소스 작성·도구 확인·기존 바이너리 실행·현재 소스 빌드·화면 확인을 구분합니다.

## 초기 환경 확인 이력

초기에는 Rust 도구가 없었고 공식 다운로드 서버의 DNS 조회가 실패했습니다.
당시 설치 오류는 `nodename nor servname provided, or not known`, curl 오류는
`Could not resolve host: static.rust-lang.org`였습니다.
Python 3.14.8, Apple clang 21.0.0과 설정 파일 문법·개발 명령의 기본 처리를 확인했습니다.
이 기록은 과거 상태이며 현재 도구 설치 상태를 뜻하지 않습니다.

## 현재 readiness 확인 결과

다음 결과는 기반 검토 담당의 확인을 전달받아 기록합니다. 이 문서 작성 작업에서는 설치·빌드하지 않았습니다.

| 항목 | 현재 확인 범위 |
| --- | --- |
| 로컬 Rust 도구 | Rust 1.99.0 도구 체인과 rustc·cargo·rustfmt·Clippy 구성 요소 존재 |
| doctor | 통과 |
| 기존 target/debug CLI | info·viewport 정상 JSON 응답 |
| 현재 소스 재빌드 | 이번 확인에서 실행하지 않음 |
| 현재 소스 전체 테스트·Clippy·포맷 | 이번 확인에서 실행하지 않음 |
| GUI 에디터 | 아직 구현 전 |
| macOS 실제 렌더링·입력 | 이번 확인에서 검증하지 않음 |
| Windows 빌드·실행 | 이번 확인에서 검증하지 않음 |
| Cargo.lock | 원본 작업 폴더의 기존 미추적 파일 보존. 별도 기반 PR에서 검토 예정 |

기존 바이너리 응답은 현재 소스와 동일한 결과물임을 보장하지 않습니다.
코어 테스트 8개와 CLI 계약 검사 5개는 소스에 존재하지만 현재 소스 검사 통과로 기록하지 않습니다.

## 이번 계획 문서 변경

GUI 에디터를 제품의 중심으로 재정의하고, GUI 셸 → 문서/명령·저장/열기 → PNG 배치 →
선택·속성·드래그 → undo/redo·미저장 보호 순서로 정리합니다.
GUI는 OS의 DPI 배율을 따르고, 게임 뷰포트의 콘텐츠만 논리 해상도를 사용합니다.
엔진 소스·manifest·Cargo.lock은 이번 계획 변경에 포함하지 않습니다.

문서의 로컬 링크와 git diff 공백 검사를 수행하며 결과는 PR에 기록합니다.
빌드·실제 창 출력·Windows 검증은 이 문서 검사로 대신하지 않습니다.

## 다음 기반 PR의 확인 항목

```sh
python3 scripts/dev.py doctor
python3 scripts/dev.py check
```

현재 소스를 대상으로 검사하고 lockfile과 macOS·Windows CI를 검토합니다.
필요한 포맷 수정은 해당 기반 PR에서 처리합니다.
실제 GUI가 추가되면 운영체제·화면 크기·DPI·입력·저장/재열기 결과와 캡처를 기록합니다.
Windows에서는 `py -3`를 사용합니다. MCP는 구현 이후 초기화·목록·호출을 따로 검증합니다.
