use super::{Stage, TransportError};
use reqwest::{Client, Url};
use std::{
    net::IpAddr,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub(super) fn connection(base: &str, timeout: Duration) -> Result<(Client, Url), TransportError> {
    let base = endpoint(base)?;
    if timeout.is_zero() || timeout > Duration::from_secs(300) {
        return Err(config(
            "各HTTP処理の待機時間は0秒超、300秒以内で指定してください",
        ));
    }
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(timeout.min(Duration::from_secs(5)))
        .timeout(timeout)
        .build()
        .map_err(|_| config("VOICEVOXのHTTPクライアントを作成できません"))?;
    Ok((client, base))
}

pub(super) fn config(message: &str) -> TransportError {
    TransportError::Configuration(message.into())
}
pub(super) fn check_cancel(cancelled: &AtomicBool) -> Result<(), TransportError> {
    if cancelled.load(Ordering::Acquire) {
        Err(TransportError::Cancelled)
    } else {
        Ok(())
    }
}
pub(super) fn failure(stage: Stage, error: reqwest::Error) -> TransportError {
    // Do not include the URL (which contains the full speech text) in errors.
    if error.is_timeout() {
        TransportError::Timeout(stage)
    } else {
        TransportError::Connection(stage)
    }
}
pub(super) async fn read(
    mut response: reqwest::Response,
    stage: Stage,
    limit: usize,
) -> Result<Vec<u8>, TransportError> {
    if response.status() != reqwest::StatusCode::OK {
        return Err(TransportError::Http {
            stage,
            status: response.status().as_u16(),
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(TransportError::TooLarge { stage, limit });
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| failure(stage, e))? {
        if chunk.len() > limit - bytes.len() {
            return Err(TransportError::TooLarge { stage, limit });
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(super) fn endpoint(base: &str) -> Result<Url, TransportError> {
    let base = Url::parse(base).map_err(|_| config("VOICEVOXのURLが不正です"))?;
    let local = base
        .host_str()
        .and_then(|host| host.trim_matches(['[', ']']).parse::<IpAddr>().ok())
        .is_some_and(|ip| ip.is_loopback());
    if base.scheme() != "http"
        || !local
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
        || base.path() != "/"
    {
        return Err(config(
            "VOICEVOXはループバックIPのHTTPルートURLを指定してください",
        ));
    }
    Ok(base)
}
