/** Labels only. Project names, text and values are not translated. */
export const englishCollections: Record<string, string> = {
  project: 'Project', tracks: 'Tracks', clips: 'Clips', scenes: 'Scenes', groups: 'Groups', assets: 'Assets', characters: 'Characters',
};
export const englishFields: Record<string, string> = {
  outline: 'Outline', playback: 'Playback', source_start_us: 'Source start', source_duration_us: 'Source duration',
  looped: 'Loop', audio_stream: 'Video audio', gain_db: 'Volume', title: 'Project title', name: 'Name', content: 'Content',
  text: 'Text', transform: 'Placement', x: 'X position', y: 'Y position', width: 'Width', height: 'Height', opacity: 'Opacity',
  flip_x: 'Flip horizontally', fade: 'Video fade', motion: 'Move and zoom', end_x: 'End X', end_y: 'End Y',
  end_scale: 'End scale', easing: 'Easing', in_us: 'Fade in', out_us: 'Fade out', start_us: 'Start', duration_us: 'Duration',
  color: 'Text color', font_size: 'Font size', fill: 'Fill', type: 'Type', shape: 'Shape', canvas: 'Canvas', fps: 'Frame rate',
  numerator: 'Numerator', denominator: 'Denominator', background: 'Background color', visible: 'Visible', muted: 'Muted',
  kind: 'Kind', order: 'Order', scene_id: 'Scene membership', track_id: 'Track membership', clip_ids: 'Member clips',
  audio_mix: 'Audio mix', pronunciation_dictionary: 'Pronunciation dictionary', normalization: 'Loudness normalization',
  voice: 'Voice', settings: 'Settings', audio: 'Generated audio', identity: 'Original identity',
  speed_scale: 'Speech speed', pitch_scale: 'Pitch', intonation_scale: 'Intonation', volume_scale: 'Synthesis volume',
  pre_phoneme_length: 'Leading silence', post_phoneme_length: 'Trailing silence', pause_length: 'Punctuation pause',
  pause_length_scale: 'Pause scale', enable_interrogative_upspeak: 'Question upspeak', output_sampling_rate: 'Sample rate',
  output_stereo: 'Stereo output', style_id: 'Speaker style', model_id: 'Model', speaker_id: 'Speaker', language: 'Language',
  style: 'Style', style_weight: 'Style strength', length: 'Speech duration scale', sdp_ratio: 'Rhythm variation',
  noise: 'Voice variation', noisew: 'Duration variation', model: 'Model', speed: 'Speech speed', instruct: 'Speaking instructions',
  voice_id: 'Registered speaker', caption: 'Voice description', cfg_scale: 'Conditioning strength', num_steps: 'Generation steps',
  seed: 'Random seed', auto_split: 'Split sentences', split_interval: 'Sentence gap',
  layout: 'Text layout', alignment: 'Alignment', wrap: 'Wrap text',
};
export const englishActions = { added: 'Added', removed: 'Removed', changed: 'Changed', reordered: 'Reordered' };
