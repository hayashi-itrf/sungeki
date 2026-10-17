import { VrmScene, type JointPose, type ExpressionWeights } from '@sungeki/vrm';

async function createScene(width: number, height: number) {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  document.body.replaceChildren(canvas);
  const base = new URL('.', window.location.href);
  const load = async (name: string) => {
    const response = await fetch(new URL(name, base));
    if (!response.ok) throw new Error(`素材読込失敗: ${response.status}`);
    return response.arrayBuffer();
  };
  const scene = new VrmScene(canvas, await load('model'), await load('motion'));
  try {
    const info = await scene.initialize();
    return { scene, info };
  } catch (error) {
    scene.dispose();
    throw error;
  }
}

let active: Awaited<ReturnType<typeof createScene>> | undefined;
const probe = {
  async load(width: number, height: number) {
    active?.scene.dispose();
    active = undefined;
    active = await createScene(width, height);
    return active.info;
  },
  frame(frame: number, fresh = false, expressions: ExpressionWeights = {}) {
    if (!active) throw new Error('モデルを先に読み込んでください');
    return active.scene.frame(frame, fresh, expressions);
  },
  frameAt(timeUs: number, fresh = false, expressions: ExpressionWeights = {}, pose: JointPose = {}) {
    if (!active) throw new Error('モデルを先に読み込んでください');
    return active.scene.frameAt(timeUs, fresh, expressions, pose);
  },
  async previewAt(timeUs: number, expressions: ExpressionWeights = {}, pose: JointPose = {}) {
    if (!active) throw new Error('モデルを先に読み込んでください');
    const result = await active.scene.displayAt(timeUs, expressions, pose);
    const canvas = document.querySelector('canvas')!;
    const bytes = await new Promise<Blob>((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) : reject(Error('PNG failed'))));
    const hash = await crypto.subtle.digest('SHA-256', await bytes.arrayBuffer());
    return { timeUs, modelLoads: result.modelLoads, simulationSteps: result.simulationSteps,
      pngHash: Array.from(new Uint8Array(hash), value => value.toString(16).padStart(2, '0')).join('') };
  },
  dispose() { active?.scene.dispose(); active = undefined; },
};
declare global { interface Window { vvmVrmProbe: typeof probe } }
window.vvmVrmProbe = probe;
