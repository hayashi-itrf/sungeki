// Keep the entry independent of editor imports so load failures remain visible.
export {};
let english = false;
try { english = localStorage.getItem('sungeki.ui.locale') === 'en'; } catch { /* Use Japanese by default. */ }
document.documentElement.lang = english ? 'en' : 'ja';
document.dispatchEvent(new Event('next-startup-entry'));
const target = document.getElementById('app');
const status = document.getElementById('startup-status');
if (english && status) {
  const paragraphs = status.querySelectorAll('p');
  if (paragraphs[0]) paragraphs[0].textContent = 'Loading…';
  if (paragraphs[1]) paragraphs[1].textContent = 'If this screen remains, close and reopen the app.';
}

void import('./app/bootstrap').then(({ start }) => {
  if (!target) throw new Error(english ? 'The application mount element is missing.' : 'アプリの表示領域がありません');
  start(target);
  status?.remove();
  document.dispatchEvent(new Event('next-startup-mounted'));
}).catch((error: unknown) => {
  document.dispatchEvent(new Event('next-startup-failed'));
  console.error('Application startup failed', error);
  if (!status) return;
  status.setAttribute('role', 'alert');
  const heading = document.createElement('h1');
  heading.textContent = english ? 'Unable to start' : '画面を起動できませんでした';
  const message = document.createElement('p');
  message.textContent = english ? 'Close and reopen the app. If the problem persists, check the details below.' : 'アプリを閉じて再度開いてください。繰り返す場合は以下の内容を確認してください。';
  const details = document.createElement('pre');
  details.textContent = error instanceof Error ? error.message : String(error);
  details.style.whiteSpace = 'pre-wrap';
  status.replaceChildren(heading, message, details);
});
