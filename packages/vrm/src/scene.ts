import { previewCamera, type PreviewView } from './preview-camera';
import { applyPose, validatePose, poseBones, type JointPose } from './pose';
import { vrmClock } from './clock';
import { validateExpressions, withExpressions, type ExpressionWeights } from './expressions';
import { AmbientLight, DirectionalLight, PerspectiveCamera, Scene, SRGBColorSpace, WebGLRenderer } from 'three';
import { disposeAvatar } from './dispose';
import { loadAvatar } from './assets';
import { fitBounds } from './camera';
import { motionBounds } from './motion-bounds';
import { inspectPixels } from './pixels';

export class VrmScene {
  private readonly scene = new Scene();
  private readonly renderer: WebGLRenderer;
  private camera: PerspectiveCamera | undefined;
  private avatar: Awaited<ReturnType<typeof loadAvatar>> | undefined;
  private pose: JointPose = {};
  private poseKey = '[]';
  private previewView: PreviewView = { yaw: 0, pitch: 0, zoom: 1 };
  private tick = 0;
  private modelLoads = 0;
  private simulationSteps = 0;
  private partial = false;
  private sampledSeconds: number | undefined;
  private rendering = false;
  private shaderError: string | undefined;
  private disposed = false;
  private readonly model: ArrayBuffer;
  private readonly motion: ArrayBuffer;

  constructor(canvas: HTMLCanvasElement, model: ArrayBuffer, motion: ArrayBuffer) {
    this.model = model;
    this.motion = motion;
    if (canvas.width < 1 || canvas.height < 1 || canvas.width > 2048 || canvas.height > 2048) {
      throw new Error('M0の描画寸法は1〜2048です');
    }
    const context = canvas.getContext('webgl2', { alpha: true, antialias: true, preserveDrawingBuffer: true });
    if (!context) throw new Error('WebGL2を作成できません');
    this.renderer = new WebGLRenderer({ canvas, context, alpha: true, antialias: true });
    this.renderer.setPixelRatio(1);
    this.renderer.setSize(canvas.width, canvas.height, false);
    this.renderer.setClearColor(0, 0);
    this.renderer.outputColorSpace = SRGBColorSpace;
    this.renderer.debug.onShaderError = (gl, program) => {
      this.shaderError = gl.getProgramInfoLog(program) || 'Shader compilation failed';
    };
    this.scene.add(new AmbientLight(0xffffff, 1));
    const light = new DirectionalLight(0xffffff, 2);
    light.position.set(1, 2, 3);
    this.scene.add(light);
  }

  async initialize() {
    await this.reset();
    this.camera = fitBounds(motionBounds(this.avatar!),
      this.renderer.domElement.width / this.renderer.domElement.height);
    await this.reset();
    const gl = this.renderer.getContext();
    if (!(gl instanceof WebGL2RenderingContext)) throw new Error('WebGL2ではありません');
    const debug = gl.getExtension('WEBGL_debug_renderer_info');
    return {
      webglVersion: gl.getParameter(gl.VERSION) as string,
      renderer: debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) as string : 'unavailable',
      vrmVersion: this.avatar!.vrm.meta.metaVersion,
      springJoints: this.avatar!.vrm.springBoneManager?.joints.size ?? 0,
      duration: this.avatar!.duration, tracks: this.avatar!.tracks,
      bones: poseBones.filter(name => this.avatar!.vrm.humanoid.getNormalizedBoneNode(name)),
      expressions: Object.keys(this.avatar!.vrm.expressionManager?.expressionMap ?? {}).sort(),
    };
  }

  async frame(frame: number, fresh = false, expressions: ExpressionWeights = {}) {
    if (!Number.isInteger(frame) || frame < 0 || frame > 3600) throw new Error('フレーム番号が範囲外です');
    const result = await this.sample({ tick: frame * 2, residual: 0, seconds: frame / 30 }, fresh, expressions);
    return { ...result, frame };
  }

  async frameAt(timeUs: number, fresh = false, expressions: ExpressionWeights = {}, pose: JointPose = {}) {
    await this.setPose(pose);
    const result = await this.sample(vrmClock(timeUs), fresh, expressions);
    return { ...result, timeUs };
  }

  async displayAt(timeUs: number, expressions: ExpressionWeights = {}, pose: JointPose = {}, cancelled = () => false, view: PreviewView = { yaw: 0, pitch: 0, zoom: 1 }) {
    await this.setPose(pose);
    this.previewView = view;
    return this.sample(vrmClock(timeUs), false, expressions, false, cancelled);
  }

  private async setPose(pose: JointPose) {
    validatePose(pose);
    for (const bone of Object.keys(pose)) {
      if (!this.avatar?.vrm.humanoid.getNormalizedBoneNode(bone as typeof poseBones[number])) {
        throw Error(`モデルに関節 ${bone} がありません`);
      }
    }
    const key = JSON.stringify(Object.entries(pose).sort(([a], [b]) => a.localeCompare(b)));
    if (key === this.poseKey) return;
    this.poseKey = key; this.pose = structuredClone(pose);
    // Reframe using the same posed motion bounds in both editor and export.
    this.rewind();
    this.camera = fitBounds(motionBounds(this.avatar!, this.pose),
      this.renderer.domElement.width / this.renderer.domElement.height);
    this.rewind();
  }

  private rewind() {
    const avatar = this.avatar!;
    avatar.mixer.stopAllAction();
    avatar.mixer.uncacheRoot(avatar.vrm.scene);
    avatar.vrm.humanoid.resetNormalizedPose();
    avatar.mixer.clipAction(avatar.clip).play();
    avatar.mixer.setTime(0);
    applyPose(avatar.vrm, this.pose);
    avatar.vrm.update(0);
    avatar.vrm.scene.updateMatrixWorld(true);
    avatar.vrm.springBoneManager?.reset();
    this.tick = 0; this.partial = false; this.sampledSeconds = undefined;
  }

  private async sample(clock: ReturnType<typeof vrmClock>, fresh: boolean, expressions: ExpressionWeights, capture = true, cancelled = () => false) {
    if (!this.camera || this.disposed) throw new Error('描画シーンが利用できません');
    if (this.rendering) throw new Error('VRM描画は1件ずつ実行してください');
    validateExpressions(this.avatar!.vrm.expressionManager, expressions);
    this.rendering = true;
    try {
      // Expression variants at the exact same sampled time share the physical
      // state. withExpressions restores the previous values after each capture.
      if (!fresh && this.sampledSeconds === clock.seconds) {
        return withExpressions(this.avatar!.vrm.expressionManager, expressions, () => this.capture(clock.seconds, capture));
      }
      this.sampledSeconds = undefined;
      // Physics advances only on the canonical 60 Hz clock. Fractional poses
      // are presentation samples and never add a variable simulation step.
      if (fresh || clock.tick < this.tick) {
        // Editing seeks reuse the verified model. Reset animation bindings and
        // physics instead of decoding GLB and uploading textures again.
        if (!capture && !fresh) this.rewind();
        else await this.reset();
      }
      else if (this.partial) {
        this.avatar!.mixer.setTime(this.tick / 60);
        applyPose(this.avatar!.vrm, this.pose);
      this.avatar!.vrm.update(0);
        this.partial = false;
      }
      let sliceStarted = performance.now();
      while (this.tick < clock.tick) {
        this.tick++;
        this.simulationSteps++;
        this.avatar!.mixer.setTime(this.tick / 60);
        applyPose(this.avatar!.vrm, this.pose);
        this.avatar!.vrm.update(1 / 60);
        // Yield by actual work time, not simulated duration. Fast models
        // should not wait for a display frame every four seconds of motion.
        if (!capture && this.tick % 32 === 0 && performance.now() - sliceStarted >= 8) {
          await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
          if (this.disposed || cancelled()) throw Error('VRM表示を更新しました');
          sliceStarted = performance.now();
        }
      }
      if (clock.residual > 0) {
        this.partial = true;
        this.avatar!.mixer.setTime(clock.seconds);
        applyPose(this.avatar!.vrm, this.pose);
      this.avatar!.vrm.update(0);
      }
      const result = withExpressions(this.avatar!.vrm.expressionManager, expressions, () => this.capture(clock.seconds, capture));
      this.sampledSeconds = clock.seconds;
      return result;
    } finally { this.rendering = false; }
  }

  private capture(seconds: number, capture = true) {
    this.renderer.render(this.scene, capture ? this.camera! : previewCamera(this.camera!, this.previewView));
    if (this.shaderError) throw new Error(this.shaderError);
    const gl = this.renderer.getContext();
    const error = gl.getError();
    if (error !== gl.NO_ERROR || gl.isContextLost()) throw new Error(`WebGL描画エラー: ${error}`);
    const canvas = this.renderer.domElement;
    return {
      modelLoads: this.modelLoads, simulationSteps: this.simulationSteps,
      seconds, png: capture ? canvas.toDataURL('image/png') : '',
      pixels: capture ? inspectPixels(gl, canvas.width, canvas.height) : { visible: 0, edge: 0, partial: 0, transparent: 0 },
      calls: this.renderer.info.render.calls, triangles: this.renderer.info.render.triangles,
    };
  }

  private async reset() {
    this.releaseAvatar();
    const avatar = await loadAvatar(this.model, this.motion);
    if (this.disposed) {
      avatar.mixer.stopAllAction(); avatar.mixer.uncacheRoot(avatar.vrm.scene);
      disposeAvatar(avatar.vrm.scene);
      throw new Error('描画シーンは終了しました');
    }
    this.modelLoads++;
    this.avatar = avatar;
    applyPose(avatar.vrm, this.pose);
    avatar.vrm.update(0);
    this.scene.add(this.avatar.vrm.scene);
    this.tick = 0;
    this.partial = false;
    this.sampledSeconds = undefined;
  }

  private releaseAvatar() {
    if (!this.avatar) return;
    this.avatar.mixer.stopAllAction();
    this.avatar.mixer.uncacheRoot(this.avatar.vrm.scene);
    this.scene.remove(this.avatar.vrm.scene);
    disposeAvatar(this.avatar.vrm.scene);
    this.avatar = undefined;
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.releaseAvatar();
    this.renderer.dispose();
    this.renderer.forceContextLoss();
  }
}
