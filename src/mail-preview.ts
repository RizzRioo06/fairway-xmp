import bridge from './mail-preview-bridge.js?raw';

// Keep this hash and the app CSP synchronized with mail-preview-bridge.js.
const bridgeHash = 'u6vELvbsWWvZ6H4jiN7Ty3WG+n3VWsN2wgOdQnjO3LE=';
const policy = `default-src 'none'; script-src 'sha256-${bridgeHash}'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'`;
const emailStyle = 'body{margin:16px 20px;font:14px/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;color:#22342a;overflow-wrap:anywhere}img{max-width:100%;height:auto}';

export function mailPreview(html: string, inlineImages?: Record<string, string>) {
  // Template contents stay inert while resolving embedded images. Remove email
  // scripts and metadata before adding our policy and interaction bridge.
  const template = document.createElement('template');
  const root = document.createElement('html');
  template.content.append(root);
  // The html context preserves head/body styles while remaining inside the
  // inert template document (images and other resources cannot load here).
  root.innerHTML = html;
  for (const element of root.querySelectorAll('script, meta, base')) element.remove();
  const images = new Map(Object.entries(inlineImages ?? {}));
  for (const image of root.querySelectorAll<HTMLImageElement>('img[src]')) {
    const source = image.getAttribute('src')?.trim() ?? '';
    if (!/^cid:/i.test(source)) continue;
    let cid = source.slice(4);
    try { cid = decodeURIComponent(cid); } catch { /* Keep malformed IDs literal. */ }
    const data = images.get(cid.replace(/^<|>$/g, ''));
    if (data && /^data:image\/(?:png|jpeg|gif|webp|avif|bmp|x-icon|vnd\.microsoft\.icon);base64,/i.test(data)) image.src = data;
  }
  const head = root.querySelector('head') ?? root.insertBefore(document.createElement('head'), root.firstChild);
  const body = root.querySelector('body') ?? root.appendChild(document.createElement('body'));
  const csp = document.createElement('meta');
  csp.httpEquiv = 'Content-Security-Policy';
  csp.content = policy;
  const style = document.createElement('style');
  style.textContent = emailStyle;
  head.prepend(csp, style);
  const script = document.createElement('script');
  script.textContent = bridge;
  body.append(script);
  return `<!doctype html>${root.outerHTML}`;
}
