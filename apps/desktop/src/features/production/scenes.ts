import { locale } from '../../shared/i18n/locale.svelte';
import { sceneFadeFromForm, type SceneFadeInput } from './scene-fade';
import type { BuildScene, Project } from '@sungeki/api-client';
import { diagramsFromForm, type DiagramInput } from './diagrams';
import { parseDialogue } from './dialogue';
export type SceneInput = { name: string; background: string; script: string; gapSeconds: number | undefined; fade?: SceneFadeInput; diagrams: DiagramInput[] };
export function newScene(name: string): SceneInput {
  return { name, background: '', gapSeconds: 0.15, script: locale.current === 'en' ? 'left: Hello.\nright: Let’s make a video.' : '左：こんにちは。\n右：動画を作ってみましょう。', diagrams: [] };
}
export function scenesFromForm(project: Project, input: SceneInput[]): BuildScene[] {
  if (!input.length || input.length > 200) throw new Error(locale.text('productionValidationSceneCount'));
  const scenes = input.map((scene, index) => {
    try {
      if (!scene.name.trim()) throw new Error(locale.text('productionValidationSceneName'));
      if (project.assets.find(a => a.id === scene.background)?.probe.type !== 'image') throw new Error(locale.text('productionValidationBackground'));
      if (scene.gapSeconds === undefined || !Number.isFinite(scene.gapSeconds) || scene.gapSeconds < 0 || scene.gapSeconds > 60) throw new Error(locale.text('productionValidationGap'));
      const fade = sceneFadeFromForm(scene.fade);
      const gap = Math.round(scene.gapSeconds * 1_000_000);
      const lines = parseDialogue(scene.script);
      if (!lines.length || lines.length > 200) throw new Error(locale.text('productionValidationLineCount'));
      return { name: scene.name, background_asset_id: scene.background, gap_us: gap, ...(fade ? { fade } : {}), lines, diagrams: diagramsFromForm(project, scene.diagrams, lines.length) };
    } catch (error) { throw new Error(locale.text('productionValidationSceneLocation').replace('{scene}', String(index + 1)) + (error instanceof Error ? error.message : String(error))); }
  });
  if (scenes.reduce((sum, scene) => sum + scene.lines.length, 0) > 200) throw new Error(locale.text('productionValidationTotalLines'));
  return scenes;
}
