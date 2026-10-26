import { readFile, stat } from 'node:fs/promises';
import { resolve, extname, sep } from 'node:path';
import { randomBytes, timingSafeEqual } from 'node:crypto';

// Loopback is reachable by other OS users. A per-run secret grants browser access.
const accessToken = randomBytes(32).toString('base64url');
export const browserUrl = port => `http://127.0.0.1:${port}/?token=${accessToken}`;
const sameToken = value => typeof value === 'string' && value.length === accessToken.length && /^[A-Za-z0-9_-]+$/.test(value)
  && timingSafeEqual(Buffer.from(value), Buffer.from(accessToken));

export function allowedRequest(req, res, port) {
  const hosts = new Set([`127.0.0.1:${port}`, `localhost:${port}`]);
  const origin = req.headers.origin;
  const site = req.headers['sec-fetch-site'];
  const valid = hosts.has(req.headers.host)
    && (!origin || [...hosts].some(host => origin === `http://${host}`))
    && site !== 'cross-site'
    && (req.method !== 'POST' || req.headers['content-type']?.split(';')[0].trim() === 'application/json');
  if (!valid) {
    res.writeHead(403, { 'content-type': 'application/json', 'cache-control': 'no-store' });
    res.end(JSON.stringify({ error: 'Open the data browser using its local URL.' }));
  }
  if (!valid) return false;
  const cookieName = `fairway_browser_access_${port}`;
  const url = new URL(req.url ?? '/', `http://127.0.0.1:${port}`);
  if (req.method === 'GET' && url.pathname === '/' && sameToken(url.searchParams.get('token'))) {
    res.writeHead(303, { location: '/', 'cache-control': 'no-store', 'referrer-policy': 'no-referrer',
      'set-cookie': `${cookieName}=${accessToken}; HttpOnly; SameSite=Strict; Path=/` });
    res.end();
    return false;
  }
  const cookie = req.headers.cookie?.split(';').map(part => part.trim()).find(part => part.startsWith(`${cookieName}=`));
  if (!sameToken(cookie?.slice(cookieName.length + 1))) {
    res.writeHead(401, { 'content-type': 'application/json', 'cache-control': 'no-store' });
    res.end(JSON.stringify({ error: 'Open the access URL printed by fairway data-browser.' }));
    return false;
  }
  return true;
}

const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8', '.png': 'image/png', '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon', '.woff2': 'font/woff2' };

export async function serveStatic(req, res, url) {
  if (!['GET', 'HEAD'].includes(req.method) || url.pathname.startsWith('/api/')) return false;
  const root = resolve(process.env.FAIRWAY_DATA_BROWSER_STATIC);
  let pathname;
  try { pathname = decodeURIComponent(url.pathname); } catch { return false; }
  const file = resolve(root, `.${pathname === '/' ? '/index.html' : pathname}`);
  if (!file.startsWith(root + sep) || pathname.includes('\0')) return false;
  let info;
  try { info = await stat(file); } catch { return false; }
  if (!info.isFile()) return false;
  const body = await readFile(file);
  res.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream',
    'content-length': body.length, 'cache-control': 'no-cache', 'x-content-type-options': 'nosniff',
    'referrer-policy': 'no-referrer', 'content-security-policy': "frame-ancestors 'none'" });
  res.end(req.method === 'HEAD' ? undefined : body);
  return true;
}
