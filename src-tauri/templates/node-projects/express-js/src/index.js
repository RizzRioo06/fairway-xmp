import { existsSync } from 'node:fs';
import { loadEnvFile } from 'node:process';
import { app } from './app.js';

if (existsSync('.env')) loadEnvFile('.env');
const host = process.env.HOST || '127.0.0.1';
const port = Number(process.env.PORT || 3000);
const server = app.listen(port, host, () => console.log(`Express listening at http://${host}:${port}`));
for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, () => server.close());
