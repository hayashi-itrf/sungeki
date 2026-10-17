use tauri::{Emitter, WebviewWindow};

#[tauri::command]
pub fn check_shell(window: WebviewWindow) -> Result<String, String> {
    window
        .emit("next-shell-ready", "ネイティブイベント受信成功")
        .map_err(|error| error.to_string())?;
    Ok("Tauriの起動確認成功。共通ホストは別途接続してください。".into())
}
