export const speechStartJapanese = {
  speechStartWaiting: '音声合成の開始要求を確認しています。',
  speechStartElapsed: '今回の応答待ち：{seconds} 秒',
  speechStartPreparationHint: 'モデル原本や保存先の確認には数分かかる場合があります。この段階の取り消しにはまだ対応していません。開始応答後は合成履歴で状態を確認・取り消しできます。',
  speechStartUncertain: '開始結果を確認できていません。同じ要求を再確認すると、重複して生成せず状態を取得できます。',
  speechStartRetry: '同じ要求を再確認',
};
export const speechStartEnglish: Record<keyof typeof speechStartJapanese, string> = {
  speechStartWaiting: 'Waiting for the speech synthesis start response.',
  speechStartElapsed: 'Waiting for this response: {seconds} seconds',
  speechStartPreparationHint: 'Checking model originals and storage may take several minutes. Cancellation during this preparation is not yet supported. After the start response, use synthesis history to check or cancel the job.',
  speechStartUncertain: 'The start result is unknown. Check the same request again to retrieve its state without generating duplicate audio.',
  speechStartRetry: 'Check the same request again',
};
