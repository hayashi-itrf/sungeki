import { applyPose, type JointPose } from './pose.ts';
import { Box3, SkinnedMesh } from 'three';
import type { loadAvatar } from './assets';

// Frame the motion once, then reset the simulation before producing images.
// Sampling is bounded; production output also checks for edge contact.
export function motionBounds(avatar: Awaited<ReturnType<typeof loadAvatar>>, pose: JointPose = {}) {
  if (!Number.isFinite(avatar.duration) || avatar.duration <= 0) throw new Error('モーションの長さが不正です');
  const bounds = new Box3();
  const steps = Math.min(600, Math.max(1, Math.ceil(avatar.duration * 30)));
  const delta = avatar.duration / steps;
  for (let index = 0; index <= steps; index++) {
    avatar.mixer.setTime(index * delta);
    applyPose(avatar.vrm, pose);
    avatar.vrm.update(index === 0 ? 0 : delta);
    avatar.vrm.scene.updateMatrixWorld(true);
    avatar.vrm.scene.traverse(object => {
      if (object instanceof SkinnedMesh) object.skeleton.update();
    });
    bounds.union(new Box3().setFromObject(avatar.vrm.scene, true));
  }
  return bounds;
}
