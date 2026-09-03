/**
 * Siegu landing — minimal zero-dependency static server.
 *
 * Usage:
 *   node scripts/serve.mjs            # serve ./ on http://localhost:4174
 *   PORT=8080 node scripts/serve.mjs  # custom port
 *   HOST=0.0.0.0 node scripts/serve.mjs
 */
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { extname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { dirname } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..', 'public');
const PORT = Number(process.env.PORT || 4174);
const HOST = process.env.HOST || '127.0.0.1';

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.txt': 'text/plain; charset=utf-8',
  '.md': 'text/markdown; charset=utf-8',
};

function safeResolve(urlPath) {
  const clean = decodeURIComponent(urlPath.split('?')[0]);
  const file = normalize(clean).replace(/^(\.\.[/\\])+/, '');
  return join(ROOT, file);
}

const server = createServer(async (req, res) => {
  const urlPath = (req.url || '/').split('?')[0];
  let filePath = safeResolve(urlPath);

  try {
    let st = await stat(filePath);
    if (st.isDirectory()) {
      filePath = join(filePath, 'index.html');
      st = await stat(filePath);
    }
    if (!st.isFile()) throw new Error('not a file');
    const ext = extname(filePath).toLowerCase();
    const body = await readFile(filePath);
    res.writeHead(200, {
      'Content-Type': MIME[ext] || 'application/octet-stream',
      'Cache-Control': ext === '.html' ? 'no-cache' : 'no-cache',
    });
    res.end(body);
  } catch {
    res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' });
    res.end('404 Not Found');
  }
});

server.listen(PORT, HOST, () => {
  console.log(`\n  ✳  siegu landing running at http://${HOST}:${PORT}/\n`);
});
