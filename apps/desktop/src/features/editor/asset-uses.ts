import type { Clip, Project } from '@sungeki/api-client';
export function assetUses(project: Project, assetId: string): Clip[] {
  return project.clips.filter(clip => {
    const content = clip.content;
    switch (content.type) {
      case 'image': case 'psd': case 'audio': case 'video': return content.asset_id === assetId;
      case 'sprite': return content.sprites.poses.some(pose =>
        pose.closed_asset_id === assetId || pose.open_asset_id === assetId);
      case 'vrm': return content.standing.model_asset_id === assetId || content.standing.motion_asset_id === assetId;
      case 'speech': return content.audio?.asset_id === assetId;
      case 'blur': case 'text': case 'shape': return false;
      default: {
        const unsupported: never = content;
        return unsupported;
      }
    }
  }).sort((a, b) => a.start_us - b.start_us || a.id.localeCompare(b.id));
}
