// This exact script is authorized by its CSP hash. Email scripts are not.
(() => {
  const send = message => parent.postMessage({ type: 'fairway-mail-preview', ...message }, '*');
  const placeholders = new Map([...document.querySelectorAll('[data-fairway-image]')].map(element => [element.getAttribute('data-fairway-image'), element]));
  document.addEventListener('click', event => {
    const button = event.target.closest?.('[data-fairway-image] button');
    if (!button) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (event.isTrusted && !button.disabled) send({ action: 'image', id: button.closest('[data-fairway-image]').getAttribute('data-fairway-image') });
  }, true);
  window.addEventListener('message', event => {
    if (event.source !== parent || event.data?.type !== 'fairway-mail-images') return;
    for (const [id, placeholder] of placeholders) {
      const state = event.data.images?.[id];
      const button = placeholder.querySelector('button');
      const image = placeholder.nextElementSibling;
      if (state?.status === 'loaded' && image?.getAttribute('data-fairway-loaded') === id && image.getAttribute('src') === state.data) continue;
      if (image?.getAttribute('data-fairway-loaded') === id) image.remove();
      placeholder.hidden = false;
      if (state?.status === 'loaded' && /^data:image\/(?:png|jpeg|gif|webp|avif|bmp|x-icon|vnd\.microsoft\.icon);base64,/i.test(state.data)) {
        const img = placeholder.querySelector('template').content.firstElementChild.cloneNode(true);
        img.removeAttribute('loading');
        img.setAttribute('data-fairway-loaded', id);
        img.addEventListener('error', () => send({ action: 'image-error', id }));
        img.src = state.data;
        placeholder.after(img);
        const focused = document.activeElement === button;
        placeholder.hidden = true;
        if (focused) {
          img.tabIndex = 0;
          img.focus({ preventScroll: true });
        }
      }
      button.hidden = state?.status !== 'error';
      button.disabled = state?.status !== 'error';
      if (!button.dataset.originalTitle) button.dataset.originalTitle = button.title;
      button.title = state?.error || button.dataset.originalTitle;
    }
  });
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
  send({ action: 'ready' });
})();
