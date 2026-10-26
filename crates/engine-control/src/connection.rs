use std::{
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use engine_protocol::{ApiResponse, DataSource, ErrorCode, LiveInspection, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};

use crate::{MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Result, SOCKET_TIMEOUT, failure};

// Debug를 구현하지 않습니다. token이 로그에 들어가는 것을 피합니다.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConnectionFile {
    pub protocol_version: u32,
    pub address: String,
    pub token: String,
}

impl ConnectionFile {
    pub(crate) fn read(path: &Path) -> Result<Self> {
        let file = fs::File::open(path).map_err(|_| invalid_connection())?;
        let mut bytes = Vec::new();
        file.take((MAX_REQUEST_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid_connection())?;
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(invalid_connection());
        }
        let connection: Self = serde_json::from_slice(&bytes).map_err(|_| invalid_connection())?;
        connection.address()?;
        if connection.protocol_version != PROTOCOL_VERSION
            || connection.token.len() != 64
            || !connection
                .token
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid_connection());
        }
        Ok(connection)
    }

    pub(crate) fn address(&self) -> Result<SocketAddr> {
        let address: SocketAddr = self.address.parse().map_err(|_| invalid_connection())?;
        if address.ip() != Ipv4Addr::LOCALHOST || address.port() == 0 {
            return Err(invalid_connection());
        }
        Ok(address)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InspectRequest {
    pub protocol_version: u32,
    pub token: String,
    pub operation: String,
}

// 작은 read timeout을 반복하되 전체 frame 기한은 연장하지 않습니다.
pub(crate) fn read_frame(
    mut stream: &TcpStream,
    maximum: usize,
    stop: Option<&AtomicBool>,
) -> Result<Vec<u8>> {
    let deadline = Instant::now() + SOCKET_TIMEOUT;
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(|_| io_error())?;
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        if Instant::now() >= deadline || stop.is_some_and(|stop| stop.load(Ordering::Acquire)) {
            return Err(io_error());
        }
        let remaining = (maximum + 1 - bytes.len()).min(chunk.len());
        match stream.read(&mut chunk[..remaining]) {
            Ok(0) => {
                return Err(failure(
                    ErrorCode::InvalidRequest,
                    "완료되지 않은 JSON 메시지입니다.",
                ));
            }
            Ok(count) => {
                let received = &chunk[..count];
                let end = received.iter().position(|byte| *byte == b'\n');
                bytes.extend_from_slice(&received[..end.map_or(count, |index| index + 1)]);
                if bytes.len() > maximum {
                    return Err(failure(
                        ErrorCode::RequestTooLarge,
                        "메시지 크기 제한을 초과했습니다.",
                    ));
                }
                if end.is_some() {
                    return Ok(bytes);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(io_error()),
        }
    }
}

pub(crate) fn write_frame(
    mut stream: &TcpStream,
    value: &impl Serialize,
    maximum: usize,
    stop: Option<&AtomicBool>,
) -> Result<()> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|_| failure(ErrorCode::InvalidRequest, "JSON 응답을 만들지 못했습니다."))?;
    bytes.push(b'\n');
    if bytes.len() > maximum {
        return Err(failure(
            ErrorCode::ResponseTooLarge,
            "응답 크기 제한을 초과했습니다.",
        ));
    }
    stream
        .set_write_timeout(Some(Duration::from_millis(50)))
        .map_err(|_| io_error())?;
    let deadline = Instant::now() + SOCKET_TIMEOUT;
    let mut written = 0;
    while written < bytes.len() {
        if Instant::now() >= deadline || stop.is_some_and(|stop| stop.load(Ordering::Acquire)) {
            return Err(io_error());
        }
        match stream.write(&bytes[written..]) {
            Ok(0) => return Err(io_error()),
            Ok(count) => written += count,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(io_error()),
        }
    }
    Ok(())
}

fn io_error() -> crate::ControlError {
    failure(
        ErrorCode::ControlIo,
        "세션 메시지 전송/수신이 중단되었거나 기한을 초과했습니다.",
    )
}

/// 연결 파일은 읽기만 하며 응답도 마지막 게시 snapshot만 받습니다.
pub fn inspect_connection_file(path: impl AsRef<Path>) -> Result<LiveInspection> {
    let connection = ConnectionFile::read(path.as_ref())?;
    let stream =
        TcpStream::connect_timeout(&connection.address()?, SOCKET_TIMEOUT).map_err(|_| {
            failure(
                ErrorCode::ControlIo,
                "실행 중인 에디터 세션에 연결하지 못했습니다.",
            )
        })?;
    stream
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| invalid_connection())?;
    stream
        .set_write_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| invalid_connection())?;
    write_frame(
        &stream,
        &InspectRequest {
            protocol_version: PROTOCOL_VERSION,
            token: connection.token,
            operation: "inspect".into(),
        },
        MAX_REQUEST_BYTES,
        None,
    )?;
    let bytes = read_frame(&stream, MAX_RESPONSE_BYTES, None)?;
    let response: ApiResponse<LiveInspection> = serde_json::from_slice(&bytes).map_err(|_| {
        failure(
            ErrorCode::InvalidRequest,
            "세션 응답 JSON이 올바르지 않습니다.",
        )
    })?;
    match response {
        ApiResponse::Ok {
            protocol_version,
            source: DataSource::Live,
            data,
        } if protocol_version == PROTOCOL_VERSION => Ok(data),
        ApiResponse::Error {
            protocol_version,
            source: DataSource::Live,
            error,
        } if protocol_version == PROTOCOL_VERSION => Err(error),
        _ => Err(failure(
            ErrorCode::UnsupportedProtocol,
            "지원하지 않는 세션 응답입니다.",
        )),
    }
}

fn invalid_connection() -> crate::ControlError {
    failure(
        ErrorCode::InvalidConnectionFile,
        "연결 파일을 읽을 수 없거나 지원하지 않는 로컬 연결입니다.",
    )
}
