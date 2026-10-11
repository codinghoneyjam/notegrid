// Bundles the e2e harness against the real package sources and serves the wasm.
//
// Two bundles land in e2e/harness/:
//   harness.js  the page under test (ESM, imported by harness.html)
//   worklet.js  the AudioWorklet processor (IIFE, loaded by audioWorklet.addModule)
//
// The spec loads one JS file plus one wasm file, which keeps the Playwright fixture free
// of bundler configuration and means a change in packages/persistence or packages/app
// shows up here instead of leaving a green spec behind.
//
//   node build.mjs              bundle, then serve until interrupted (used by CI/webServer)
//   node build.mjs --build-only bundle and exit

import { build } from 'esbuild';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, isAbsolute, join, normalize, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = normalize(fileURLToPath(new URL('.', import.meta.url)));
const ROOT = normalize(join(HERE, '..'));

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json; charset=utf-8',
};

// The workspace packages are TypeScript; point them at the sources so the bundle
// needs no per-package build step.
const alias = {
  '@ng/persistence': join(ROOT, 'packages/persistence/src/index.ts'),
  '@ng/app': join(ROOT, 'packages/app/src/index.ts'),
  '@ng/document': join(ROOT, 'packages/document/src/index.ts'),
  '@ng/abi': join(ROOT, 'packages/abi/src/index.ts'),
  '@ng/playback': join(ROOT, 'packages/playback/src/index.ts'),
  '@ng/editing': join(ROOT, 'packages/editing/src/index.ts'),
};

// `loadNg()` with no argument reads the wasm off disk, which a browser cannot do.
// The harness never calls that path -- it passes bytes it fetched itself -- so the
// dynamic import is replaced rather than shimmed at runtime.
const dropNodeFs = {
  name: 'drop-node-fs',
  setup(pluginBuild) {
    pluginBuild.onResolve({ filter: /^node:fs\/promises$/ }, () => ({ path: 'node-fs', namespace: 'drop-node-fs' }));
    pluginBuild.onLoad({ filter: /.*/, namespace: 'drop-node-fs' }, () => ({
      contents: 'export const readFile = () => { throw new Error("loadNg() without bytes is Node-only"); };',
      loader: 'js',
    }));
  },
};

await build({
  entryPoints: [join(HERE, 'harness', 'harness.ts')],
  outfile: join(HERE, 'harness', 'harness.js'),
  bundle: true,
  format: 'esm',
  target: 'es2022',
  logLevel: 'warning',
  alias,
  plugins: [dropNodeFs],
});

// The worklet is its own realm: audioWorklet.addModule evaluates it as a
// self-contained script in the AudioWorkletGlobalScope, where no module
// loader exists. IIFE keeps the bundle free of import statements.
await build({
  entryPoints: [join(ROOT, 'packages/app/src/worklet/processor.ts')],
  outfile: join(HERE, 'harness', 'worklet.js'),
  bundle: true,
  format: 'iife',
  target: 'es2022',
  logLevel: 'warning',
  alias,
  plugins: [dropNodeFs],
});

if (process.argv.includes('--build-only')) {
  process.exit(0);
}

const PORT = Number(process.env.E2E_PORT ?? 4318);

const server = createServer((req, res) => {
  const url = (req.url ?? '/').split('?')[0];
  const file =
    url === '/ng-wasm.wasm'
      ? join(ROOT, 'packages/abi/dist/ng-wasm.wasm')
      : join(HERE, 'harness', url === '/' ? 'harness.html' : url.replace(/^\//, ''));

  // Only the harness directory and the built wasm are reachable. `resolve` is used
  // rather than a prefix test because a prefix test on Windows rejects its own files
  // whenever the drive letter or separator differs from the one it was built with.
  const rel = relative(HERE, file);
  const inside = rel !== '' && !rel.startsWith('..') && !isAbsolute(rel);
  if (!inside && !rel.endsWith(join('packages', 'abi', 'dist', 'ng-wasm.wasm'))) {
    process.stdout.write(`403 ${url} -> ${file}\n`);
    res.writeHead(403).end('forbidden');
    return;
  }

  readFile(file)
    .then((bytes) => {
      res.writeHead(200, {
        'content-type': TYPES[extname(file)] ?? 'application/octet-stream',
        'cache-control': 'no-store',
      });
      res.end(bytes);
    })
    .catch(() => {
      res.writeHead(404).end(`not found: ${url}`);
    });
});

server.listen(PORT, '127.0.0.1', () => {
  process.stdout.write(`e2e harness on http://127.0.0.1:${PORT}/\n`);
});
