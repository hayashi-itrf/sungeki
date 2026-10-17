import type { Clip, Issue } from '@sungeki/api-client';
export { assetUses } from '../editor/asset-uses';
export function issueTime(issue: Issue, clip: Clip | undefined, stale: boolean): number | undefined {
  const time = issue.start_us;
  return !stale && clip && time !== null && Number.isSafeInteger(time)
    && time >= clip.start_us && time < clip.start_us + clip.duration_us ? time : undefined;
}
