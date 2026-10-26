# GUI·CLI·MCP 접근 규약

전체 v1 범위·순서·인수 조건은 [개발계획](roadmap.md)을 기준으로 합니다.
현재 CLI는 info·viewport, 저장된 프로젝트/장면 조회와 opt-in GUI live 조회를 지원합니다.
같은 GUI 세션의 기존 스프라이트 변경·이력·저장과 공식 SDK 기반 MCP stdio 어댑터를 지원합니다.

```sh
target/debug/engine-cli info
target/debug/engine-cli viewport 1001 701
target/debug/engine-cli project inspect "내 프로젝트/project.json"
target/debug/engine-cli scene inspect "내 프로젝트/project.json"
target/debug/engine-cli session inspect --connection-file "/사용자/외부경로/editor-connection.json"
```

Windows 실행 파일은 `.exe`를 사용합니다. 현재 계약은 stdout JSON 한 건, 진단 로그 stderr,
성공 종료 0·입력 오류 2입니다. 새 계약을 추가해도 기존 동작을 유지합니다.

`project inspect`는 canonical `project_file`과 기존 `document` 전체를 반환합니다.
`scene inspect`는 `project_file`, `project_name`, 논리 해상도, `scene.entities`, `assets`를 반환하며 ID와 그리기 순서를 바꾸지 않습니다.
두 명령은 JSON과 문서 모델만 검증합니다. PNG 존재·디코딩, GUI의 저장 전 상태는 확인하지 않고 파일도 수정하지 않습니다.

새 조회는 프로젝트의 `schema_version`과 별개인 `protocol_version: 1`, `source: "disk"`를 사용합니다.
성공은 `status: "ok"`와 `data`, 실패는 `status: "error"`와 `error: {code, message}`를 포함합니다.
오류 코드는 `invalid_arguments`, `project_not_found`, `project_access_denied`, `project_io`, `invalid_project`입니다.
잘못된 JSON·미지원 형식 버전·모델 오류는 `invalid_project`로 묶고 이유를 message에 표시합니다. 실패 종료는 2입니다.
기존 info·viewport와 그 외 legacy 오류의 `schema_version`/`ok` 출력 계약은 바꾸지 않습니다.

## 현재 live 조회 계약

GUI를 `--agent-control --connection-file FILE`로 명시 실행하면 IPv4 loopback의 임시 포트만 엽니다.
파일은 프로젝트 외부 사용자 전용 경로에 두며 기존 파일은 덮어쓰지 않습니다.
비밀 token을 포함하므로 Git·로그·응답에 복사하지 않습니다. 정상 종료는 자신이 만든 연결 파일만 지웁니다.
비정상 종료 후 남은 파일은 재연결 성공을 뜻하지 않습니다. 기존 인스턴스를 확인한 뒤 별도 경로를 선택합니다.

`session inspect`는 `protocol_version: 1`, `source: "live"`로 응답합니다.
`data`는 `session_id`, 단조 `revision`, `busy`, `is_dirty`, `can_undo`, `can_redo`,
`project`(없으면 null, 있으면 `project_file`·확정 `document`)입니다.
저장하지 않은 확정 변경은 포함하고 TextEdit draft·드래그 미리보기는 제외합니다.
조회는 파일을 다시 열거나 PNG를 디코딩하지 않으며 busy 중에도 마지막 게시 상태를 반환합니다.
문서가 바뀌면 undo/redo도 revision을 증가시키고 저장·busy 전환은 증가시키지 않습니다.
재열기·프로젝트 교체·재시작은 새 session ID가 됩니다.

JSONL 한 요청/응답 뒤 연결을 닫습니다. 요청 8 KiB·응답 4 MiB, frame 전체 읽기/쓰기 기한 2초,
작업 스레드 4개·대기 연결 8개로 제한합니다. 초과 요청은 `request_too_large`, 큰 응답은
`response_too_large`, 포화는 `server_busy`입니다. 연결/기한 실패는 `control_io`, 잘못된 연결 파일은
`invalid_connection_file`, 인증은 `unauthorized`, 메시지는 `invalid_request`, 버전은 `unsupported_protocol`입니다.
에디터는 명시적 opt-in 실행에서 조회와 다음 변경 요청을 지원합니다. 전체 TCP 회귀는 두 OS CI에서 필수 실행하고,
소켓이 제한된 로컬 부분 검사는 실제 TCP·native GUI 교차 확인과 구분해 기록합니다.

## 현재 live 변경과 결과 조회

먼저 inspect로 session ID·revision과 기존 asset/entity ID를 확인합니다. 요청 파일 예시:

```json
{
  "expected_session_id": "inspect에서 받은 session_id",
  "expected_revision": 3,
  "request_id": "move-hero-001",
  "timeout_ms": 5000,
  "command": {"type": "move_entity", "id": 73, "position": {"x": 60, "y": 38}}
}
```

```sh
engine-cli session mutate --connection-file /사용자/외부경로/control.json --request-file move.json
engine-cli session result --connection-file /사용자/외부경로/control.json --session-id SESSION_ID --request-id move-hero-001
```

명령은 `add_sprite`(기존 asset_id·name·position), `remove_entity`(id), `rename_entity`(id·name),
`move_entity`(id·position), `undo`, `redo`, `save`입니다. 새 프로젝트 열기/생성·PNG 가져오기·임의 파일 쓰기는 지원하지 않습니다.
변경 파일은 읽기만 하며 실제 편집/저장은 GUI가 소유한 EditorSession에서 직렬 실행합니다.
요청 ID는 영숫자/-/_ 1~128자, timeout_ms는 1~10000입니다. 기한은 **서버 접수 뒤 대기 시간**이며
네트워크 전체 왕복 시간이나 이미 시작한 저장의 강제 취소를 보장하지 않습니다.

성공 응답 `data`는 session_id·request_id·state를 포함하는 receipt입니다.
`queued`/`running`은 적용 성공이 아닙니다. `applied`는 revision·is_dirty·effect.entity_id(추가 외에는 null),
`rejected`는 revision·error, `expired_before_apply`는 미적용 만료입니다.
stdout의 `status: "ok"`/종료 0은 receipt를 받았다는 뜻이며 rejected도 포함할 수 있습니다.
입력/전송/admission 오류는 기존처럼 `status: "error"`/종료 2입니다. 적용 여부는 반드시 state로 판정합니다.

대기열 32개, 원장 1024개/세션으로 제한합니다. 원장은 항목을 퇴출하지 않고 꽉 차면 새 ID를 `ledger_full`로 거부합니다.
기존 ID의 결과 조회와 동일 요청 재전송은 계속 가능하며 다른 payload 재사용은 `request_conflict`입니다.
정상 접수한 요청의 busy/stale/명령·저장 실패/만료도 원장에 남고 문서 전체는 receipt에 복사하지 않습니다.
인증·잘못된 형식·다른 세션·queue_full·ledger_full 등 접수 전 오류는 원장에 등록하지 않습니다.
GUI는 실행 직전에 session/revision/busy/deadline을 다시 확인합니다. 사람 draft를 확정하거나 취소하지 않습니다.
저장 실패는 문서·이력·기존 파일을 보존하며 undo/redo로 revision을 뒤로 돌리지 않습니다.

응답을 잃었으면 같은 session/request ID로 result를 조회하거나 **동일 payload**만 재전송합니다.
현재 세션의 `unknown_request`도 미적용 증거로 단정하지 않습니다. 새 ID로 blind retry하지 않습니다.
프로젝트 교체·재시작은 새 session ID와 원장을 만들고 이전 queue를 버립니다. 이전 결과를 영속 보관하지 않습니다.

## MCP stdio 사용

[공식 Rust SDK rmcp](https://github.com/modelcontextprotocol/rust-sdk)의 stdio 서버를 사용합니다.
MCP 클라이언트의 서버 설정에서 실행 파일을 `target/debug/engine-mcp`(Windows는 `.exe`),
인자를 `--connection-file /사용자/외부경로/control.json`으로 지정합니다. 경로는 절대 경로여야 합니다.
MCP 서버 프로세스 자체가 GUI를 열거나 연결 파일을 새로 만들지는 않습니다. 실행 중인 opt-in GUI가 필요합니다.
버전 처리는 rmcp에 맡깁니다. 실제 stdio 회귀는 2025-11-25의 initialize/list/call,
2024-11-05 초기화 호환, 2026-07-28의 요청 metadata 기반 no-initialize list/call을 확인합니다.
SDK의 structured 응답 helper가 JSON과 텍스트 content를 함께 제공하며 별도 호환 계층은 만들지 않습니다.

| 도구 | 입력 | 반환 |
| --- | --- | --- |
| `session_inspect` | 빈 객체 `{}` | 현재 확정 문서·session/revision·busy |
| `session_mutate` | 위 MutationRequest와 같은 필드·typed command | queued/running/applied/rejected/expired receipt |
| `session_result` | `session_id`, `request_id` | 같은 공통 원장의 receipt |

먼저 inspect → 그 session/revision으로 mutate → result에서 terminal state 확인 순서로 사용합니다.
MCP의 JSON-RPC id와 편집 중복 방지용 request_id는 별개입니다. transport 응답·도구 호출 성공은 편집 적용 완료가 아닙니다.
`structuredContent`는 CLI와 같은 protocol_version/source/status/data 또는 error 계약을 담습니다.
공통 client 오류는 MCP `isError: true`로 표시하고, 정상 조회한 rejected receipt는 data/state/error 그대로 전달합니다.
알 수 없는 명령·누락 필드·잘못된 정수·추가 필드는 SDK typed 입력 단계에서 거부하며 임의 JSON을 명령으로 추측하지 않습니다.

MCP는 control client를 직접 호출하고 CLI 출력 파싱·별도 문서/이력·프로젝트 직접 쓰기 경로를 만들지 않습니다.
전송 실패/취소 뒤 이미 접수됐을 가능성이 있으면 동일 session/request ID 결과를 확인합니다. 자동 새 ID 재시도는 하지 않습니다.
stdio stdout은 SDK wire 전용이며 비밀 token과 요청 원문을 진단 로그로 내보내지 않습니다.
실제 SDK initialize/list/call process 검사, GUI↔CLI↔MCP 교차 적용·재조회·저장/재열기는 별도 검증 근거로 기록합니다.

## M2: 같은 편집 세션 사용

도입 순서는 읽기 전용 디스크 검사/조회 → GUI live 조회 → 기존 편집·undo/redo·저장 CLI →
[공식 Rust MCP SDK rmcp](https://github.com/modelcontextprotocol/rust-sdk)의 stdio 어댑터입니다.
GUI 하나가 연 프로젝트 하나를 대상으로 하며 CLI·MCP가 별도 문서를 만들어 파일을 덮어쓰지 않습니다.
세 도구는 공통 계약·control client를 재사용하며 추가 기능의 세부 계약은 구현 때 검증합니다.

조회 결과에서 저장 파일인지 실행 중인 문서인지 구분합니다.
입력·드래그·대화상자 중에도 읽기는 확정 문서·revision·busy 상태를 반환하고,
미확정 draft를 적용된 문서로 내보내지 않습니다. 이때 외부 변경·undo/redo·저장은 `editor_busy`로 거부합니다.
사람의 입력을 강제로 확정·취소하거나 저장하지 않습니다.

다시 열기·재시작 때 session ID를 교체합니다. revision은 세션 안에서 단조 증가하며 undo로 되돌리지 않습니다.
변경 요청은 대상 세션·예상 revision·request ID를 포함합니다. 오래된 세션/상태는 적용 전에 거부합니다.
성공한 명령은 같은 GUI 표시·선택·캐시·이력·dirty에 반영하고, 저장 실패 시 문서·이력·기존 파일을 보존합니다.

## 큐 만료와 응답 유실

같은 request ID와 같은 요청의 재시도는 중복 적용하지 않고 같은 ID의 다른 내용은 거부합니다.
요청 결과를 조회할 수 있게 하고 보관 범위·조회 불가능한 경우를 구현 PR에 명시합니다.

- 큐에서 대기하다 deadline이 지난 요청은 적용하지 않습니다.
- 이미 적용한 뒤 응답을 잃은 경우는 미적용 실패와 다릅니다. 같은 ID로 결과를 확인합니다.
- 결과를 모른 채 새 ID로 재시도하지 않습니다. 새 세션이라 확인할 수 없으면 현재 상태를 다시 확인합니다.

예: 요청 A가 위치를 `(60, 38)`로 바꿨지만 응답을 받지 못했다면 새 요청 B를 보내기 전에 A의 결과를 조회합니다.
최소화·가려짐 중에도 요청 기한을 지키고, 닫힌 세션의 요청이 나중에 다른 문서에 적용되지 않게 합니다.

## M3 이후: 편집과 실행의 구분

실행은 편집 문서/자산 snapshot에서 시작합니다. 편집 세션과 실행 세션의 식별자를 구분하고,
실행 결과에 tick·대상 snapshot의 근거를 연결합니다. runtime 상태를 편집 문서에 자동 저장하지 않습니다.
초기 Play 중에는 편집 변경을 거부하고 stop 뒤 재개합니다.

Play·pause·정확한 N tick·상태/오류/화면 조회를 GUI와 에이전트가 같은 실행 경로에서 사용합니다.
제어 요청의 실행 한도·중단·응답 기한을 정하며, 오래된 실행 세션에 새 요청을 적용하지 않습니다.
화면 캡처에는 해당 상태의 tick/버전을 연결해 JSON 응답 성공과 실제 화면을 따로 검사합니다.

M4의 프로젝트 행동 코드 작성/검증은 엔진을 고치지 않는 지원 경로가 됩니다.
소스/리소스 변경도 버전·오류·저장 보존 규칙을 지키며, 행동 코드는 선택한 제한된 호스트 API와 실행 예산 안에서 동작합니다.
이는 MCP 도구에서 임의 호스트 코드·shell을 실행하도록 허용한다는 뜻이 아닙니다.

## 새 기능을 도구에 연결하는 기준

| 단계 | 추가할 접점 | 함께 확인할 것 |
| --- | --- | --- |
| M2 | 프로젝트/장면 조회·기존 개체 편집·이력·저장 | 같은 문서·busy·stale·중복/유실·GUI 즉시 반영 |
| M3 | 실행 제어·입력·tick·상태/화면 | snapshot 원본 보존·정확한 step·별도 실행 세션 |
| M4 | 행동 리소스 작성/검증·진단 | 코드 오류·중단·허용 API·export 호환 |
| M5~M6 | 충돌 속성·타일 stroke·장면 참조 | 같은 명령/undo·형식 전환·좌표/접촉 결과 |
| M7 | 픽셀 도구·대화 문맥·job·후보 승인/적용 | 오프라인 편집·원본 보존·expected version·비용/취소 상태 |
| M8~M9 | 클립·텍스트/폰트·오디오 속성/재생 | 시간·자산 수명·실제 화면/소리·패키지 포함 |
| M10~M11 | export 검사·인수 결과 | 지원 타깃·누락 자산·독립 실행·기능별 진단 |

이 표는 지원 시점의 방향이며 전송 필드·함수 이름을 미리 확정하지 않습니다.
새 GUI 기능만 먼저 완성하고 도구 노출을 무기한 미루지 않습니다. 공통 모델/명령과 어댑터의 차이를 검증합니다.
기능 상태·파일 저장·GUI 출력·실제 연결·패키지 실행을 각각 확인하고 미검증 환경을 분리합니다.

## M7: 도트 작업과 생성 job

에이전트는 선택한 픽셀 작업 세션의 공통 도구 명령과 별도의 생성 job을 사용합니다.
대화 backend와 bitmap provider를 구분하며 prompt·참조/버전·크기·팔레트·선택 영역·스타일의 범위를 명시합니다.
채팅 결과를 임의 코드나 프로젝트 쓰기로 실행하지 않습니다. 키는 외부 사용자 설정에만 두고 요청/오류 로그에도 노출하지 않습니다.

job 결과는 후보이며 적용 명령이 아닙니다. 지원되는 견적/실제 사용량과 알 수 없는 비용을 구분하고,
명시적 실행 없이 유료 job을 새로 만들지 않습니다. 취소 가능 범위와 결과 불명을 표시하고 무과금 취소를 보장하지 않습니다.
늦은 결과·stale 기준·취소 불명은 자동 적용/재시도하지 않습니다.

규격 검사·명시 정규화·비교/수동 보정·승인 뒤 기대 자산 버전을 검사해 새 버전을 적용합니다.
영역 밖 픽셀 보존과 consumer/tileset/frame 참조·cache·undo·재열기를 확인합니다.
undo/redo·재열기는 보존한 버전을 사용하며 외부 생성을 다시 호출하지 않습니다.
provider 미연결 상태의 수동 편집도 같은 도구 명령으로 검사합니다. 정적 PNG 뒤의 frame 연계는 M8 gate를 따릅니다.

## 범위와 배포 경계

v1의 공동 편집은 로컬 GUI 한 개/프로젝트 한 개가 기준입니다. 원격 공동 편집 서비스·다중 GUI 협업을 포함하지 않습니다.
MCP가 CLI 출력 문자열을 재해석하거나 별도 편집 규칙/자체 MCP 프로토콜을 구현하지 않습니다.
export된 일반 게임이 편집 서버·대화/이미지 provider·그 키를 필수로 요구하거나 개발용 제어/디버그 기능을 자동 노출하지 않게 합니다.
인증·연결/도구 실행과 프로젝트 게임 코드의 실행 경계를 구분해 검사합니다.

## M3 실행 조회와 제어

`session inspect`/MCP `session_inspect`의 `runtime`은 정지 중 `null`, 실행 중 `run_id`, `paused`, `tick`, `frame`, `input`, `pressed`, `released`, `frame_hash`를 포함합니다.
입력 값은 마지막 완료 tick의 값입니다. frame은 표시 요청 횟수이며 GPU 완료를 보장하지 않습니다. frame_hash는 공통 RGBA의 FNV-1a 64bit 회귀 식별자이며 암호학적 무결성 용도가 아닙니다.
실행은 문서 revision을 바꾸지 않고, 매 Play의 run ID가 바뀝니다. 요청은 기존 편집 session ID/revision/request ID/timeout 규칙을 그대로 사용합니다.

```json
{"expected_session_id":"조회한 세션 ID","expected_revision":0,"request_id":"play-1","timeout_ms":5000,"command":{"type":"run_start"}}
```

이 JSON을 파일로 저장해 `engine-cli session mutate --connection-file CONNECTION --request-file REQUEST`로 보내거나 MCP `session_mutate` 인수로 전달합니다.
결과는 기존 `session result`/`session_result`로 확인합니다. `run_start` 후 조회한 run ID를 다음 명령에 사용합니다.

```json
{"type":"run_pause","run_id":1,"paused":true}
{"type":"run_step","run_id":1,"ticks":73,"input":{"left":true,"right":false,"up":false,"down":false,"primary":false,"pointer":null}}
{"type":"run_stop","run_id":1}
```

Step은 Pause에서 1~100000 tick을 정확히 처리합니다. 명시적 input은 마지막 완료 tick을 기준으로 비교하므로 중간 GUI frame의 포커스 처리와 무관합니다.
같은 request ID를 재전송하면 기존 결과만 반환하며 tick이 중복 진행되지 않습니다. 오래된 run ID는 `run_mismatch`로 거부합니다.
실행 중 문서 변경은 `editor_busy`로 거부하지만 실행 조회와 제어는 가능합니다. Stop 후 기존 문서 편집을 다시 사용할 수 있습니다.
