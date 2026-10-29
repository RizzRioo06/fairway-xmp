import { existsSync } from 'node:fs';
import { createServer } from 'node:http';
import { loadEnvFile } from 'node:process';

if (existsSync('.env')) loadEnvFile('.env');
const host = process.env.HOST || '127.0.0.1';
const port = Number(process.env.PORT || 3000);
const server = createServer((request, response) => {
  response.writeHead(200, { 'content-type': 'application/json; charset=utf-8' });
  response.end(JSON.stringify(request.url === '/health' ? { status: 'ok' } : { message: 'Hello from TypeScript' }));
});
server.listen(port, host, () => {
  const address = server.address();
  if (address && typeof address === 'object') {
    const hostname = address.address.includes(':') ? `[${address.address}]` : address.address;
    console.log(`Node.js listening at http://${hostname}:${address.port}`);
  }
});
for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, () => server.close());
