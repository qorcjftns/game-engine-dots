//! 초기 CLI: stdout에는 JSON만 출력한다. 사용법은 README를 참고한다.

use std::{env, process::ExitCode};

use engine_core::{LOGICAL_HEIGHT, LOGICAL_WIDTH, TICKS_PER_SECOND, viewport::Viewport};

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            // 오류 코드는 내부의 고정 문자열이다. 사용자 입력은 JSON에 직접 넣지 않는다.
            println!("{{\"schema_version\":1,\"ok\":false,\"error\":\"{code}\"}}");
            ExitCode::from(2)
        }
    }
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
