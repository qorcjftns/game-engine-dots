use std::{
    fs,
    io::Write,
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use engine_protocol::{ApiResponse, DataSource, ErrorCode, LiveInspection, PROTOCOL_VERSION};

use crate::{
    MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Result, SOCKET_TIMEOUT, SessionTracker,
    SnapshotPublisher,
    connection::{ConnectionFile, InspectRequest, read_frame, write_frame},
    failure, random_id,
};

pub struct QueryServer {
    connection_file: PathBuf,
    token: String,
    publisher: SnapshotPublisher,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl QueryServer {
    /// 외부 사용자 전용 경로에 새 연결 파일을 만듭니다. 기존 파일은 덮어쓰지 않습니다.
    pub fn start(connection_file: &Path) -> Result<Self> {
        Self::start_with_snapshot(
            connection_file,
            SessionTracker::new()?.observe(0, None, false)?,
        )
    }

    /// 첫 연결부터 GUI가 제공한 상태를 반환하여 임시 빈 문서가 노출되지 않게 합니다.
    pub fn start_with_snapshot(connection_file: &Path, initial: LiveInspection) -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| failure(ErrorCode::ControlIo, "로컬 제어 통로를 열지 못했습니다."))?;
        listener.set_nonblocking(true).map_err(|_| {
            failure(
                ErrorCode::ControlIo,
                "로컬 제어 통로를 설정하지 못했습니다.",
            )
        })?;
        let address = listener
            .local_addr()
            .map_err(|_| failure(ErrorCode::ControlIo, "로컬 주소를 조회하지 못했습니다."))?;
        let token = random_id()?;
        let path = std::path::absolute(connection_file).map_err(|_| file_error())?;
        let parent = path.parent().ok_or_else(file_error)?;
        fs::create_dir_all(parent).map_err(|_| file_error())?;
        let parent = parent.canonicalize().map_err(|_| file_error())?;
        let path = parent.join(path.file_name().ok_or_else(file_error)?);
        let descriptor = ConnectionFile {
            protocol_version: PROTOCOL_VERSION,
            address: address.to_string(),
            token: token.clone(),
        };
        let bytes = serde_json::to_vec(&descriptor).map_err(|_| file_error())?;
        let mut temporary = tempfile::NamedTempFile::new_in(&parent).map_err(|_| file_error())?;
        temporary.write_all(&bytes).map_err(|_| file_error())?;
        temporary.as_file().sync_all().map_err(|_| file_error())?;
        let publisher = SnapshotPublisher::new(initial);
        temporary
            .persist_noclobber(&path)
            .map_err(|_| file_error())?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker_publisher = publisher.clone();
        let worker_token = token.clone();
        let worker =
            thread::spawn(move || run(listener, worker_publisher, worker_token, worker_stop));
        Ok(Self {
            connection_file: path,
            token,
            publisher,
            stop,
            worker: Some(worker),
        })
    }

    pub fn publisher(&self) -> SnapshotPublisher {
        self.publisher.clone()
    }

    pub fn connection_file(&self) -> &Path {
        &self.connection_file
    }
}

impl Drop for QueryServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        // 종료 중 사용자가 파일을 바꿨다면 지우지 않습니다.
        if ConnectionFile::read(&self.connection_file)
            .is_ok_and(|connection| connection.token == self.token)
        {
            let _ = fs::remove_file(&self.connection_file);
        }
    }
}

fn run(listener: TcpListener, publisher: SnapshotPublisher, token: String, stop: Arc<AtomicBool>) {
    let (sender, receiver) = mpsc::sync_channel::<TcpStream>(8);
    let receiver = Arc::new(Mutex::new(receiver));
    let mut workers = Vec::new();
    for _ in 0..4 {
        let receiver = receiver.clone();
        let publisher = publisher.clone();
        let token = token.clone();
        let stop = stop.clone();
        workers.push(thread::spawn(move || {
            loop {
                let stream = match receiver.lock() {
                    Ok(receiver) => receiver.recv(),
                    Err(_) => return,
                };
                let Ok(stream) = stream else { return };
                if stop.load(Ordering::Acquire) {
                    return;
                }
                let _ = stream.set_read_timeout(Some(SOCKET_TIMEOUT));
                let _ = stream.set_write_timeout(Some(SOCKET_TIMEOUT));
                let response = answer(&stream, &token, &publisher, &stop);
                if let Err(error) = write_frame(&stream, &response, MAX_RESPONSE_BYTES, Some(&stop))
                    && error.code == ErrorCode::ResponseTooLarge
                {
                    let response = ApiResponse::<()>::failure(DataSource::Live, error);
                    let _ = write_frame(&stream, &response, MAX_RESPONSE_BYTES, Some(&stop));
                }
            }
        }));
    }
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Err(mpsc::TrySendError::Full(stream)) = sender.try_send(stream) {
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                    let response = ApiResponse::<()>::failure(
                        DataSource::Live,
                        failure(
                            ErrorCode::ServerBusy,
                            "제어 요청이 많습니다. 잠시 후 다시 조회하세요.",
                        ),
                    );
                    let _ = write_frame(&stream, &response, MAX_RESPONSE_BYTES, Some(&stop));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5))
            }
            Err(_) => break,
        }
    }
    drop(sender);
    for worker in workers {
        let _ = worker.join();
    }
}

fn answer(
    stream: &TcpStream,
    token: &str,
    publisher: &SnapshotPublisher,
    stop: &AtomicBool,
) -> ApiResponse<Arc<LiveInspection>> {
    let result = (|| {
        let bytes = read_frame(stream, MAX_REQUEST_BYTES, Some(stop))?;
        let request: InspectRequest = serde_json::from_slice(&bytes).map_err(|_| {
            failure(
                ErrorCode::InvalidRequest,
                "올바른 조회 요청 JSON이 아닙니다.",
            )
        })?;
        if request.token != token {
            return Err(failure(
                ErrorCode::Unauthorized,
                "세션 인증에 실패했습니다.",
            ));
        }
        if request.protocol_version != PROTOCOL_VERSION {
            return Err(failure(
                ErrorCode::UnsupportedProtocol,
                "지원하지 않는 요청 버전입니다.",
            ));
        }
        if request.operation != "inspect" {
            return Err(failure(
                ErrorCode::InvalidRequest,
                "이 단계에서는 inspect 조회만 지원합니다.",
            ));
        }
        publisher.snapshot()
    })();
    match result {
        Ok(snapshot) => ApiResponse::success(DataSource::Live, snapshot),
        Err(error) => ApiResponse::failure(DataSource::Live, error),
    }
}

fn file_error() -> crate::ControlError {
    failure(
        ErrorCode::InvalidConnectionFile,
        "연결 파일을 새로 만들지 못했습니다. 기존 파일은 덮어쓰지 않습니다.",
    )
}
