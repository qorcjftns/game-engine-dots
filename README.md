# Engine-1

사람과 AI 에이전트가 함께 사용하는 개인용 **2D 도트 게임 엔진**입니다.
첫 실행 대상은 **macOS와 Windows**이며, 이름은 임시로 Engine-1을 사용합니다.

## 먼저 읽을 문서

- [설계와 언어 선택](docs/architecture.md): Rust를 선택한 이유와 엔진의 경계
- [기여와 리뷰 절차](CONTRIBUTING.md): 작업 단위, 브랜치, PR 작성과 검증 규칙
- [에이전트 접근 규약](docs/agent-interface.md): CLI와 향후 MCP의 공통 계약
- [개발 순서와 완료 기준](docs/roadmap.md): 현재 단계와 다음 단계
- [검증 기록](docs/verification.md): 작성한 것과 실제 확인한 것의 구분

모든 저장소 변경은 리뷰 가능한 단위의 PR로 전달합니다.
로컬 에이전트 지침과 설정은 저장소에서 제외합니다.

## 개발 환경 준비

Python **3.11 이상**과 인터넷 연결이 필요합니다. Python은 개발 명령 실행에만 사용하며,
배포 게임에는 필요하지 않습니다. Rust **1.99.0**, Edition 2024로 설정했습니다.

macOS에는 Xcode Command Line Tools가 필요합니다. `xcode-select -p`로 확인하고,
없으면 `xcode-select --install`을 실행합니다.
Windows에는 Visual Studio Build Tools의 **Desktop development with C++**와 Windows SDK가 필요합니다.
Windows x64를 첫 검증 대상으로 하고, ARM64는 별도 검증합니다.

```sh
# macOS: 저장소 루트에서 실행
python3 scripts/dev.py setup
python3 scripts/dev.py doctor
python3 scripts/dev.py fmt
python3 scripts/dev.py check
python3 scripts/dev.py run
```

Windows에서는 위 명령의 `python3`를 `py -3`으로 바꿉니다.
설치기는 공식 서버에서 Rust를 받아 `.tools/` 안에 설치하고 셸의 PATH를 수정하지 않습니다.
이후에는 `scripts/dev.py`가 이 도구 경로를 설정해 줍니다.
설치에 실패하면 원인을 해결한 뒤 같은 `setup` 명령을 다시 실행할 수 있습니다.

| 명령 | 하는 일 |
| --- | --- |
| `doctor` | 필수 도구 경로와 Cargo 버전 확인 |
| `fmt` | Rust 소스 포맷 정리 |
| `check` | 포맷 검사 → Clippy → Rust 테스트 → 빌드 → CLI 계약 검사 |
| `test` | Rust 테스트 실행 |
| `run` | 방향키로 사각형을 움직이는 픽셀 화면 데모 실행 |
| `info` | 엔진의 기본 설정을 JSON으로 출력 |
| `viewport 1001 701` | 해당 창 크기에서 표시 영역을 JSON으로 계산 |
| `build` | 현재 운영체제용 릴리스 실행 파일 생성 |

예: `python3 scripts/dev.py info`. 릴리스 결과는 `target/release/`에 생성됩니다.
Windows는 `.exe`를 사용합니다. Mac 빌드 명령만으로 Windows 배포 파일까지 생성되지는 않습니다.
Rust 도구를 전역으로 설치한 경우 `cargo fmt --all`, `cargo test --workspace` 등도 직접 사용할 수 있습니다.

## 코드 위치

```text
crates/engine-core/     창 없이 쓸 수 있는 화면 배치·시간 계산
crates/engine-desktop/  Macroquad를 통한 창·입력·픽셀 화면 데모
crates/engine-cli/      에이전트용 JSON 명령
scripts/               두 운영체제에서 공통으로 쓰는 개발 명령
docs/                  설계, 인터페이스, 검증 기록
```

코어에는 외부 라이브러리 의존성이 없습니다. 데스크톱 출력은 Macroquad **0.4.16**을 사용합니다.
현재 버전의 게임 화면은 **320×180**, 계산 속도는 **초당 60틱**입니다.
화면은 정수 배율과 nearest 필터로 확대하고 남는 영역은 검게 표시합니다.
320×180보다 작은 창에서는 게임 화면을 표시하지 않으며, 창을 다시 키우면 표시합니다.
프로젝트별 해상도 설정은 다음 단계에서 추가합니다.

## 현재 상태

프로젝트 골격, CLI 소스, 렌더링 데모 소스, 테스트와 설치 스크립트를 작성했습니다.
현재 실행 환경에는 Rust가 없고 다운로드 서버 DNS 조회가 실패하므로,
**Rust 설치·컴파일·테스트·실제 창 출력은 아직 검증하지 못했습니다.**
MCP 서버, 장면 편집기, 게임 저장 형식은 아직 구현하지 않았습니다.
`Cargo.lock`은 최초 `setup`의 의존성 다운로드가 성공하면 생성되며 이후 버전 관리에 포함합니다.
