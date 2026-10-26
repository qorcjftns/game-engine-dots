use engine_protocol::{ApiError, ApiResponse, DataSource, ErrorCode, MutationRequest};
use std::io::Read;

pub fn run(arguments: &[String]) -> Result<(), ApiError> {
    match arguments {
        [command, operation, option, file]
            if command == "session" && operation == "inspect" && option == "--connection-file" =>
        {
            let snapshot = engine_control::inspect_connection_file(file)?;
            crate::print_response(ApiResponse::success(DataSource::Live, snapshot));
            Ok(())
        }
        [
            command,
            operation,
            option,
            file,
            request_option,
            request_file,
        ] if command == "session"
            && operation == "mutate"
            && option == "--connection-file"
            && request_option == "--request-file" =>
        {
            let input = std::fs::File::open(request_file).map_err(|_| {
                ApiError::new(ErrorCode::InvalidRequest, "요청 파일을 읽을 수 없습니다.")
            })?;
            let mut bytes = Vec::new();
            input
                .take((engine_control::MAX_REQUEST_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| {
                    ApiError::new(ErrorCode::InvalidRequest, "요청 파일을 읽을 수 없습니다.")
                })?;
            if bytes.len() > engine_control::MAX_REQUEST_BYTES {
                return Err(ApiError::new(
                    ErrorCode::RequestTooLarge,
                    "요청 파일 크기 제한을 초과했습니다.",
                ));
            }
            let request: MutationRequest = serde_json::from_slice(&bytes).map_err(|_| {
                ApiError::new(
                    ErrorCode::InvalidRequest,
                    "MutationRequest JSON이 올바르지 않습니다.",
                )
            })?;
            let receipt = engine_control::mutate_connection_file(file, request)?;
            crate::print_response(ApiResponse::success(DataSource::Live, receipt));
            Ok(())
        }
        [
            command,
            operation,
            option,
            file,
            session_option,
            session_id,
            request_option,
            request_id,
        ] if command == "session"
            && operation == "result"
            && option == "--connection-file"
            && session_option == "--session-id"
            && request_option == "--request-id" =>
        {
            let receipt = engine_control::lookup_connection_file(file, session_id, request_id)?;
            crate::print_response(ApiResponse::success(DataSource::Live, receipt));
            Ok(())
        }
        _ => Err(ApiError::new(
            ErrorCode::InvalidArguments,
            "사용법: session inspect --connection-file FILE | session mutate --connection-file FILE --request-file JSON | session result --connection-file FILE --session-id ID --request-id ID",
        )),
    }
}
