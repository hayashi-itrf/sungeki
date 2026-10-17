const labels: Record<string, [string, string]> = {
  'speech.inspect_originals': ['モデルと実行環境の原本を確認中', 'Checking model and runtime originals'],
  'speech.start_engine': ['音声エンジンを起動・読み込み中', 'Starting and loading the voice engine'],
  'speech.recheck_originals': ['起動後の原本を再確認中', 'Rechecking originals after startup'],
  'speech.synthesize': ['音声を生成中', 'Generating speech'],
  'speech.verify_output': ['生成音声と原本を検査中', 'Verifying generated audio and originals'],
  'speech.stop_engine': ['音声エンジンを停止中', 'Stopping the voice engine'],
  'speech.save_audio': ['音声と口パク情報を保存・反映中', 'Saving and applying audio and lip sync'],
};
export function speechProgress(stage: string, language: string): string {
  return labels[stage]?.[language === 'en' ? 1 : 0] ?? stage;
}
