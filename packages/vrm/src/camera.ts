import { Box3, Object3D, PerspectiveCamera, Vector3 } from 'three';

export function fitCamera(object: Object3D, aspect: number): PerspectiveCamera {
  if (!Number.isFinite(aspect) || aspect <= 0) throw new Error('描画比率が不正です');
  return fitBounds(new Box3().setFromObject(object), aspect);
}

export function fitBounds(bounds: Box3, aspect: number): PerspectiveCamera {
  if (!Number.isFinite(aspect) || aspect <= 0) throw new Error('描画比率が不正です');
  const size = bounds.getSize(new Vector3());
  const center = bounds.getCenter(new Vector3());
  if (![size.x, size.y, size.z].every(Number.isFinite) || size.y <= 0) {
    throw new Error('モデルの境界を計算できません');
  }
  const camera = new PerspectiveCamera(30, aspect, .01, 100);
  const halfFov = Math.tan(camera.fov * Math.PI / 360);
  const distance = Math.max(size.y / halfFov, size.x / (halfFov * aspect)) * .7 + size.z;
  camera.position.set(center.x, center.y, center.z + distance);
  camera.userData.target = center.toArray();
  camera.lookAt(center);
  camera.updateMatrixWorld();
  return camera;
}
