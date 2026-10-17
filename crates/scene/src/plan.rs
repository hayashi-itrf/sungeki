use crate::{AudioCue, AudioSource, FrameSpan, first_frame_at_or_after};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use sungeki_project::*;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Primitive {
    Vrm {
        width: u32,
        height: u32,
        model_asset_id: String,
        motion_asset_id: String,
        source_time_us: TimeUs,
        expressions: BTreeMap<String, f64>,
        pose: BTreeMap<String, [f64; 3]>,
    },
    Video {
        asset_id: String,
        source_time_us: TimeUs,
    },
    Blur {
        sigma: f64,
    },
    Psd {
        asset_id: String,
        pose: PsdPose,
    },
    Image {
        asset_id: String,
    },
    Text {
        layout: Option<SubtitleLayout>,
        outline: Option<SubtitleOutline>,
        text: String,
        font_size: f64,
        color: String,
    },
    Shape {
        shape: ShapeKind,
        fill: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Layer {
    pub clip_id: String,
    pub scene_id: Option<String>,
    pub start_us: TimeUs,
    pub end_us: TimeUs,
    pub transform: Transform,
    pub primitive: Primitive,
}

/// Visual and audio visibility are independent. No files are opened and no
/// absent/stale synthesis is silently converted into an acceptable render.
/// The render preflight must validate every audible cue and captured original.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScenePlan {
    pub canvas: Canvas,
    pub duration_us: TimeUs,
    pub frame_count: u64,
    /// Back to front: track array order, then original clip array order.
    pub layers: Vec<Layer>,
    pub audio: Vec<AudioCue>,
    pub audio_mix: AudioMix,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisualSpan {
    pub frames: FrameSpan,
    /// Indices into layers, in back-to-front order.
    pub layer_indices: Vec<usize>,
}

impl ScenePlan {
    pub fn new(project: &Project) -> Result<Self, ValidationError> {
        validate(project)?;
        let duration_us = project
            .duration()
            .ok_or_else(|| ValidationError::new("作品尺が不正です".into()))?;
        let mut result = Self {
            canvas: project.canvas.clone(),
            duration_us,
            frame_count: first_frame_at_or_after(&project.canvas.fps, duration_us)
                .ok_or_else(|| ValidationError::new("フレーム数が不正です".into()))?,
            layers: vec![],
            audio: vec![],
            audio_mix: project.audio_mix.clone(),
        };
        let characters: HashMap<_, _> = project.characters.iter().map(|c| (&c.id, c)).collect();
        let mut tracks: HashMap<&str, Vec<&Clip>> = HashMap::new();
        for clip in &project.clips {
            tracks.entry(&clip.track_id).or_default().push(clip);
        }
        for track in &project.tracks {
            for clip in tracks.get(track.id.as_str()).into_iter().flatten() {
                let end_us = TimeUs(clip.start_us.0 + clip.duration_us.0);
                if let ClipContent::Audio { asset_id, playback } = &clip.content {
                    if !track.muted {
                        result.audio.push(AudioCue {
                            clip_id: clip.id.clone(),
                            start_us: clip.start_us,
                            end_us,
                            source: AudioSource::Material {
                                asset_id: asset_id.clone(),
                                playback: playback.clone(),
                            },
                            gain_db: Some(playback.gain_db),
                        });
                    }
                }
                if let ClipContent::Speech { audio, gain_db, .. } = &clip.content {
                    if !track.muted {
                        result.audio.push(AudioCue {
                            clip_id: clip.id.clone(),
                            start_us: clip.start_us,
                            end_us,
                            source: AudioSource::Speech {
                                audio: audio.clone(),
                            },
                            gain_db: *gain_db,
                        });
                    }
                }
                if let ClipContent::Video { asset_id, playback } = &clip.content {
                    if !track.muted && playback.audio_stream.is_some() {
                        result.audio.push(AudioCue {
                            clip_id: clip.id.clone(),
                            start_us: clip.start_us,
                            end_us,
                            source: AudioSource::Video {
                                asset_id: asset_id.clone(),
                                playback: playback.clone(),
                            },
                            gain_db: Some(playback.gain_db),
                        });
                    }
                }
                if !track.visible {
                    continue;
                }
                if matches!(clip.content, ClipContent::Video { .. }) {
                    crate::video_layers::append(project, clip, &mut result.layers)?;
                    continue;
                }
                if let ClipContent::Psd {
                    asset_id,
                    pose,
                    speech: Some(binding),
                } = &clip.content
                {
                    result.layers.extend(crate::psd_speech::layers(
                        project, clip, asset_id, pose, binding,
                    )?);
                    if result.layers.len() > 100_000 {
                        return Err(ValidationError::new(
                            "描画区間が100,000を超えています".into(),
                        ));
                    }
                    continue;
                }
                let primitive = match &clip.content {
                    ClipContent::Vrm { .. } => {
                        crate::vrm_layers::append(project, clip, &mut result.layers)?;
                        continue;
                    }
                    ClipContent::Sprite {
                        sprites,
                        pose,
                        character_id,
                    } => {
                        result.layers.extend(crate::sprite::layers(
                            project,
                            clip,
                            sprites,
                            pose.as_deref(),
                            character_id.as_deref(),
                        )?);
                        if result.layers.len() > 100_000 {
                            return Err(ValidationError::new(
                                "描画区間が100,000を超えています".into(),
                            ));
                        }
                        continue;
                    }
                    ClipContent::Blur { sigma } => Primitive::Blur { sigma: *sigma },
                    ClipContent::Audio { .. } => continue,
                    ClipContent::Video { .. } => {
                        unreachable!("video layers were expanded above");
                    }
                    ClipContent::Psd { asset_id, pose, .. } => Primitive::Psd {
                        asset_id: asset_id.clone(),
                        pose: pose.clone(),
                    },
                    ClipContent::Image { asset_id } => Primitive::Image {
                        asset_id: asset_id.clone(),
                    },
                    ClipContent::Text {
                        outline,
                        layout,
                        text,
                        font_size,
                        color,
                    } => Primitive::Text {
                        layout: layout.clone(),
                        outline: outline.clone(),
                        text: text.clone(),
                        font_size: *font_size,
                        color: color.clone(),
                    },
                    ClipContent::Shape { shape, fill } => Primitive::Shape {
                        shape: shape.clone(),
                        fill: fill.clone(),
                    },
                    ClipContent::Speech {
                        character_id,
                        subtitle_text,
                        ..
                    } => {
                        let style = &characters[character_id].subtitle_style;
                        Primitive::Text {
                            layout: style.layout.clone(),
                            outline: style.outline.clone(),
                            text: subtitle_text.clone(),
                            font_size: style.font_size,
                            color: style.color.clone(),
                        }
                    }
                };
                result.layers.push(Layer {
                    clip_id: clip.id.clone(),
                    scene_id: clip.scene_id.clone(),
                    start_us: clip.start_us,
                    end_us,
                    transform: clip.transform.clone(),
                    primitive,
                });
            }
        }
        result.layers = crate::visual_fade::expand(project, result.layers)?;
        crate::scene_fade::append(project, &mut result.layers)?;
        Ok(result)
    }

    pub fn layers_at(&self, time: TimeUs) -> impl Iterator<Item = &Layer> {
        self.layers
            .iter()
            .filter(move |layer| layer.start_us <= time && time < layer.end_us)
    }

    pub fn frame(&self, index: u64) -> Option<Vec<&Layer>> {
        if index >= self.frame_count {
            return None;
        }
        Some(self.layers_at(self.canvas.fps.frame_time(index)?).collect())
    }

    /// Partition only at visible clip boundaries, never allocate per output frame.
    /// Video primitives already have one source-time sample per output frame.
    pub fn visual_spans(&self) -> impl Iterator<Item = VisualSpan> {
        let mut events: BTreeMap<u64, Vec<(usize, bool)>> = BTreeMap::new();
        events.entry(0).or_default();
        events.entry(self.frame_count).or_default();
        for (index, layer) in self.layers.iter().enumerate() {
            let start = first_frame_at_or_after(&self.canvas.fps, layer.start_us).unwrap();
            let end = first_frame_at_or_after(&self.canvas.fps, layer.end_us).unwrap();
            if start < end {
                events.entry(start).or_default().push((index, true));
                events.entry(end).or_default().push((index, false));
            }
        }
        events
            .into_iter()
            .scan((0, BTreeSet::new()), |(first, active), (at, changes)| {
                let span = (*first < at).then(|| VisualSpan {
                    frames: FrameSpan {
                        first: *first,
                        end: at,
                    },
                    layer_indices: active.iter().copied().collect(),
                });
                for (index, enters) in changes {
                    if enters {
                        active.insert(index);
                    } else {
                        active.remove(&index);
                    }
                }
                *first = at;
                Some(span)
            })
            .flatten()
    }
}
