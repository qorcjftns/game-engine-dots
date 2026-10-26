//! stdout은 공식 SDK stdio 전송에만 사용하고 시작 실패 진단은 stderr에 남깁니다.
use rmcp::{ServiceExt, transport::stdio};
use std::{ffi::OsString, path::PathBuf, process::ExitCode};
mod input;
mod server;
const USAGE: &str = "사용법: engine-mcp --connection-file ABSOLUTE_FILE\n연결 파일은 GUI가 만든 프로젝트 외부 사용자 전용 경로입니다.";

fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Option<PathBuf>, ()> {
    let arguments: Vec<_> = arguments.into_iter().collect();
    match arguments.as_slice() {
        [help] if help == "--help" => Ok(None),
        [option, path] if option == "--connection-file" => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                Ok(Some(path))
            } else {
                Err(())
            }
        }
        _ => Err(()),
    }
}
#[tokio::main]
async fn main() -> ExitCode {
    let path = match parse(std::env::args_os().skip(1)) {
        Ok(Some(path)) => path,
        Ok(None) => {
            eprintln!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(()) => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let service = match server::EditorTools::new(path).serve(stdio()).await {
        Ok(service) => service,
        Err(_) => {
            eprintln!("MCP 초기화가 중단됐습니다.");
            return ExitCode::from(2);
        }
    };
    match service.waiting().await {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("MCP 세션이 중단됐습니다. 편집 적용 여부는 같은 요청 ID로 확인하세요.");
            ExitCode::from(2)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_absolute_connection_configuration_is_accepted() {
        assert!(parse([]).is_err());
        assert!(parse(["--connection-file".into(), "relative.json".into()]).is_err());
        let path = std::env::temp_dir().join("한글 연결.json");
        assert_eq!(
            parse(["--connection-file".into(), path.clone().into_os_string()]).unwrap(),
            Some(path)
        );
        assert!(parse([OsString::from("--help")]).unwrap().is_none());
    }
}
