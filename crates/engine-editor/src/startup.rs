//! 원격 제어는 기본적으로 끈다. 비밀 연결 파일을 지정한 명시적 실행만 허용한다.

use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AgentControlOptions {
    pub connection_file: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Startup {
    Run(AgentControlOptions),
    Help,
}

pub const USAGE: &str = "사용법: engine-editor [--agent-control --connection-file ABSOLUTE_FILE]\n\
    연결 파일은 프로젝트 밖의 사용자 전용 경로를 지정하세요. 기본 실행은 에이전트 연결을 열지 않습니다.";

pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Startup, &'static str> {
    let mut arguments = arguments.into_iter();
    let mut enabled = false;
    let mut connection_file = None;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--help") => return Ok(Startup::Help),
            Some("--agent-control") if !enabled => enabled = true,
            Some("--connection-file") if connection_file.is_none() => {
                let path = PathBuf::from(arguments.next().ok_or("연결 파일 경로가 필요합니다.")?);
                if !path.is_absolute() {
                    return Err("연결 파일은 프로젝트 밖의 절대 경로로 지정하세요.");
                }
                connection_file = Some(path);
            }
            _ => return Err("지원하지 않거나 중복된 실행 인자입니다."),
        }
    }
    if enabled != connection_file.is_some() {
        return Err("--agent-control과 --connection-file을 함께 지정하세요.");
    }
    Ok(Startup::Run(AgentControlOptions { connection_file }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_launch_does_not_enable_control() {
        assert_eq!(
            parse([]).unwrap(),
            Startup::Run(AgentControlOptions::default())
        );
    }

    #[test]
    fn an_explicit_flag_and_external_absolute_path_enable_control() {
        let path = std::env::temp_dir().join("engine-review-한글.json");
        let result = parse([
            OsString::from("--agent-control"),
            OsString::from("--connection-file"),
            path.clone().into_os_string(),
        ])
        .unwrap();
        assert_eq!(
            result,
            Startup::Run(AgentControlOptions {
                connection_file: Some(path)
            })
        );
    }

    #[test]
    fn incomplete_relative_and_duplicate_configuration_is_rejected() {
        for arguments in [
            vec!["--agent-control"],
            vec!["--connection-file", "relative.json"],
            vec!["--agent-control", "--agent-control"],
            vec!["--connection-file"],
        ] {
            assert!(parse(arguments.into_iter().map(OsString::from)).is_err());
        }
    }
}
