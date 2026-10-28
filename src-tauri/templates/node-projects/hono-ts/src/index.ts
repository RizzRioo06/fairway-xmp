import { existsSync } from 'node:fs';
import { loadEnvFile } from 'node:process';
import { serve } from '@hono/node-server';
import { Hono } from 'hono';

if (existsSync('.env')) loadEnvFile('.env');
const app = new Hono();
app.get('/', c => c.json({ message: 'Hello from Hono' }));
app.get('/health', c => c.json({ status: 'ok' }));
const hostname = process.env.HOST || '127.0.0.1';
const port = Number(process.env.PORT || 3000);
const server = serve({ fetch: app.fetch, hostname, port }, info => console.log(`Hono listening at http://${hostname}:${info.port}`));
for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, () => server.close());
