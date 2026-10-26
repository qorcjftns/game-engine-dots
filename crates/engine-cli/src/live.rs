use engine_protocol::{ApiError, ApiResponse, DataSource, ErrorCode};

pub fn run(arguments: &[String]) -> Result<(), ApiError> {
    match arguments {
        [command, operation, option, file]
            if command == "session" && operation == "inspect" && option == "--connection-file" =>
        {
            let snapshot = engine_control::inspect_connection_file(file)?;
            crate::print_response(ApiResponse::success(DataSource::Live, snapshot));
            Ok(())
        }
        _ => Err(ApiError::new(
            ErrorCode::InvalidArguments,
            "사용법: session inspect --connection-file FILE",
        )),
    }
}
