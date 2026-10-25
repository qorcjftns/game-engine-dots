# Engine-1

사람과 AI 에이전트가 함께 게임을 제작하는 **GUI 게임 에디터와 자체 2D 도트 엔진**입니다.
첫 지원 대상은 **macOS와 Windows**입니다. 게임 제작 도구가 제품이며,
실행 데모는 엔진의 출력을 확인하기 위한 예제입니다.

## 먼저 읽을 문서

- [설계와 언어 선택](docs/architecture.md): Rust를 선택한 이유와 엔진의 경계
- [에이전트 역할](docs/agent-roles.md): 담당 영역, 작업 전달 방법, 사람이 읽기 쉬운 코드 규칙
- [에이전트 접근 규약](docs/agent-interface.md): CLI와 향후 MCP의 공통 계약
- [개발 순서와 완료 기준](docs/roadmap.md): 현재 단계와 다음 단계
- [검증 기록](docs/verification.md): 작성한 것과 실제 확인한 것의 구분

기존 루트 `AGENTS.md`는 최초 요청에 따라 수정하지 않았습니다.
현재 프로젝트 구성과 실행 방법은 이 README를 기준으로 확인합니다.

## 첫 번째 제품 목표

GUI에서 프로젝트를 생성·열고, PNG 스프라이트를 장면에 배치하고,
속성을 수정한 뒤 저장·재열기하여 결과를 유지할 수 있게 만듭니다.
장면 목록·뷰포트·속성 패널을 먼저 제공하고, 이후 실행 취소·다시 실행과 미저장 변경 보호를 추가합니다.
게임 규칙·충돌·타일맵·플레이 기능은 이 편집 흐름이 동작한 뒤 개발합니다.

GUI 패널과 글자는 OS의 DPI 배율을 따릅니다. 게임 뷰포트 안의 콘텐츠에만
320×180 같은 논리 해상도와 픽셀 확대 규칙을 적용합니다.

## 현재 구성과 예정 구성

```text
crates/engine-core/     현재: 화면 배치·시간 계산, 향후 공통 문서·편집 명령
crates/engine-desktop/  현재: Macroquad 출력 검증 데모
crates/engine-cli/      현재: 설정 조회·화면 배치 계산 JSON 명령
scripts/               공통 개발 명령
docs/                  설계, 역할, 인터페이스, 검증 기록
```

Rust는 유지합니다. 별도 `engine-editor`에 egui/eframe과 wgpu 기반 GUI를 추가할 계획입니다.
이 크레이트와 GUI는 아직 없으며, 라이브러리 숫자 버전은 실제 빌드 확인 후 결정합니다.
기존 Macroquad 데모는 유지하되 에디터 UI로 확장하지 않습니다.

## 개발 환경과 명령

Python **3.11 이상**은 개발 명령에만 필요하며 배포 게임에는 필요하지 않습니다.
Rust **1.99.0**, Edition 2024로 설정되어 있습니다.
macOS에는 Xcode Command Line Tools, Windows에는 Visual Studio Build Tools의
**Desktop development with C++**와 Windows SDK가 필요합니다.

```sh
python3 scripts/dev.py doctor
python3 scripts/dev.py check
```

Rust가 없는 환경에서는 먼저 `python3 scripts/dev.py setup`을 실행합니다.
설치 스크립트는 공식 서버에서 `.tools/`에 도구를 설치하고 셸 PATH는 바꾸지 않습니다.
Windows에서는 `python3` 대신 `py -3`을 사용합니다.

| 명령 | 하는 일 |
| --- | --- |
| `setup` | 프로젝트 내부 Rust 도구와 의존성 준비 |
| `doctor` | 필수 도구 경로와 Cargo 버전 확인 |
| `fmt` | Rust 소스 포맷 정리 |
| `check` | 포맷·Clippy·Rust 테스트·빌드·CLI 계약 검사 |
| `test` | Rust 테스트 실행 |
| `run` | 기존 출력 검증 데모 실행. GUI 에디터 실행 명령은 아직 없음 |
| `info` | 현재 코어의 기본 설정을 JSON으로 출력 |
| `viewport 1001 701` | 해당 창 크기의 픽셀 표시 영역 계산 |
| `build` | 현재 운영체제용 릴리스 실행 파일 생성 |

예: `python3 scripts/dev.py info`. 결과물은 `target/`에 생성되며 Windows 실행 파일은 `.exe`입니다.
Mac 빌드가 Windows 실행 검증을 대신하지 않습니다. `Cargo.lock`은 실제 의존성 해석 결과를 검토해 버전 관리에 포함합니다.

## 확인된 상태

2026-10-25 현재 로컬 Rust 도구 1.99.0과 `doctor` 통과가 확인되었습니다.
기존 `target/debug` CLI의 `info`·`viewport` 응답도 확인했습니다.
이는 현재 소스의 재빌드·전체 테스트·GUI 실행을 확인한 결과는 아닙니다.
GUI 에디터, 프로젝트 저장 형식, 공통 편집 명령과 MCP 서버는 아직 구현 전입니다.
다음 기반 PR에서 기존 코드와 lockfile을 검증하면서 첫 GUI 셸 작업을 준비합니다.
