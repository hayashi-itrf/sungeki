// Preserve the exact server value for selection and fingerprints; localize labels only.
const labels: Record<string, string> = {
  auto: '自動判定', japanese: '日本語', english: '英語', chinese: '中国語',
  korean: '韓国語', german: 'ドイツ語', french: 'フランス語', russian: 'ロシア語',
  portuguese: 'ポルトガル語', spanish: 'スペイン語', italian: 'イタリア語',
};
const english: Record<string, string> = {
  auto: 'Automatic', japanese: 'Japanese', english: 'English', chinese: 'Chinese',
  korean: 'Korean', german: 'German', french: 'French', russian: 'Russian',
  portuguese: 'Portuguese', spanish: 'Spanish', italian: 'Italian',
};
export function qwenLanguageLabel(language: string, locale: 'ja' | 'en' = 'ja'): string {
  return (locale === 'en' ? english : labels)[language.toLowerCase()] ?? language;
}
