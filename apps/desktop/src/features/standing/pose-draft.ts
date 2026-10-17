import type { PsdPose } from '@sungeki/api-client';

// Selecting a parent resets overrides below it; the host validates the resulting pose.
export function selectPoseOption(pose: PsdPose, group: string, option: string): PsdPose {
  const selections = pose.selections.filter(selection =>
    selection.group !== group && !selection.group.startsWith(`${group}/`));
  if (option) selections.push({ group, option });
  return { ...pose, selections };
}
