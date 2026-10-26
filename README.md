# Engine-1

사람과 AI 에이전트가 함께 게임을 제작하는 **GUI 게임 에디터와 자체 2D 도트 엔진**입니다.
첫 지원 대상은 **macOS와 Windows**입니다. 게임 제작 도구가 제품이며,
실행 데모는 엔진의 출력을 확인하기 위한 예제입니다.

## 먼저 읽을 문서

- [설계와 언어 선택](docs/architecture.md): Rust를 선택한 이유와 엔진의 경계
- [기여와 리뷰 절차](CONTRIBUTING.md): 작업 단위, 브랜치, PR 작성과 검증 규칙
- [에이전트 접근 규약](docs/agent-interface.md): CLI와 향후 MCP의 공통 계약
- [전체 개발계획과 완료 기준](docs/roadmap.md): v1 정의, M1~M11, 다음 PR과 검증 기준
- [검증 기록](docs/verification.md): 작성한 것과 실제 확인한 것의 구분

모든 저장소 변경은 리뷰 가능한 단위의 PR로 전달합니다.
로컬 에이전트 지침과 설정은 저장소에서 제외합니다.

## 제품 목표와 현재 단계

v1의 목표는 GUI와 CLI·MCP에서 프로젝트를 편집하고, **사용자가 엔진 소스를 수정하지 않고
게임 행동을 작성해 Play·검증한 뒤 macOS·Windows용 독립 실행 게임으로 내보내는 것**입니다.
입력·기본 충돌·타일맵·애니메이션·게임 텍스트/HUD·오디오와 제작 중 오류 확인·복구까지 다룹니다.
추가된 M7 계획은 도트 캔버스의 수동 편집과 에이전트 대화로 정적 에셋을 생성·수정하는 통합 제작 경험입니다.
AI 후보는 비교·수동 보정·승인 후 새 버전으로 적용하며, provider 없이도 수동 편집할 수 있게 합니다.
전체 단계, 지원 범위와 최종 인수 조건은 [개발계획](docs/roadmap.md)을 기준으로 합니다.

현재 M1은 프로젝트 생성·PNG 배치·선택·이름/위치·드래그·undo/redo·저장·재열기까지 연결했습니다.
다음 M2는 같은 실행 중인 GUI 세션을 CLI·MCP로 함께 편집하는 단계입니다.
공동 세션·새 runtime·행동 코드·도트/대화형 에셋 제작·export는 아직 구현 전이며, 이 전체 계획에서 순서와 완료 기준을 정합니다.
검증용 게임/장면은 엔진의 기능을 확인하기 위한 fixture입니다.

GUI 패널과 글자는 OS의 DPI 배율을 따릅니다. 게임 뷰포트 안의 콘텐츠에만
320×180 같은 논리 해상도와 픽셀 확대 규칙을 적용합니다.

## 현재 구성과 예정 구성

```text
crates/engine-core/     현재: 화면 배치·시간 계산
crates/engine-document/ 현재: 프로젝트 문서·PNG 가져오기·편집 명령·저장·undo/redo
crates/engine-desktop/  현재: Macroquad 출력 검증 데모
crates/engine-editor/   현재: 프로젝트·PNG 배치·선택·속성·드래그·undo/redo GUI
crates/engine-protocol/ 현재: CLI 공통 JSON 응답·문서/장면 조회 자료형
crates/engine-cli/      현재: 설정·표시 영역·저장된 프로젝트/장면 JSON 조회
scripts/               공통 개발 명령
docs/                  설계, 역할, 인터페이스, 검증 기록
```

Rust를 유지하고 별도 `engine-editor`에 eframe 0.37.0과 wgpu 기반 GUI 에디터를 사용합니다.
프로젝트·PNG 장면 편집은 공통 문서 세션 API를 사용합니다.
기존 Macroquad 데모는 유지하되 에디터 UI로 확장하지 않습니다.

## 개발 환경과 명령

개발 명령은 Python **3.11 이상**을 사용합니다.
최종 플레이어가 Python을 사전 설치하지 않아도 실행되는 것은 v1 인수 조건입니다.
Rust **1.99.0**, Edition 2024로 설정되어 있습니다.
macOS에는 Xcode Command Line Tools, Windows에는 Visual Studio Build Tools의
**Desktop development with C++**와 Windows SDK가 필요합니다.

```sh
python3 scripts/dev.py doctor
python3 scripts/dev.py check
python3 scripts/dev.py editor
```

Rust가 없는 환경에서는 먼저 `python3 scripts/dev.py setup`을 실행합니다.
설치 스크립트는 공식 서버에서 `.tools/`에 도구를 설치하고 셸 PATH는 바꾸지 않습니다.
Windows에서는 `python3` 대신 `py -3`을 사용합니다.

| 명령 | 하는 일 |
| --- | --- |
| `setup` | 프로젝트 내부 Rust 도구와 의존성 준비 |
| `doctor` | 필수 도구 경로와 Cargo 버전 확인 |
| `fmt` | Rust 소스 포맷 정리 |
| `check` | lockfile 고정 상태에서 포맷·Clippy·Rust 테스트·빌드·CLI 계약 검사 |
| `test` | Rust 테스트 실행 |
| `editor` | GUI 게임 에디터 실행 |
| `run` | 기존 Macroquad 출력 검증 데모 실행 |
| `info` | 현재 코어의 기본 설정을 JSON으로 출력 |
| `viewport 1001 701` | 해당 창 크기의 픽셀 표시 영역 계산 |
| `project inspect FILE` | 저장된 프로젝트 JSON과 문서 모델 검사·조회 |
| `scene inspect FILE` | 실제 개체 ID·그리기 순서·좌표·자산 참조 조회 |
| `build` | 현재 운영체제용 릴리스 실행 파일 생성 |

예: `python3 scripts/dev.py info`. 릴리스 결과는 `target/release/`에 생성됩니다.
Windows는 `.exe`를 사용합니다. Mac 빌드 명령만으로 Windows 배포 파일까지 생성되지는 않습니다.
Rust 도구를 전역으로 설치한 경우 `cargo fmt --all`, `cargo test --workspace` 등도 직접 사용할 수 있습니다.
`Cargo.lock`은 버전 관리에 포함하며 검사·테스트·실행·릴리스 빌드는 `--locked`를 사용합니다.
의존성을 바꿀 때는 `setup`으로 lockfile을 갱신하고 변경 내역을 함께 리뷰합니다.

## 코드 위치

```text
crates/engine-core/     창 없이 쓸 수 있는 화면 배치·시간 계산
crates/engine-document/ GUI에 독립적인 프로젝트 문서·편집 명령·파일 처리
crates/engine-desktop/  Macroquad를 통한 창·입력·픽셀 화면 데모
crates/engine-editor/   GUI 편집 화면·입력·파일 대화상자·미저장 확인
crates/engine-protocol/ GUI에 독립적인 JSON 응답·문서/장면 조회 자료형
crates/engine-cli/      에이전트용 JSON 명령
scripts/               두 운영체제에서 공통으로 쓰는 개발 명령
docs/                  설계, 인터페이스, 검증 기록
```

`engine-core`에는 외부 라이브러리 의존성이 없습니다. 출력 검증 데모는 Macroquad **0.4.16**을 사용합니다.
데모는 320×180/초당 60틱이며, 에디터의 프로젝트별 논리 해상도 기본값도 **320×180**입니다.
새 프로젝트 폼에서 가로·세로를 각각 1~4096으로 정합니다.
게임 뷰포트는 물리 픽셀의 정수 배율과 nearest 필터로 확대하며,
논리 화면을 담을 수 없으면 창이나 패널 크기를 조절하라는 안내를 표시합니다.

## 확인된 상태

2026-10-25 macOS ARM64에서 현재 소스의 포맷·Clippy·Rust 테스트 40개와 doctest 1개·
workspace 빌드·CLI 계약 5개를 확인했습니다. 최종 편집 소스의 GitHub Actions는 macOS ARM64와 Windows x64 모두 통과했습니다.
실제 macOS/Retina 200%에서 PNG 배치·한글 개체 이름·좌표 수정·드래그·undo/redo·저장·재열기와
Cmd+Q의 저장·취소와 창 닫기의 변경 버리기를 확인했습니다. 상세 근거는 [검증 기록](docs/verification.md)에 있습니다.
GUI에 프로젝트 생성·열기, PNG 가져오기와 장면 배치, 개체 선택·속성·드래그·undo/redo와 저장·재열기를 연결했습니다.
Windows의 실제 GUI 실행과 DPI별 화면은 아직 확인하지 않았습니다.
CLI는 설정·표시 영역과 저장된 프로젝트/장면을 읽기 전용으로 조회합니다.
문서 편집 명령·실행 중 GUI 연결·MCP 서버는 아직 구현 전입니다.

예: `python3 scripts/dev.py scene inspect "내 프로젝트/project.json"`.
새 조회 응답은 `protocol_version: 1`, `source: "disk"`, `status: "ok"`와 `data`를 포함합니다.
실패는 `status: "error"`와 구조화된 `error`를 반환하고 종료 코드 2를 사용합니다.
프로젝트 파일을 변경하지 않으며 JSON/모델만 검사합니다. PNG 존재·디코딩과 GUI의 미저장 상태는 검사하지 않습니다.

## 현재 GUI에서 시도할 흐름

1. `python3 scripts/dev.py editor`로 실행합니다.
2. **파일 → 새 프로젝트…**에서 이름·논리 해상도를 입력하고 새 JSON 파일의 저장 위치를 선택합니다.
3. 리소스 패널의 **PNG 가져오기…**에서 이미지를 선택한 뒤 **장면에 배치**를 누릅니다.
4. 장면 목록이나 뷰포트에서 개체를 선택합니다. 속성의 이름·X·Y를 바꾸고 Enter 또는 다른 곳을 눌러 적용합니다.
5. 뷰포트에서 개체를 드래그합니다. 한 번의 드래그는 실행 취소 한 번으로 돌아갑니다.
6. **편집 → 실행 취소/다시 실행**으로 변경을 확인합니다. 텍스트를 입력 중이면 해당 입력창의 편집을 우선합니다.
7. **파일 → 저장**을 실행하고 **프로젝트 열기…**에서 같은 JSON을 열어 결과를 불러옵니다.

입력 중인 속성도 저장할 때 반영합니다. 미저장 문서가 있으면 프로젝트 전환·창 닫기·에디터 메뉴 종료·Cmd+Q에서
**저장 후 계속 / 변경 버리기 / 취소**를 선택하도록 연결했습니다.
macOS Dock 메뉴의 Quit와 운영체제 강제 종료는 이 보호 경로의 지원 범위가 아닙니다.
기존 파일을 덮어써서 새 프로젝트를 만들지는 않습니다.
