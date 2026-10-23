mod audio_preview;
mod exit;
#[cfg(target_os = "macos")]
mod exit_macos;
mod host;
mod mcp_setup;
mod startup;
mod startup_trace;

fn main() {
    startup_trace::configure(tauri::Builder::default())
        .register_asynchronous_uri_scheme_protocol(
            "audiopreview",
            |_context, request, responder| {
                tauri::async_runtime::spawn(async move {
                    responder.respond(audio_preview::respond(request, false).await);
                });
            },
        )
        .register_asynchronous_uri_scheme_protocol(
            "videopreview",
            |_context, request, responder| {
                tauri::async_runtime::spawn(async move {
                    responder.respond(audio_preview::respond(request, true).await);
                });
            },
        )
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            exit_macos::install(app.handle()).map_err(std::io::Error::other)?;
            exit::start_lease();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            startup::check_shell,
            mcp_setup::mcp_executable_path,
            startup_trace::trace_startup,
            exit::exit_status,
            exit::finish_exit,
            exit::exit_ready,
            host::get_host_capabilities,
            host::session_request,
            host::watch_sessions
        ])
        .build(tauri::generate_context!())
        .expect("Sungeki Makerのネイティブアプリを起動できませんでした")
        .run(exit::on_event);
}
