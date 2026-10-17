//! Media validation shared by speech, rendering and review. No Project edits.
pub mod proxy_timeline;
#[cfg(unix)]
pub mod video_probe;
mod wav;
mod wav_store;
pub use wav_store::{WavStoreError, store_wav_new};
mod wav_container;
pub use wav::{ExpectedAudio, VerifiedWav, WavError, WavInfo, inspect_wav, verify_wav};
pub const MAX_WAV_BYTES: usize = 64 * 1024 * 1024;
#[cfg(unix)]
mod process;
#[cfg(unix)]
mod process_capture;
#[cfg(unix)]
pub use process::{ProcessOutput, ProcessSpec, StopReason, run_process, run_process_to_writer};
#[cfg(target_os = "macos")]
mod process_group;
#[cfg(target_os = "macos")]
pub use process_group::only_exited as owned_group_only_exited;
mod lipsync;
pub use lipsync::{
    LIPSYNC_ANALYSIS_VERSION, LipSyncAnalysis, LipSyncError, LipSyncSettings, MouthInterval,
    analyze_lipsync,
};

mod waveform;
mod waveform_file;
pub use waveform::{
    SamplePeak, WAVEFORM_VERSION, Waveform, WaveformError, WaveformLevel, WaveformWindow,
    analyze_waveform,
};
pub use waveform_file::analyze_decoded_mp3_waveform;

#[cfg(unix)]
mod audio_original;
#[cfg(unix)]
mod waveform_cache;
#[cfg(unix)]
pub use waveform_cache::{CachedWaveform, WaveformCache, WaveformCacheError};

#[cfg(unix)]
mod loudness;
#[cfg(unix)]
mod loudness_file;
#[cfg(unix)]
mod mix_limits;
#[cfg(unix)]
pub use mix_limits::MixFrameLimit;
#[cfg(unix)]
mod disk_space;
#[cfg(unix)]
pub use disk_space::ensure_available_space;
#[cfg(unix)]
mod loudness_report;
#[cfg(unix)]
mod mix_container;
#[cfg(unix)]
pub use loudness::{
    LOUDNESS_VERSION, LoudnessAnalysis, LoudnessError, LoudnessUnavailable, LoudnessValue,
    measure_loudness,
};
#[cfg(unix)]
pub use loudness_file::{
    measure_decoded_mp3_loudness, measure_mix_loudness, measure_render_mix_loudness,
};

#[cfg(unix)]
pub use audio_original::{
    capture as read_audio_original, capture_with_cancel as read_audio_original_with_cancel,
    source_identity,
};

#[cfg(unix)]
mod gain_proposal;
#[cfg(unix)]
pub use gain_proposal::{
    GAIN_PEAK_MARGIN_DB, GainProposal, GainProposalError, propose_speech_gain,
};

#[cfg(unix)]
pub use audio_original::inspect as probe_audio;

#[cfg(unix)]
mod mp3_decode;
#[cfg(unix)]
pub use mp3_decode::{Mp3DecodeError, decode_mp3};

#[cfg(unix)]
mod compressed_original;
#[cfg(unix)]
pub use compressed_original::{
    AudioDecodeError, AudioDecodeRuntime, decode_audio_original, decode_mp3_original_file,
    probe_mp3,
};

#[cfg(unix)]
mod mp3_file;
#[cfg(unix)]
pub use mp3_file::{DecodedMp3File, decode_mp3_file};

#[cfg(unix)]
pub mod vrm_probe;
