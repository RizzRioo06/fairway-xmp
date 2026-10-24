import bridge from './mail-preview-bridge.js?raw';

// Keep this hash and the app CSP synchronized with mail-preview-bridge.js.
const bridgeHash = 'fICz093+jdO2OQO/C9nvit6WrgbBQ/mZcBpfhpjDOK8=';
const policy = `default-src 'none'; script-src 'sha256-${bridgeHash}'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'`;
const emailStyle = 'body{margin:16px 20px;font:14px/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;color:#22342a;overflow-wrap:anywhere}img{max-width:100%;height:auto}';
const placeholderStyle = '.fairway-image-placeholder[hidden]{display:none!important}.fairway-image-placeholder{display:inline-flex;flex-direction:column;align-items:flex-start;gap:6px;max-width:100%;box-sizing:border-box;padding:10px;border:1px dashed #a8b8ad;border-radius:5px;background:#f3f6f4;color:#53685b;font:12px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;overflow-wrap:anywhere}.fairway-image-placeholder button{padding:4px 8px;border:1px solid #a8b8ad;border-radius:4px;background:white;color:#24664f;font:inherit;cursor:pointer}.fairway-image-placeholder button:focus-visible{outline:2px solid #24664f;outline-offset:2px}.fairway-image-placeholder button:disabled{cursor:default;opacity:.6}';

export type RemoteImage = { id: string; url: string };
export type ImageState = { status: 'loading' | 'loaded' | 'error'; data?: string; error?: string };
export const remoteImageLimit = 16;

function remoteUrl(value: string) {
  try {
    const url = new URL(value.startsWith('//') ? `https:${value}` : value);
    if (['http:', 'https:'].includes(url.protocol) && !url.username && !url.password && url.href.length <= 8192 && !/[\u0000-\u001f\u007f]/.test(value)) return url.href;
  } catch { /* Relative URLs have no trustworthy base in an email. */ }
  return null;
}

export function prepareMailPreview(html: string, inlineImages?: Record<string, string>) {
  // Template contents stay inert while resolving embedded images. Remove email
  // scripts and metadata before adding our policy and interaction bridge.
  const template = document.createElement('template');
  const root = document.createElement('html');
  template.content.append(root);
  // The html context preserves head/body styles while remaining inside the
  // inert template document (images and other resources cannot load here).
  root.innerHTML = html;
  for (const element of root.querySelectorAll('script, meta, base')) element.remove();
  // Email-authored attributes must never impersonate our image controls.
  for (const element of root.querySelectorAll('*')) {
    for (const attribute of [...element.attributes]) {
      if (attribute.name.startsWith('data-fairway-')) element.removeAttribute(attribute.name);
    }
  }
  const images = new Map(Object.entries(inlineImages ?? {}));
  const remoteImages: RemoteImage[] = [];
  for (const image of root.querySelectorAll<HTMLImageElement>('img')) {
    const source = image.getAttribute('src')?.trim() ?? '';
    const candidates = image.getAttribute('srcset')?.split(',').map(item => item.trim().split(/\s+/)[0]) ?? [];
    image.removeAttribute('srcset');
    // Use the ordinary src fallback; srcset-only images use their first web
    // candidate. Never give responsive sources to the browser to fetch itself.
    const remote = remoteUrl(source) ?? (!source ? candidates.map(remoteUrl).find(Boolean) : null);
    if (remote) {
      image.removeAttribute('src');
      const placeholder = document.createElement('span');
      placeholder.className = 'fairway-image-placeholder';
      const label = document.createElement('span');
      label.textContent = image.alt || 'Remote image';
      const button = document.createElement('button');
      button.type = 'button';
      button.textContent = 'Load image';
      button.title = `Loads from ${new URL(remote).host}. This contacts the server and may reveal your IP address.`;
      const id = String(remoteImages.length);
      if (remoteImages.length < remoteImageLimit) {
        remoteImages.push({ id, url: remote });
        placeholder.setAttribute('data-fairway-image', id);
        // Keep the original image inert until our bridge receives raster data.
        const template = document.createElement('template');
        image.replaceWith(placeholder);
        template.content.append(image);
        placeholder.append(label, button, template);
      } else {
        label.textContent = 'Remote image limit reached (16 images). Open in browser to view more.';
        image.replaceWith(placeholder);
        placeholder.append(label);
      }
      continue;
    }
    if (!/^cid:/i.test(source)) continue;
    let cid = source.slice(4);
    try { cid = decodeURIComponent(cid); } catch { /* Keep malformed IDs literal. */ }
    const data = images.get(cid.replace(/^<|>$/g, ''));
    if (data && /^data:image\/(?:png|jpeg|gif|webp|avif|bmp|x-icon|vnd\.microsoft\.icon);base64,/i.test(data)) image.src = data;
  }
  for (const source of root.querySelectorAll('picture source')) source.remove();
  const head = root.querySelector('head') ?? root.insertBefore(document.createElement('head'), root.firstChild);
  const body = root.querySelector('body') ?? root.appendChild(document.createElement('body'));
  const csp = document.createElement('meta');
  csp.httpEquiv = 'Content-Security-Policy';
  csp.content = policy;
  const style = document.createElement('style');
  style.textContent = emailStyle + placeholderStyle;
  head.prepend(csp, style);
  const script = document.createElement('script');
  script.textContent = bridge;
  body.append(script);
  return { source: `<!doctype html>${root.outerHTML}`, images: remoteImages };
}
