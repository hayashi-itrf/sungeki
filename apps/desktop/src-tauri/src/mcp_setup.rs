#[tauri::command]
pub fn mcp_executable_path() -> Result<String, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = executable.parent().ok_or("アプリの場所を取得できません")?;
    let mcp = directory.join("sungeki-mcp");
    if !mcp.is_file() || !directory.join("sungeki-host").is_file() {
        return Err("同梱MCPが見つかりません。MCPを含む版をビルドしてください。".into());
    }
    mcp.to_str()
        .map(str::to_owned)
        .ok_or_else(|| "アプリのパスを文字列として取得できません".into())
}
