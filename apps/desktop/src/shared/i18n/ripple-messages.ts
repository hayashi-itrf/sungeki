export const rippleEnglish = {
  rippleTitle: 'Move clips from a time boundary',
  rippleHint: 'Choose clips and a boundary. Selected clips starting at or after that time move together. Clips ending before it stay in place.',
  rippleBoundary: 'Boundary (seconds)',
  rippleSelection: 'Use timeline selection',
  rippleAll: 'Select all clips',
  rippleClear: 'Clear selection',
  rippleCount: '{count} clips selected',
  rippleConstraint: 'A selected clip crossing the boundary prevents the edit. Clips are not split. If a moved clip would leave its scene, adjust the scene first.',
  rippleDescription: 'Of {count} selected clips, move those starting at or after {boundary} s by {distance} s {direction}.',
} as const;
export const rippleJapanese: Record<keyof typeof rippleEnglish, string> = {
  rippleTitle: '指定時刻以降のクリップをまとめて移動',
  rippleHint: '対象のクリップと境界時刻を指定します。選んだ中で、境界時刻以降に始まるクリップを移動します。境界までに終わるクリップはそのままです。',
  rippleBoundary: '境界時刻（秒）',
  rippleSelection: 'タイムラインの選択を使う',
  rippleAll: '全クリップを選択',
  rippleClear: '選択を解除',
  rippleCount: '選択中：{count}クリップ',
  rippleConstraint: '選んだクリップが境界をまたぐ場合は適用できません。クリップは分割しません。移動後に所属シーンからはみ出す場合は、先にシーンの範囲を調整してください。',
  rippleDescription: '選択した{count}クリップのうち、{boundary}秒以降に始まるものを{distance}秒{direction}移動',
};
