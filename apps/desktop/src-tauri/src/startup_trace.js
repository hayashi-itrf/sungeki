// Injected only when SUNGEKI_STARTUP_TRACE=1; never changes window visibility.
;(() => {
  let remaining = 24;
  const record = (stage) => {
    if (remaining-- <= 0) return;
    const bounds = document.getElementById('app')?.getBoundingClientRect();
    void window.__TAURI_INTERNALS__.invoke('trace_startup', {
      stage, hidden: document.hidden,
      nodes: document.getElementById('app')?.childElementCount ?? 0,
      viewportWidth: window.innerWidth, viewportHeight: window.innerHeight,
      appWidth: Math.round(bounds?.width ?? 0), appHeight: Math.round(bounds?.height ?? 0),
      pixelRatio: window.devicePixelRatio,
    }).catch(() => {});
  };
  // Tauri installs invoke after this initialization script. Register listeners now,
  // but do not call the bridge until page scripts / DOMContentLoaded run.
  document.addEventListener('DOMContentLoaded', () => record('dom_ready'), { once: true });
  document.addEventListener('visibilitychange', () => record('visibility'));
  window.addEventListener('resize', () => record('resize'));
  for (const stage of ['entry', 'mounted', 'failed']) {
    document.addEventListener(`next-startup-${stage}`, () => {
      record(stage);
      if (stage === 'mounted') {
        queueMicrotask(() => record('layout'));
        requestAnimationFrame(() => record('frame'));
      }
    }, { once: true });
  }
})();
