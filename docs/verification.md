# 개발 기반과 계획 검증 기록

기준일: 2026-10-25. macOS ARM64 환경입니다.
소스 작성·도구 확인·기존 바이너리 실행·현재 소스 빌드·화면 확인을 구분합니다.

## 초기 환경 확인 이력

초기에는 Rust 도구가 없었고 공식 다운로드 서버의 DNS 조회가 실패했습니다.
당시 설치 오류는 `nodename nor servname provided, or not known`, curl 오류는
`Could not resolve host: static.rust-lang.org`였습니다.
Python 3.14.8, Apple clang 21.0.0과 설정 파일 문법·개발 명령의 기본 처리를 확인했습니다.
이 기록은 과거 상태이며 현재 도구 설치 상태를 뜻하지 않습니다.

## 현재 기반 검증 결과

원본 작업 폴더의 사용자 파일을 보존하고, 원격 main의 `f7d2a2fc` 기준 격리 작업 폴더에서 확인했습니다.
Rust 1.99.0 도구 체인과 기존에 다운로드된 의존성을 사용했습니다.

| 항목 | 현재 확인 범위 |
| --- | --- |
| 로컬 Rust 도구 | Rust 1.99.0 도구 체인과 rustc·cargo·rustfmt·Clippy 구성 요소 존재 |
| doctor | 통과 |
| 기존 target/debug CLI | info·viewport 정상 JSON 응답 |
| 현재 소스 재빌드 | workspace 빌드 통과 |
| 현재 소스 전체 테스트 | 코어 테스트 8개 통과 |
| Clippy·포맷 | 모든 workspace 대상 Clippy 경고 없음, rustfmt 검사 통과 |
| 새 빌드 CLI 계약 | 정상 응답·잘못된 입력 처리 5개 통과 |
| GUI 에디터 | 아직 구현 전 |
| macOS 실제 렌더링·입력 | 이번 확인에서 검증하지 않음 |
| Windows 빌드·실행 | CI에 빌드·테스트 구성, 원격 실행 결과와 실제 창 출력은 아직 미확인 |
| Cargo.lock | 원본 미추적 파일을 보존하고 동일 내용을 복사. `--locked` 검사 중 변경 없음 |

기존 바이너리의 응답 확인과 별도로, 이번에는 현재 소스를 빌드한 CLI의 계약 검사까지 실행했습니다.
실제 데스크톱 창의 렌더링·입력과 Windows 동작은 이 결과로 대신하지 않습니다.

## GUI 계획 문서 이력

GUI 에디터를 제품의 중심으로 재정의하고, GUI 셸 → 문서/명령·저장/열기 → PNG 배치 →
선택·속성·드래그 → undo/redo·미저장 보호 순서로 정리합니다.
GUI는 OS의 DPI 배율을 따르고, 게임 뷰포트의 콘텐츠만 논리 해상도를 사용합니다.
위 계획 PR은 문서만 변경했습니다. 이후 기반 PR에서 lockfile과 검사·CI 설정을 추가합니다.

## 기반 PR 검사 명령

```sh
python3 scripts/dev.py doctor
CARGO_NET_OFFLINE=true python3 scripts/dev.py check
```

macOS ARM64에서 두 명령이 통과했습니다. 기존 캐시만으로 검사하기 위해 offline 모드를 사용했습니다.
검사·테스트·실행·릴리스 빌드 명령은 `--locked`를 사용하며, lockfile이 맞지 않으면 실패합니다.
원본과 복사본의 Cargo.lock SHA-256은 모두 다음 값입니다.

```text
2f4eb81b8707a6fff39870316a125ac42ec0919df73d202f742dac160a219717
```

GitHub Actions는 `macos-15` ARM64와 `windows-2025` x64에서 도구 준비와 같은 검사를 실행합니다.
runner 라벨은 [GitHub 공식 지원 목록](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)을 확인했습니다.
checkout v7.0.1과 setup-python v7.1.0은 공식 태그의 commit SHA로 고정했습니다.
로컬에서 CI 설정을 작성한 것과 원격 CI가 실제 통과한 것은 별도로 기록합니다.

최초 원격 CI에서 macOS 검사는 통과했고, Windows는 Python의 cp1252 출력 인코딩으로
한국어 설치 안내를 출력하다 실패했습니다. 개발 스크립트의 stdout·stderr를 UTF-8로 고정해
수정하고, 로컬에서 `PYTHONIOENCODING=cp1252` 상태의 doctor·도움말 출력을 확인합니다.
수정 후 Windows CI 결과는 별도로 확인합니다.

## 다음 단계의 확인 항목

실제 GUI가 추가되면 운영체제·화면 크기·DPI·입력·저장/재열기 결과와 캡처를 기록합니다.
Windows에서는 `py -3`를 사용합니다. MCP는 구현 이후 초기화·목록·호출을 따로 검증합니다.
