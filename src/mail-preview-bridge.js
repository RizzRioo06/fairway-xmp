// This exact script is authorized by its CSP hash. Email scripts are not.
(() => {
  const send = message => parent.postMessage({ type: 'fairway-mail-preview', ...message }, '*');
  const follow = event => {
    const link = event.target.closest?.('a[href], area[href], a[xlink\\:href]');
    if (!link) return;
    event.preventDefault();
    if (!event.isTrusted || (event.type === 'auxclick' ? event.button !== 1 : event.button !== 0)) return;
    try {
      const url = new URL(link.getAttribute('href') ?? link.getAttribute('xlink:href') ?? '');
      if (['http:', 'https:'].includes(url.protocol) && !url.username && !url.password) send({ action: 'open', url: url.href });
    } catch { /* Relative links and other destinations stay in the preview. */ }
  };
  document.addEventListener('click', follow);
  document.addEventListener('auxclick', follow);
  document.addEventListener('keydown', event => {
    if (!event.isTrusted) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      send({ action: 'escape' });
    } else if (event.key === 'Tab') {
      const items = [...document.querySelectorAll('button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, [tabindex]')]
        .filter(element => element.tabIndex >= 0 && !element.closest('[inert]') && element.getClientRects().length > 0);
      if (!items.length || (event.shiftKey ? document.activeElement === items[0] : document.activeElement === items.at(-1))) {
        event.preventDefault();
        send({ action: 'tab', backward: event.shiftKey });
      }
    }
  });
})();
