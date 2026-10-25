# 에이전트 접근 규약

## 현재 CLI

빌드 후 `target/debug/engine-cli`를 직접 호출합니다. Windows에서는 `.exe`를 붙입니다.
개발 중에는 `python3 scripts/dev.py info` 또는 `viewport WIDTH HEIGHT`를 사용할 수 있습니다.
CLI의 stdout은 JSON 결과 한 건, 진단 로그는 stderr, 성공은 종료 코드 0, 입력 오류는 2입니다.
개발 명령은 Cargo 실행 실패 등에서 별도 오류 코드를 낼 수 있으므로 자동화에는 직접 실행 파일을 권장합니다.

```sh
target/debug/engine-cli info
target/debug/engine-cli viewport 1001 701
```

```json
{"schema_version":1,"ok":true,"logical_width":320,"logical_height":180,"ticks_per_second":60}
```

```json
{"schema_version":1,"ok":true,"x":20,"y":80,"width":960,"height":540,"scale":3}
```

입력 크기는 `u32` 범위의 정수입니다. 잘못된 크기는 `invalid_window_size`,
화면을 담지 못하는 크기는 `window_too_small` 오류를 반환합니다.
아직 실행 중인 데스크톱 인스턴스에 연결하는 기능은 없습니다.

## MCP 연결 계획

[공식 Rust MCP SDK인 rmcp](https://github.com/modelcontextprotocol/rust-sdk)를 사용하고,
첫 전송 방식은 로컬 프로세스의 stdio로 정합니다. MCP 프로토콜은 직접 구현하지 않습니다.
SDK 버전은 구현 시점에 확인하고 고정합니다. 현재는 MCP 의존성과 서버가 없습니다.

| 도구 이름 초안 | 동작 | 현재 상태 |
| --- | --- | --- |
| `engine_info` | 버전·해상도·지원 명령 조회 | CLI의 info에 해당, MCP 미구현 |
| `engine_viewport` | 창 크기에 따른 표시 영역 계산 | CLI의 viewport에 해당, MCP 미구현 |
| `project_validate` | 프로젝트 파일 형식과 리소스 검사 | 프로젝트 형식부터 설계 필요 |
| `session_open` | 독립된 실행 세션 생성 | 미구현 |
| `scene_inspect` | 개체 ID·속성·현재 틱 조회 | 미구현 |
| `simulation_step` | 입력을 적용하고 지정한 틱만큼 진행 | 미구현 |
| `frame_capture` | 같은 틱의 픽셀 화면 저장 | 미구현 |

향후 CLI와 MCP는 공통 명령 계층을 호출합니다. MCP가 CLI 출력 문자열을 다시 해석하거나
게임 규칙을 복제하는 구조는 피합니다.

## 상태 변경 계약

다음 단계에서 명령마다 입력 스키마·출력 스키마·오류 코드·수정 여부를 정의합니다.
세션 ID로 병렬 작업을 격리하고, 예상 상태 버전으로 오래된 상태에 대한 수정을 검출합니다.
`simulation_step`은 틱 수를 명시하고, 무한 실행 대신 상한과 취소 방법을 제공합니다.
임의 코드 실행은 기본 도구로 노출하지 않습니다.
파일 출력은 선택한 프로젝트의 출력 폴더를 기준으로 경로를 검증합니다.

## 에이전트가 확인할 근거

명령 결과에는 현재 틱과 상태를 포함하고, 화면 캡처에도 해당 틱을 연결합니다.
상태 응답만 성공한 것과 실제 화면이 올바른 것을 별도로 검증합니다.
같은 초기 상태·입력·틱 수로 재현하는 회귀 시나리오를 첫 게임부터 유지합니다.
