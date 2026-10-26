//! 초기 CLI: stdout에는 JSON만 출력한다. 사용법은 README를 참고한다.

use std::{env, process::ExitCode};

use engine_core::{LOGICAL_HEIGHT, LOGICAL_WIDTH, TICKS_PER_SECOND, viewport::Viewport};
use engine_protocol::{ApiResponse, DataSource};

mod inspect;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|command| command == "project" || command == "scene")
    {
        return match inspect::run(&arguments) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                print_response(ApiResponse::<()>::failure(DataSource::Disk, error));
                ExitCode::from(2)
            }
        };
    }
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            // 오류 코드는 내부의 고정 문자열이다. 사용자 입력은 JSON에 직접 넣지 않는다.
            println!("{{\"schema_version\":1,\"ok\":false,\"error\":\"{code}\"}}");
            ExitCode::from(2)
        }
    }
}

fn print_response<T: serde::Serialize>(response: ApiResponse<T>) {
    // 응답은 UTF-8 문자열·정수·배열·구조체만 사용합니다.
    println!(
        "{}",
        serde_json::to_string(&response).expect("inspection response is serializable")
    );
}

fn run(arguments: &[String]) -> Result<(), &'static str> {
    match arguments {
        [command] if command == "info" => {
            println!(
                "{{\"schema_version\":1,\"ok\":true,\
                \"logical_width\":{LOGICAL_WIDTH},\
                \"logical_height\":{LOGICAL_HEIGHT},\
                \"ticks_per_second\":{TICKS_PER_SECOND}}}"
            );
            Ok(())
        }
        [command, width, height] if command == "viewport" => {
            let width = width.parse::<u32>().map_err(|_| "invalid_window_size")?;
            let height = height.parse::<u32>().map_err(|_| "invalid_window_size")?;
            let viewport = Viewport::fit(width, height).ok_or("window_too_small")?;
            println!(
                "{{\"schema_version\":1,\"ok\":true,\
                \"x\":{},\"y\":{},\"width\":{},\"height\":{},\"scale\":{}}}",
                viewport.x, viewport.y, viewport.width, viewport.height, viewport.scale
            );
            Ok(())
        }
        _ => Err("usage_info_or_viewport_width_height"),
    }
}
