import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { resolve, extname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
const shared = fileURLToPath(new URL('../ui/', import.meta.url));
const talus = process.argv.includes('--talus');
const talusUi = fileURLToPath(new URL('../examples/showcase/talus/ui/', import.meta.url));
const steam = process.argv.includes('--steam');
const steamUi = fileURLToPath(new URL('../examples/steam/ui/', import.meta.url));
const port = Number(process.env.PORT || 4173);
const types = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.json': 'application/json',
  '.webp': 'image/webp',
  '.woff2': 'font/woff2',
  '.md': 'text/plain; charset=utf-8',
};
createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    const useShared =
      ['/catalog.js', '/theme.css'].includes(pathname) ||
      pathname.startsWith('/sdk/') ||
      pathname.startsWith('/art/');
    const root = talus && !useShared ? talusUi : steam && !useShared ? steamUi : shared;
    const path = resolve(root, '.' + (pathname === '/' ? '/index.html' : pathname));
    if (!path.startsWith(resolve(root) + sep)) {
      response.writeHead(403);
      response.end();
      return;
    }
    if (!(await stat(path)).isFile()) throw new Error('not a file');
    response.writeHead(200, {
      'Content-Type': types[extname(path)] || 'application/octet-stream',
      'Cache-Control': 'no-store',
      'X-Content-Type-Options': 'nosniff',
    });
    response.end(await readFile(path));
  } catch {
    response.writeHead(404);
    response.end('Not found');
  }
}).listen(port, '127.0.0.1', () =>
  console.log(`RustyBox browser preview: http://127.0.0.1:${port}`)
);
