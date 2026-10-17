import { PerspectiveCamera, Vector3 } from 'three';
export type PreviewView = { yaw: number; pitch: number; zoom: number };
export function previewCamera(base: PerspectiveCamera, view: PreviewView) {
  if (![view.yaw, view.pitch, view.zoom].every(Number.isFinite)
    || Math.abs(view.yaw) > 180 || Math.abs(view.pitch) > 80 || view.zoom < 1 || view.zoom > 4) {
    throw Error('確認用の視点が範囲外です');
  }
  if (!view.yaw && !view.pitch && view.zoom === 1) return base;
  const target = new Vector3().fromArray(base.userData.target);
  const distance = base.position.distanceTo(target) / view.zoom;
  const yaw = view.yaw * Math.PI / 180, pitch = view.pitch * Math.PI / 180;
  const camera = base.clone();
  camera.position.copy(target).add(new Vector3(
    Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), Math.cos(yaw) * Math.cos(pitch)).multiplyScalar(distance));
  camera.lookAt(target); camera.updateMatrixWorld();
  return camera;
}
