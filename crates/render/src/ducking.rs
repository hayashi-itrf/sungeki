use crate::RenderPlan;
use sungeki_project::{AssetProbe, Ducking, TimeUs};
use sungeki_scene::{AudioSource, DuckingEnvelope};

/// The validated plan contains only audible cues and has checked synthesis
/// freshness and original duration. Subtitle tail padding never drives ducking.
pub(crate) fn envelope(plan: &RenderPlan, settings: &Ducking) -> DuckingEnvelope {
    let windows = plan.scene.audio.iter().filter_map(|cue| {
        if !matches!(cue.source, AudioSource::Speech { .. }) {
            return None;
        }
        let AssetProbe::Audio(probe) = &plan.sources[cue.source.asset_id().unwrap()].probe else {
            unreachable!("validated speech probe")
        };
        Some((cue.start_us, TimeUs(cue.start_us.0 + probe.duration_us.0)))
    });
    DuckingEnvelope::new(
        windows,
        settings.attenuation_db,
        settings.attack_us,
        settings.release_us,
    )
    .expect("validated timeline and ducking settings")
}

// FFmpeg's if evaluates only the chosen branch. Binary search selects the
// preceding release / following attack, as in DuckingEnvelope::gain_at.
fn weight(
    windows: &[(TimeUs, TimeUs)],
    lo: usize,
    hi: usize,
    t: &str,
    settings: &Ducking,
) -> String {
    if lo < hi {
        let mid = (lo + hi) / 2;
        return format!(
            "if(lt({t},{}),{},{})",
            windows[mid].0.0,
            weight(windows, lo, mid, t, settings),
            weight(windows, mid + 1, hi, t, settings)
        );
    }
    let previous = if lo == 0 {
        "0".into()
    } else {
        let end = windows[lo - 1].1.0;
        let release = if settings.release_us.0 == 0 {
            "0".into()
        } else {
            format!("max(0,1-({t}-{end})/{})", settings.release_us.0)
        };
        format!("if(lt({t},{end}),1,{release})")
    };
    let next = if lo == windows.len() || settings.attack_us.0 == 0 {
        "0".into()
    } else {
        format!(
            "max(0,1-({}-{t})/{})",
            windows[lo].0.0, settings.attack_us.0
        )
    };
    format!("max({previous},{next})")
}

pub(crate) fn filter(plan: &RenderPlan, start: TimeUs, settings: &Ducking) -> String {
    let envelope = envelope(plan, settings);
    let offset = (start.0 * 48000 + 500000) / 1000000;
    // n is evaluated per sample, before adelay, on the 48kHz timeline.
    // Keep fractional microseconds: rounding them causes boundary drift.
    let t = format!("((n+{offset})*1000000/48000)");
    let windows: Vec<_> = envelope.windows().collect();
    if windows.is_empty() || settings.attenuation_db == 0.0 {
        return String::new();
    }
    let gain = format!(
        "(1+({:.17})*({}))",
        10f64.powf(settings.attenuation_db / 20.0) - 1.0,
        weight(&windows, 0, windows.len(), &t, settings)
    );
    format!("aeval=exprs='val(0)*{gain}|val(1)*{gain}',")
}
