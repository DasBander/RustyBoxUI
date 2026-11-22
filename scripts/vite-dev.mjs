// Keep top-level navigation on the native app origin. Only this development page
// loads modules from the project's loopback Vite server, including its HMR client.
import { createRequire } from 'node:module';
import { randomBytes } from 'node:crypto';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export async function startVite(project, updateEntry) {
  const root = join(project, 'frontend');
  const require = createRequire(join(root, 'package.json'));
  const { createServer } = await import(pathToFileURL(require.resolve('vite')).href);
  const server = await createServer({
    root,
    base: '/',
    plugins: [
      {
        name: 'rustybox-entry',
        async handleHotUpdate(context) {
          if (!updateEntry || resolve(context.file) !== join(root, 'index.html')) return;
          await context.read();
          await updateEntry();
          context.server.ws.send({ type: 'full-reload', path: '*' });
          return [];
        },
      },
    ],
    server: {
      host: '127.0.0.1',
      port: 0,
      open: false,
      cors: { origin: ['https://rustybox.example', 'https://rustybox.local'] },
      fs: { strict: true, allow: [project] },
    },
  });
  try {
    await server.listen();
  } catch (error) {
    await server.close();
    throw error;
  }
  const origin = 'http://127.0.0.1:' + server.httpServer.address().port;
  return {
    close: () => server.close(),
    async html() {
      const response = await fetch(origin + '/', { headers: { Accept: 'text/html' } });
      if (!response.ok) throw new Error('Vite entry returned ' + response.status);
      const nonce = randomBytes(18).toString('base64');
      const policy =
        "default-src 'self' " +
        origin +
        "; script-src 'self' " +
        origin +
        " 'nonce-" +
        nonce +
        "'; style-src 'self' 'unsafe-inline' " +
        origin +
        "; connect-src 'self' " +
        origin +
        ' ' +
        origin.replace('http:', 'ws:') +
        "; img-src 'self' data: blob: " +
        origin +
        "; font-src 'self' " +
        origin +
        '; base-uri ' +
        origin +
        "; object-src 'none'; frame-src 'none'";
      const head =
        '<meta http-equiv="Content-Security-Policy" content="' +
        policy +
        '"><base href="' +
        origin +
        '/">';
      const inspector =
        '<script type="module" nonce="' +
        nonce +
        '">import(location.origin + "/sdk/inspector.js")</script>';
      return (await response.text())
        .replace(/<meta\b[^>]*http-equiv\s*=\s*["']content-security-policy["'][^>]*>/gi, '')
        .replace(/<base\b[^>]*>/gi, '')
        .replace(/<script\b/gi, '<script nonce="' + nonce + '"')
        .replace(/<head\b[^>]*>/i, (match) => match + head)
        .replace(/<\/body>/i, inspector + '</body>');
    },
  };
}
