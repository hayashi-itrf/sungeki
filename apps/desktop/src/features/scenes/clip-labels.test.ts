import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { ClipContent, Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/scene-evaluation.json';
import { locale } from '../../shared/i18n/locale.svelte';
import SceneForm from './SceneForm.svelte';
import GroupForm from './GroupForm.svelte';
import SelectionForm from './SelectionForm.svelte';

afterEach(() => { locale.current = 'ja'; });

// Display-only content: asset loading and host validation are covered elsewhere.
const contents = [
  { type: 'speech', subtitle_text: '残す字幕の内容', spoken_text: '読み上げ用' },
  { type: 'speech', subtitle_text: '', spoken_text: '表示する読み上げ文' },
  { type: 'text', text: '残すテキスト' },
  ...['image', 'video', 'audio', 'vrm', 'psd', 'sprite', 'blur', 'shape'].map(type => ({ type })),
] as ClipContent[];

test.each([SceneForm, GroupForm, SelectionForm])('selection labels identify speech and media in each structure form', Form => {
  const project = structuredClone(fixture.project) as Project;
  project.scenes = []; project.groups = [];
  project.clips = contents.map((content, i) => ({
    id: `display-${i}`, track_id: 'track', scene_id: null, start_us: 0, duration_us: 1000000,
    transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 }, content,
  }));
  const before = JSON.stringify(project), submit = vi.fn();
  for (const [language, labels, shape] of [
    ['ja', ['画像', '動画', '音声素材', 'VRM立ち絵', 'PSD立ち絵', 'スプライト立ち絵', '背面のぼかし'], '図形'],
    ['en', ['Image', 'Video', 'Audio asset', 'VRM character', 'PSD character', 'Sprite character', 'Backdrop blur'], 'Shape'],
  ] as const) {
    locale.current = language;
    const body = render(Form, { props: { project, submit } }).body.replace(/<[^>]*>/g, '');
    for (const text of ['残す字幕の内容', '表示する読み上げ文', '残すテキスト', ...labels]) expect(body).toContain(text);
    expect(body.split(`${shape}（0${language === 'ja' ? '秒' : ' s'}）`).length - 1).toBe(1);
    if (language === 'en') {
      expect(body).not.toMatch(/編集対象|まとめるクリップ|シーンを作成|移動方向|秒/);
      expect(body).toContain(Form === SceneForm ? 'Create scene' : Form === GroupForm ? 'Create group' : 'Review changes for the selected items');
    }
    expect(JSON.stringify(project)).toBe(before);
    expect(submit).not.toHaveBeenCalled();
  }
});
