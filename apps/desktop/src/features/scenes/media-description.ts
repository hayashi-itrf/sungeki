import type { Command, Project } from '@sungeki/api-client';

/** English descriptions for media edits; user-authored names remain unchanged. */
export function describeMedia(command: Command, project: Project): string | null {
  switch (command.type) {
    case 'set_text_outline': return command.outline
      ? `Set the text outline to ${command.outline.width}px, ${command.outline.color}.` : 'Remove the text outline.';
    case 'set_text_layout': return command.layout
      ? `Align text ${command.layout.alignment}; automatic wrapping ${command.layout.wrap ? 'on' : 'off'}.` : 'Remove custom text layout.';
    case 'set_video': return `Use ${command.playback.source_duration_us / 1e6} s of source video starting at ${command.playback.source_start_us / 1e6} s (${command.playback.looped ? 'repeat' : 'play once'}; ${command.playback.audio_stream === null ? 'no source audio' : 'use selected source audio'}).`;
    case 'set_transform': {
      const t = command.transform, m = t.motion;
      return `Set placement to X ${t.x}, Y ${t.y}, width ${t.width}, height ${t.height}. ${m ? `Move to X ${m.end_x}, Y ${m.end_y}, final scale ${m.end_scale}, ${m.easing === 'linear' ? 'constant speed' : 'smooth motion'}.` : 'No movement or scaling.'}`;
    }
    case 'refresh_scene_credits': {
      const scene = project.scenes.find(s => s.id === command.scene_id);
      return `Replace the selected text with asset and speaker credits for ${scene ? `scene “${scene.name}”` : 'the selected scene'}.`;
    }
    case 'fit_visual_to_speech': return 'Fit the selected image, character, text or shape from the first speech start to the last speech end.';
    case 'apply_dialogue_template': return `Create dialogue “${command.template.title}” from ${command.template.start_us / 1e6} s with ${command.template.speech_clip_ids.length} speech clips, a background, two characters, BGM, a title and credits.`;
    case 'layout_speech': return `Arrange ${command.clip_ids.length} speech clips using their generated audio lengths from ${command.start_us / 1e6} s with gaps of ${command.gap_us / 1e6} s.`;
    case 'set_reaction_motion': return `Enlarge the image around (${command.center_x}, ${command.center_y}) to ${command.width} × ${command.height}; initial scale ${command.start_scale}, fade in ${command.fade.in_us / 1e6} s, fade out ${command.fade.out_us / 1e6} s. This replaces existing movement, scaling and fades.`;
    default: return null;
  }
}
