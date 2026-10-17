// Keep the entry independent of editor imports so load failures remain visible.
export {};
document.dispatchEvent(new Event('next-startup-entry'));
const target = document.getElementById('app');
const status = document.getElementById('startup-status');

void import('./app/bootstrap').then(({ start }) => {
  if (!target) throw new Error('アプリの表示領域がありません');
  start(target);
  status?.remove();
  document.dispatchEvent(new Event('next-startup-mounted'));
}).catch((error: unknown) => {
  document.dispatchEvent(new Event('next-startup-failed'));
  console.error('Application startup failed', error);
  if (!status) return;
  status.setAttribute('role', 'alert');
  const heading = document.createElement('h1');
  heading.textContent = '画面を起動できませんでした / Unable to start';
  const message = document.createElement('p');
  message.textContent = 'アプリを閉じて再度開いてください。繰り返す場合は以下の内容を確認してください。';
  const details = document.createElement('pre');
  details.textContent = error instanceof Error ? error.message : String(error);
  details.style.whiteSpace = 'pre-wrap';
  status.replaceChildren(heading, message, details);
});
