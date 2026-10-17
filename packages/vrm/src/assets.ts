import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { VRM, VRMLoaderPlugin, VRMUtils } from '@pixiv/three-vrm';
import { VRMAnimationLoaderPlugin, VRMLookAtQuaternionProxy, createVRMAnimationClip } from '@pixiv/three-vrm-animation';
import { disposeAvatar } from './dispose';
import { AnimationMixer, LoopRepeat, LoadingManager } from 'three';

export async function loadAvatar(model: ArrayBuffer, motion: ArrayBuffer) {
  const manager = new LoadingManager();
  manager.setURLModifier(url => {
    if (!url.startsWith('blob:') && !url.startsWith('data:')) throw Error('VRM外部リソースの読込はできません');
    return url;
  });
  const loader = new GLTFLoader(manager);
  loader.register((parser) => new VRMLoaderPlugin(parser));
  const gltf = await loader.parseAsync(model.slice(0), '');
  const vrm = gltf.userData.vrm as VRM | undefined;
  if (!vrm) { disposeAvatar(gltf.scene); throw new Error('VRMモデルではありません'); }
  try {
    VRMUtils.rotateVRM0(vrm);
    const motionLoader = new GLTFLoader(manager);
    motionLoader.register((parser) => new VRMAnimationLoaderPlugin(parser));
    const animation = await motionLoader.parseAsync(motion.slice(0), '');
    const motions = animation.userData.vrmAnimations;
    if (!Array.isArray(motions) || motions.length !== 1) throw new Error('単一のVRMAが必要です');
    if (vrm.lookAt) {
      const proxy = new VRMLookAtQuaternionProxy(vrm.lookAt);
      proxy.name = 'VRMLookAtQuaternionProxy';
      vrm.scene.add(proxy);
    }
    const clip = createVRMAnimationClip(motions[0], vrm);
    if (!(clip.duration > 0) || clip.tracks.length === 0) throw new Error('VRMAに有効なモーションがありません');
    const mixer = new AnimationMixer(vrm.scene);
    mixer.clipAction(clip).setLoop(LoopRepeat, Infinity).play();
    mixer.setTime(0);
    vrm.update(0);
    vrm.scene.updateMatrixWorld(true);
    vrm.springBoneManager?.reset();
    return { vrm, mixer, clip, duration: clip.duration, tracks: clip.tracks.length };
  } catch (error) {
    disposeAvatar(vrm.scene);
    throw error;
  }
}
