#[path = "response_timeout.rs"]
mod response_timeout;
use crate::HostOptions;
use std::{io::ErrorKind, process::Stdio, time::Duration};
use sungeki_protocol::{
    framing::{read_frame, write_frame},
    *,
};
use tokio::{
    net::UnixStream,
    process::Command,
    time::{Instant, sleep, timeout},
};

pub struct HostClient {
    stream: UnixStream,
    request_id: u32,
    protocol_minor: u16,
}

impl HostClient {
    pub async fn connect_or_start(options: &HostOptions, name: &str) -> Result<Self, ApiError> {
        options.validate()?;
        let stream = match connect(options).await? {
            Some(stream) => stream,
            None => start_and_connect(options).await?,
        };
        Self::handshake(stream, name).await
    }

    async fn handshake(stream: UnixStream, name: &str) -> Result<Self, ApiError> {
        let mut client = Self {
            stream,
            request_id: 0,
            protocol_minor: 0,
        };
        match client
            .request(RequestBody::Hello {
                hello: ClientHello::new(name),
            })
            .await?
        {
            ResponseBody::Hello { host }
                if host.application == APPLICATION && host.protocol_major == PROTOCOL_MAJOR =>
            {
                client.protocol_minor = host.protocol_minor;
                Ok(client)
            }
            _ => Err(ApiError::new(
                ErrorCode::ProtocolMismatch,
                "ホストの識別情報/プロトコルが一致しません",
            )),
        }
    }

    /// Exit inspection must never launch a new host.
    pub async fn connect_existing(
        options: &HostOptions,
        name: &str,
    ) -> Result<Option<Self>, ApiError> {
        match connect(options).await? {
            Some(stream) => Self::handshake(stream, name).await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn lifecycle(&mut self, action: ExitAction) -> Result<ExitStatus, ApiError> {
        if self.protocol_minor < 147 {
            return Err(ApiError::new(
                ErrorCode::ProtocolMismatch,
                "終了管理には更新版ホストが必要です。以前のアプリで実行中の処理を完了して終了してください",
            ));
        }
        match self.request(RequestBody::Lifecycle { action }).await? {
            ResponseBody::Lifecycle { status } => Ok(status),
            _ => Err(ApiError::new(
                ErrorCode::ProtocolMismatch,
                "終了情報の応答形式が不正です",
            )),
        }
    }

    pub async fn capabilities(&mut self) -> Result<HostInfo, ApiError> {
        match self.request(RequestBody::GetCapabilities {}).await? {
            ResponseBody::Capabilities { host } => Ok(host),
            _ => Err(ApiError::new(
                ErrorCode::ProtocolMismatch,
                "機能情報の応答形式が不正です",
            )),
        }
    }

    pub async fn session(&mut self, request: SessionRequest) -> Result<SessionResponse, ApiError> {
        if self.protocol_minor < request.minimum_protocol_minor() {
            return Err(ApiError::new(
                ErrorCode::ProtocolMismatch,
                "この操作には更新版ホストが必要です。実行中の作業を終えてSungeki MakerアプリとMCPを更新してください",
            ));
        }
        match self.request(RequestBody::Session { request }).await? {
            ResponseBody::Session { result } => Ok(result),
            _ => Err(ApiError::new(
                ErrorCode::ProtocolMismatch,
                "セッション応答形式が不正です",
            )),
        }
    }

    pub async fn watch(
        &mut self,
        after: Option<EventCursor>,
        wait_ms: u32,
    ) -> Result<EventBatch, ApiError> {
        if self.protocol_minor < 2 {
            return Err(ApiError::new(
                ErrorCode::ProtocolMismatch,
                "変更通知には更新版ホストが必要です",
            ));
        }
        if wait_ms > 20_000 {
            return Err(ApiError::new(
                ErrorCode::ValidationFailed,
                "通知待機は20秒以内です",
            ));
        }
        match self.request(RequestBody::Watch { after, wait_ms }).await? {
            ResponseBody::Events { batch } => Ok(batch),
            _ => Err(ApiError::new(
                ErrorCode::ProtocolMismatch,
                "通知応答の形式が不正です",
            )),
        }
    }

    pub(crate) async fn request(&mut self, body: RequestBody) -> Result<ResponseBody, ApiError> {
        self.request_id = self
            .request_id
            .checked_add(1)
            .ok_or_else(|| ApiError::new(ErrorCode::HostUnavailable, "再接続が必要です"))?;
        let wait = response_timeout::for_request(&body);
        let result = timeout(wait, async {
            write_frame(
                &mut self.stream,
                &Request {
                    request_id: self.request_id,
                    body,
                },
            )
            .await?;
            read_frame::<Response>(&mut self.stream).await
        })
        .await
        .map_err(|_| {
            ApiError::new(
                ErrorCode::HostUnavailable,
                "ホスト応答がタイムアウトしました",
            )
        })??;
        if result.request_id != self.request_id {
            return Err(ApiError::new(
                ErrorCode::ProtocolMismatch,
                "応答IDが一致しません",
            ));
        }
        match result.body {
            ResponseBody::Error { error } => Err(error),
            body => Ok(body),
        }
    }
}

async fn connect(options: &HostOptions) -> Result<Option<UnixStream>, ApiError> {
    options.validate()?;
    match timeout(
        Duration::from_secs(1),
        UnixStream::connect(options.socket()),
    )
    .await
    {
        Ok(Ok(stream)) => Ok(Some(stream)),
        Ok(Err(error))
            if matches!(
                error.kind(),
                ErrorKind::NotFound | ErrorKind::ConnectionRefused
            ) =>
        {
            Ok(None)
        }
        Ok(Err(error)) => Err(ApiError::io(error)),
        Err(_) => Err(ApiError::new(
            ErrorCode::HostUnavailable,
            "ホスト接続がタイムアウトしました",
        )),
    }
}

async fn start_and_connect(options: &HostOptions) -> Result<UnixStream, ApiError> {
    if !options.host_binary.is_file() {
        let mut error = ApiError::new(
            ErrorCode::SidecarMissing,
            "Sungeki Makerのhost実行ファイルがありません",
        );
        error
            .details
            .insert("path".into(), options.host_binary.display().to_string());
        error
            .suggested_actions
            .push("ホストをビルドするか、Sungeki Makerアプリを再インストールしてください".into());
        return Err(error);
    }
    let mut child = Command::new(&options.host_binary)
        .arg("--runtime-dir")
        .arg(&options.runtime_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(ApiError::io)?;
    let progress = crate::startup_progress::observe(child.stdout.take().unwrap());
    // Reap only the process we started; a shared host survives a client disconnect.
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(stream) = connect(options).await? {
            return Ok(stream);
        }
        sleep(Duration::from_millis(40)).await;
    }
    let mut error = ApiError::new(
        ErrorCode::HostUnavailable,
        "ホストの準備が時間内に完了しませんでした。OSのアクセス許可と保存先の接続を確認してから、もう一度お試しください",
    );
    if let Some(stage) = crate::startup_progress::label(&progress) {
        error.message = format!(
            "ホストの起動を確認できませんでした。最後に確認した工程：{stage}。少し待って再試行してください"
        );
        error.details.insert("startup_stage".into(), stage.into());
    }
    error.retryable = true;
    error
        .suggested_actions
        .push("runtimeのパスと権限、hostの標準エラーを確認してください".into());
    Err(error)
}
