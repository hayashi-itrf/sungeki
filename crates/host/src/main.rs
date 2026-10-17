mod audio_analysis;
mod audio_decode;
mod audio_preview_api;
mod audio_preview_read;
mod audio_preview_record;
mod audio_preview_worker;
mod audio_probe;
mod build_record;
mod build_state;
mod checkpoint_compare;
mod checkpoints;
mod collection_execute;
mod data;
mod edit_preview;
mod endpoint;
mod events;
mod exit_cleanup;
mod exit_control;
mod frame_preview_api;
mod frame_preview_publish;
mod frame_preview_read;
mod frame_preview_record;
mod frame_preview_worker;
mod gain_proposal;
mod image_probe;
mod irodori_originals;
mod loudness_api;
mod loudness_record;
mod loudness_worker;
mod media_job_runtime;
mod original_file;
mod project_read;
mod pronunciation;
mod psd_preview;
mod psd_probe;
mod qwen_originals;
mod render_api;
mod render_location;
mod render_publish;
mod render_record;
mod render_runtime;
mod render_worker;
mod review_freshness;
mod review_jobs;
mod review_record;
mod review_workdir;
mod review_worker;
mod runtime_root;
mod runtime_settings;
mod saved_audio;
mod sbv2_api;
mod sbv2_exchange;
mod sbv2_job;
mod sbv2_originals;
mod sbv2_process;
mod sbv2_startup;
mod server;
mod session_errors;
mod session_execute;
mod session_open;
mod session_save;
mod sessions;
mod speech_api;
mod speech_apply;
mod speech_artifacts;
mod speech_dispatch;
mod speech_exchange;
mod speech_failure;
mod speech_journal;
mod speech_original;
mod speech_output;
mod speech_preparation;
mod speech_runtime;
mod speech_state;
mod speech_transition;
mod speech_worker;
mod startup;
mod subtitle_export;
mod voice_catalog;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    startup::run().await
}
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod review_tests;
#[cfg(test)]
mod save_tests;
#[cfg(test)]
mod saved_audio_tests;
#[cfg(test)]
mod speech_apply_tests;
#[cfg(test)]
mod speech_dispatch_tests;
#[cfg(test)]
mod speech_exchange_tests;
#[cfg(test)]
mod speech_journal_tests;
#[cfg(test)]
mod speech_original_tests;
#[cfg(test)]
mod speech_output_tests;
