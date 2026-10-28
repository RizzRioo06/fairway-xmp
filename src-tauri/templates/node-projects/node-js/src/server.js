import { createServer as httpServer } from 'node:http';

export function createServer() {
  return httpServer((request, response) => {
    const healthy = request.url === '/health';
    response.writeHead(200, { 'content-type': 'application/json; charset=utf-8' });
    response.end(JSON.stringify(healthy ? { status: 'ok' } : { message: 'Hello from Node.js' }));
  });
}
