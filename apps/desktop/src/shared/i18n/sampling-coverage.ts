export const coverageEnglish = {
    title: 'Where images were not sampled', omitted: 'Omitted candidate times (seconds)',
    spans: 'Frame ranges without samples', hint: 'Frame numbers start at 0. Both endpoints below are included. These ranges belong to this report; editing the project may change their timing.',
    caution: 'Selecting or displaying an image does not mark it as visually inspected. Increase the image limit or request omitted times in a new review.',
    more: 'Show more',
  };
export const coverageJapanese: Record<keyof typeof coverageEnglish, string> = {
    title: '画像を抜き出していない箇所', omitted: '省略された候補の時刻（秒）',
    spans: '画像を抜き出していないフレーム範囲', hint: 'フレーム番号は0から始まり、両端を含みます。この結果を作成した時点の範囲です。作品を編集すると時刻が変わる場合があります。',
    caution: '画像を選択・表示しても目視確認済みにはなりません。枚数の上限を増やすか、省略された時刻を指定して新しく確認してください。',
    more: '続きを表示',
  };
