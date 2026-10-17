import { Material, Object3D, Texture } from 'three';
import { VRMUtils } from '@pixiv/three-vrm';

// deepDispose releases GPU textures but ImageBitmap needs an explicit close.
export function disposeAvatar(root: Object3D) {
  const images = new Set<ImageBitmap>();
  const inspect = (value: unknown) => {
    if (value instanceof Texture && value.image instanceof ImageBitmap) images.add(value.image);
  };
  root.traverse((object) => {
    const material = (object as Object3D & { material?: Material | Material[] }).material;
    for (const item of Array.isArray(material) ? material : material ? [material] : []) {
      Object.values(item).forEach(inspect);
      const uniforms = (item as Material & { uniforms?: Record<string, { value: unknown }> }).uniforms;
      if (uniforms) Object.values(uniforms).forEach((uniform) => inspect(uniform.value));
    }
  });
  VRMUtils.deepDispose(root);
  images.forEach((image) => image.close());
}
