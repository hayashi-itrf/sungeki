use super::{VoicevoxError, VoicevoxSettings, finite};
use serde::{Deserialize, Serialize};

pub const MAX_QUERY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mora {
    pub text: String,
    pub consonant: Option<String>,
    pub consonant_length: Option<f64>,
    pub vowel: String,
    pub vowel_length: f64,
    pub pitch: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccentPhrase {
    pub moras: Vec<Mora>,
    pub accent: usize,
    pub pause_mora: Option<Mora>,
    #[serde(default)]
    pub is_interrogative: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioQuery {
    #[serde(rename = "accent_phrases")]
    pub accent_phrases: Vec<AccentPhrase>,
    pub speed_scale: f64,
    pub pitch_scale: f64,
    pub intonation_scale: f64,
    pub volume_scale: f64,
    pub pre_phoneme_length: f64,
    pub post_phoneme_length: f64,
    pub pause_length: Option<f64>,
    #[serde(default = "one")]
    pub pause_length_scale: f64,
    pub output_sampling_rate: u32,
    pub output_stereo: bool,
    pub kana: Option<String>,
}
fn one() -> f64 {
    1.0
}
impl AudioQuery {
    pub fn validate(&self) -> Result<(), VoicevoxError> {
        self.settings().validate()?;
        for phrase in &self.accent_phrases {
            if phrase.accent == 0 || phrase.accent > phrase.moras.len() {
                return Err(VoicevoxError("アクセント位置がモーラの範囲外です".into()));
            }
            for mora in phrase.moras.iter().chain(phrase.pause_mora.iter()) {
                finite("母音の長さ", mora.vowel_length, Some(0.0))?;
                finite("モーラの音高", mora.pitch, None)?;
                if let Some(length) = mora.consonant_length {
                    finite("子音の長さ", length, Some(0.0))?;
                }
            }
        }
        Ok(())
    }
    fn settings(&self) -> VoicevoxSettings {
        // These two request parameters are not carried in AudioQuery's JSON body.
        VoicevoxSettings {
            style_id: 0,
            enable_interrogative_upspeak: false,
            speed_scale: self.speed_scale,
            pitch_scale: self.pitch_scale,
            intonation_scale: self.intonation_scale,
            volume_scale: self.volume_scale,
            pre_phoneme_length: self.pre_phoneme_length,
            post_phoneme_length: self.post_phoneme_length,
            pause_length: self.pause_length,
            pause_length_scale: self.pause_length_scale,
            output_sampling_rate: self.output_sampling_rate,
            output_stereo: self.output_stereo,
        }
    }
}

/// Validate the engine response before applying every user control. The caller
/// must use settings.style_id for BOTH audio_query and synthesis requests.
/// Unknown response extensions are ignored; known required fields never default.
/// VoicevoxClient also caps the response while reading its bytes.
pub fn prepare_synthesis_query(
    response: &[u8],
    settings: &VoicevoxSettings,
) -> Result<AudioQuery, VoicevoxError> {
    settings.validate()?;
    if response.len() > MAX_QUERY_BYTES {
        return Err(VoicevoxError(
            "VOICEVOXの解析応答が8MiBを超えています".into(),
        ));
    }
    let mut query: AudioQuery = serde_json::from_slice(response)
        .map_err(|error| VoicevoxError(format!("VOICEVOXの解析応答が不正です: {error}")))?;
    query.validate()?;
    query.speed_scale = settings.speed_scale;
    query.pitch_scale = settings.pitch_scale;
    query.intonation_scale = settings.intonation_scale;
    query.volume_scale = settings.volume_scale;
    query.pre_phoneme_length = settings.pre_phoneme_length;
    query.post_phoneme_length = settings.post_phoneme_length;
    query.pause_length = settings.pause_length;
    query.pause_length_scale = settings.pause_length_scale;
    query.output_sampling_rate = settings.output_sampling_rate;
    query.output_stereo = settings.output_stereo;
    Ok(query)
}
