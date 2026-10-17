//! A bounded, best-effort startup diagnostic channel, separate from host IPC.
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use tokio::io::{AsyncRead, AsyncReadExt};

pub(crate) fn observe(mut reader: impl AsyncRead + Unpin + Send + 'static) -> Arc<AtomicU8> {
    let progress = Arc::new(AtomicU8::new(0));
    let state = progress.clone();
    tokio::spawn(async move {
        let mut buffer = [0u8; 256];
        let mut line = Vec::with_capacity(64);
        let mut overflow = false;
        while let Ok(size) = reader.read(&mut buffer).await {
            if size == 0 {
                break;
            }
            for &byte in &buffer[..size] {
                if byte == b'\n' {
                    if !overflow {
                        accept(&line, &state);
                    }
                    line.clear();
                    overflow = false;
                } else if line.len() < 64 {
                    line.push(byte);
                } else {
                    overflow = true;
                }
            }
        }
    });
    progress
}

fn accept(line: &[u8], progress: &AtomicU8) {
    let phase = match line {
        b"VVM_STARTUP:runtime" => 1,
        b"VVM_STARTUP:data_directory" => 2,
        b"VVM_STARTUP:recovery" => 3,
        b"VVM_STARTUP:listen" => 4,
        _ => return,
    };
    progress.store(phase, Ordering::SeqCst);
}

pub(crate) fn label(progress: &AtomicU8) -> Option<&'static str> {
    match progress.load(Ordering::SeqCst) {
        1 => Some("接続先の確保"),
        2 => Some("保存領域の確認"),
        3 => Some("保存済みの編集状態を復元"),
        4 => Some("接続受付の準備"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    #[tokio::test]
    async fn chunked_markers_are_read_but_unknown_or_oversized_lines_are_ignored() {
        let (mut writer, reader) = tokio::io::duplex(4096);
        let state = observe(reader);
        writer.write_all(b"VVM_STARTUP:rec").await.unwrap();
        writer.write_all(b"overy\n").await.unwrap();
        tokio::task::yield_now().await;
        assert_eq!(label(&state), Some("保存済みの編集状態を復元"));
        writer.write_all(b"private arbitrary text\n").await.unwrap();
        writer.write_all(&[b'x'; 2048]).await.unwrap();
        writer.write_all(b"VVM_STARTUP:listen\n").await.unwrap();
        tokio::task::yield_now().await;
        assert_eq!(label(&state), Some("保存済みの編集状態を復元"));
        writer.write_all(b"VVM_STARTUP:listen\n").await.unwrap();
        tokio::task::yield_now().await;
        assert_eq!(label(&state), Some("接続受付の準備"));
    }
}
