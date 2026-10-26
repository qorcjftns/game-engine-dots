//! 저장된 JSON과 문서 모델만 검사합니다. PNG 존재/디코딩과 live 조회는 별도 기능입니다.

use engine_document::EditorSession;
use engine_protocol::{
    ApiError, ApiResponse, DataSource, ErrorCode, ProjectInspection, SceneInspection,
};

use super::print_response;

pub(crate) fn run(arguments: &[String]) -> Result<(), ApiError> {
    let [kind, command, project_file] = arguments else {
        return Err(usage());
    };
    if command != "inspect" {
        return Err(usage());
    }

    // open은 JSON/모델 검증만 하며 저장·가져오기·명령 적용을 하지 않습니다.
    let session = EditorSession::open(project_file).map_err(ApiError::from)?;
    let project_file = session.project_file().display().to_string();
    let document = session.document();
    match kind.as_str() {
        "project" => print_response(ApiResponse::success(
            DataSource::Disk,
            ProjectInspection {
                project_file,
                document: document.clone(),
            },
        )),
        "scene" => print_response(ApiResponse::success(
            DataSource::Disk,
            SceneInspection {
                project_file,
                project_name: document.name.clone(),
                logical_width: document.logical_width,
                logical_height: document.logical_height,
                scene: document.scene.clone(),
                assets: document.assets.clone(),
            },
        )),
        _ => return Err(usage()),
    }
    Ok(())
}

fn usage() -> ApiError {
    ApiError::new(
        ErrorCode::InvalidArguments,
        "사용법: engine-cli project inspect FILE 또는 engine-cli scene inspect FILE",
    )
}
