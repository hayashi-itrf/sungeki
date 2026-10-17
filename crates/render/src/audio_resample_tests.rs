use crate::RenderPlan;
use std::{ffi::OsString, fs, path::Path, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::{ProcessSpec, run_process};
use sungeki_project::{AssetProbe, ClipContent, Project, TimeUs};

fn run(
    binary: &Path,
    dir: &Path,
    graph: &str,
    rate: u32,
    reference: bool,
    selected: u64,
    looping: bool,
) -> Vec<f32> {
    fs::write(dir.join("filter"), graph).unwrap();
    // Actual input has a strong sample outside the selection. Reference has
    // only the selected impulse followed by a long zero continuation.
    let outside = if reference {
        "0".to_string()
    } else {
        format!("if(between(n\\,1\\,{selected})\\,0\\,0.75)")
    };
    let at = if reference && looping {
        format!("mod(n-1\\,{selected})")
    } else {
        "n-1".into()
    };
    let input = format!("aevalsrc=if(eq({at}\\,0)\\,0.25\\,{outside}):s={rate}:d=0.5");
    let args: Vec<OsString> = [
        "-nostdin",
        "-hide_banner",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=s=2x2:d=0.1",
        "-f",
        "lavfi",
        "-i",
        &input,
        "-filter_complex_script",
        "filter",
        "-map",
        "[audio]",
        "-f",
        "f32le",
        "-c:a",
        "pcm_f32le",
        "pipe:1",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    let output = run_process(
        &ProcessSpec {
            program: binary,
            args: &args,
            directory: dir,
            timeout: Duration::from_secs(10),
            max_output_bytes: 1024 * 1024,
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(
        output.succeeded(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
        .stdout
        .chunks_exact(4)
        .map(|s| f32::from_le_bytes(s.try_into().unwrap()))
        .collect()
}

#[test]
#[ignore = "requires VVM_TEST_FFMPEG; real short-region resampling"]
fn short_material_is_not_silently_lost_during_resampling() {
    let binary = std::env::var("VVM_TEST_FFMPEG").expect("explicit FFmpeg path");
    let dir = std::env::temp_dir().join(format!("vvm-resample-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    for rate in [8000u32, 22050, 24000, 32000, 44100, 48000, 96000, 192000] {
        for selected in [1u64, 2, 17, 128, 1001] {
            for looping in [false, true] {
                let mut p: Project = serde_json::from_str(include_str!(
                    "../../../tests/fixtures/audio-project.json"
                ))
                .unwrap();
                let AssetProbe::Audio(probe) = &mut p.assets[0].probe else {
                    panic!()
                };
                probe.sample_rate = rate;
                probe.frames = u64::from(rate);
                probe.byte_length = u64::from(rate) * 2 + 44;
                let start = 1_000_000 / u64::from(rate); // ceil(start*rate/1e6) == 1
                let end = (selected + 1) * 1_000_000 / u64::from(rate);
                p.clips[0].start_us = TimeUs(333);
                let duration = if looping { 12345 } else { end - start };
                p.clips[0].duration_us = TimeUs(duration);
                let ClipContent::Audio { playback, .. } = &mut p.clips[0].content else {
                    panic!()
                };
                playback.source_start_us = TimeUs(start);
                playback.source_duration_us = TimeUs(end - start);
                playback.looped = looping;
                playback.gain_db = 0.;
                playback.fade_in_us = TimeUs(0);
                playback.fade_out_us = TimeUs(0);
                let first = (333 * 48000 + 500000) / 1000000;
                let count = ((333 + duration) * 48000 + 500000) / 1000000 - first;
                if count == 0 {
                    assert!(
                        RenderPlan::new(&p, |_| panic!())
                            .unwrap_err()
                            .to_string()
                            .contains("1サンプル")
                    );
                    continue;
                }
                let plan = RenderPlan::new(&p, |_| panic!()).unwrap();
                let actual = run(
                    Path::new(&binary),
                    &dir,
                    &plan.audio_graph(),
                    rate,
                    false,
                    selected,
                    looping,
                );
                let reference = format!(
                    "[1:a]atrim=start_sample=1,asetpts=PTS-STARTPTS,aresample=48000,aformat=channel_layouts=stereo,atrim=end_sample={count},adelay={first}S:all=1,apad=whole_len={},atrim=end_sample={}[audio]",
                    plan.audio_samples(),
                    plan.audio_samples()
                );
                let expected = run(
                    Path::new(&binary),
                    &dir,
                    &reference,
                    rate,
                    true,
                    selected,
                    looping,
                );
                assert_eq!(actual.len(), expected.len());
                assert!(expected.iter().any(|s| s.abs() > 0.01));
                for (index, (a, b)) in actual.iter().zip(&expected).enumerate() {
                    assert!(
                        (a - b).abs() < 1e-6,
                        "rate={rate} selected={selected} loop={looping} sample={index}: {a} vs {b}"
                    );
                }
            }
        }
    }
    fs::remove_dir_all(dir).unwrap();
}
