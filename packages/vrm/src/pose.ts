import { Euler } from 'three';
import type { VRM, VRMHumanBoneName } from '@pixiv/three-vrm';
export type JointPose = Record<string, [number, number, number]>;
export const poseBones = ['hips','spine','chest','upperChest','neck','head',
  'leftShoulder','leftUpperArm','leftLowerArm','leftHand','rightShoulder','rightUpperArm','rightLowerArm','rightHand',
  'leftUpperLeg','leftLowerLeg','leftFoot','rightUpperLeg','rightLowerLeg','rightFoot'] as const;
export function validatePose(pose: JointPose) {
  if (!pose || typeof pose !== 'object' || Array.isArray(pose)) throw Error('VRMポーズが不正です');
  for (const [bone, angles] of Object.entries(pose)) {
    if (!(poseBones as readonly string[]).includes(bone) || !Array.isArray(angles) || angles.length !== 3
      || angles.some(v => typeof v !== 'number' || !Number.isFinite(v) || Math.abs(v) > 180)) {
      throw Error('VRMの関節名または角度（−180〜180度）が不正です');
    }
  }
}
export function applyPose(vrm: VRM, pose: JointPose) {
  for (const [bone, angles] of Object.entries(pose)) {
    const node = vrm.humanoid.getNormalizedBoneNode(bone as VRMHumanBoneName);
    if (!node) throw Error(`モデルに関節 ${bone} がありません`);
    node.quaternion.setFromEuler(new Euler(...angles.map(v => v * Math.PI / 180) as [number, number, number], 'XYZ'));
  }
}
