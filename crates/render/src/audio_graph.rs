use crate::{RenderError, RenderPlan, plan::error};
use sungeki_project::{AssetProbe, AudioPlayback, AudioProbe};
use sungeki_scene::AudioSource;

fn sample(us: u64) -> u64 {
    (us * 48000 + 500000) / 1000000
}
fn selection(p: &AudioPlayback, probe: &AudioProbe) -> (u64, u64) {
    crate::audio_selection::range(
        p.source_start_us,
        p.source_duration_us,
        probe.sample_rate,
        probe.frames,
    )
}
fn probe<'a>(plan: &'a RenderPlan, id: &str) -> &'a AudioProbe {
    let AssetProbe::Audio(p) = &plan.sources[id].probe else {
        unreachable!("validated audio")
    };
    p
}
pub(crate) fn validate_loop_memory(plan: &RenderPlan) -> Result<(), RenderError> {
    let mut bytes = 0u64;
    for cue in &plan.scene.audio {
        if let AudioSource::Video { asset_id, playback } = &cue.source {
            let AssetProbe::Video(probe) = &plan.sources[asset_id].probe else {
                unreachable!()
            };
            let selected = crate::VideoAudioSelection::from_probe(
                playback,
                probe,
                cue.start_us,
                sungeki_project::TimeUs(cue.end_us.0 - cue.start_us.0),
            )?;
            bytes = bytes
                .checked_add(selected.loop_bytes())
                .ok_or_else(|| error("音声ループのサイズが不正です"))?;
        }
        if let AudioSource::Material { asset_id, playback } = &cue.source {
            if sample(cue.start_us.0) == sample(cue.end_us.0) {
                return Err(error(
                    "音声クリップが出力の1サンプルより短いため出力できません",
                ));
            }
            if playback.looped {
                let p = probe(plan, asset_id);
                let (first, end) = selection(playback, p);
                bytes = bytes
                    .checked_add((end - first) * u64::from(p.channels) * 8)
                    .ok_or_else(|| error("音声ループのサイズが不正です"))?;
            }
        }
    }
    if bytes > 256 * 1024 * 1024 {
        return Err(error("音声ループの展開サイズ合計が256MiBを超えています"));
    }
    Ok(())
}
impl RenderPlan {
    #[cfg(test)]
    pub(crate) fn audio_graph(&self) -> String {
        self.audio_graph_with_offset(1)
    }
    pub(crate) fn audio_graph_with_offset(&self, input_offset: usize) -> String {
        let mut graph = format!(
            "anullsrc=r=48000:cl=stereo,atrim=end_sample={}[base];",
            self.audio_samples()
        );
        for (i, cue) in self.scene.audio.iter().enumerate() {
            graph.push_str(&format!("[{}:a]", i + input_offset));
            if let AudioSource::Video { asset_id, playback } = &cue.source {
                let AssetProbe::Video(probe) = &self.sources[asset_id].probe else {
                    unreachable!()
                };
                let selected = crate::VideoAudioSelection::from_probe(
                    playback,
                    probe,
                    cue.start_us,
                    sungeki_project::TimeUs(cue.end_us.0 - cue.start_us.0),
                )
                .expect("validated video audio");
                graph.push_str(&format!("{}[a{i}];", selected.filter()));
                continue;
            }
            if let AudioSource::Material { asset_id, playback } = &cue.source {
                let (first, end) = selection(playback, probe(self, asset_id));
                graph.push_str(&crate::audio_selection::trim_loop(
                    first,
                    end,
                    playback.looped,
                ));
                if !playback.looped && probe(self, asset_id).sample_rate != 48000 {
                    // Zero continuation lets the resampler flush even a one-sample
                    // selection. Trim at the output boundary below, never expose
                    // samples outside the selected source or extend the clip.
                    let padding =
                        u64::from(probe(self, asset_id).sample_rate).div_ceil(48000) * 64 + 64;
                    graph.push_str(&format!("apad=pad_len={padding},"));
                }
            }
            if let Some(db) = cue.gain_db {
                graph.push_str(&format!(
                    "volume={:.15}:precision=double,",
                    10f64.powf(db / 20.0)
                ));
            }
            graph.push_str("aresample=48000,aformat=channel_layouts=stereo,asetpts=PTS-STARTPTS,");
            if let AudioSource::Material { playback, .. } = &cue.source {
                let count = sample(cue.end_us.0) - sample(cue.start_us.0);
                graph.push_str(&format!("atrim=end_sample={count},"));
                let fade_in = sample(playback.fade_in_us.0).min(count);
                let fade_out = sample(playback.fade_out_us.0).min(count - fade_in);
                if fade_in > 0 {
                    graph.push_str(&format!("afade=t=in:ss=0:ns={fade_in}:curve=tri,"));
                }
                if fade_out > 0 {
                    graph.push_str(&format!(
                        "afade=t=out:ss={}:ns={fade_out}:curve=tri,",
                        count - fade_out
                    ));
                }
                graph.push_str(&crate::volume_envelope::filter(&playback.volume_points));
                if let Some(settings) = &playback.ducking {
                    graph.push_str(&crate::ducking::filter(self, cue.start_us, settings));
                }
            }
            graph.push_str(&format!("adelay={}S:all=1[a{i}];", sample(cue.start_us.0)));
        }
        graph.push_str("[base]");
        for i in 0..self.scene.audio.len() {
            graph.push_str(&format!("[a{i}]"));
        }
        graph.push_str(&format!(
            "amix=inputs={}:duration=first:normalize=0,atrim=end_sample={}[audio]",
            self.scene.audio.len() + 1,
            self.audio_samples()
        ));
        graph
    }
}
