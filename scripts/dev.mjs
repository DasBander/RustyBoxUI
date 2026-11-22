// Watch a generated project. The live-reload client exists only in the development web root.
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { watch, watchFile, unwatchFile } from 'node:fs';
import { readFile, readdir, writeFile, cp, mkdir, rm } from 'node:fs/promises';
import { join, resolve, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../', import.meta.url));
const live = join(root, 'target', 'live'),
  web = join(live, 'starter-ui');
let iconPath, externalIcon;
let child,
  stopping = false,
  busy = false,
  timer,
  revision = 0,
  pending = new Set();
async function run(command, args) {
  const p = spawn(command, args, { cwd: root, stdio: 'inherit', windowsHide: true });
  const [code] = await once(p, 'exit');
  if (code) throw Error(command + ' exited with ' + code);
}
async function stopHost() {
  if (child && child.exitCode === null && child.signalCode === null) {
    const end = once(child, 'exit');
    child.kill();
    await end;
  }
  child = null;
}
const client = async function () {
  let version = null;
  async function poll() {
    try {
      const state = await fetch('./__rustybox_revision.json', { cache: 'no-store' }).then((r) =>
        r.json()
      );
      if (version !== null && state.revision !== version) {
        if (state.css) {
          for (const link of document.querySelectorAll('link[rel=stylesheet]')) {
            const url = new URL(link.href);
            url.searchParams.set('rbdev', state.revision);
            link.href = url.href;
          }
        } else {
          location.reload();
          return;
        }
      }
      version = state.revision;
    } catch {
      /* The build may be replacing the development directory. */
    }
    setTimeout(poll, 250);
  }
  poll();
};
async function inject() {
  const html = (await readFile(join(root, 'ui', 'index.html'), 'utf8')).replace(
    /connect-src\s+'none'/gi,
    "connect-src 'self'"
  );
  const tag = '<script type="module" src="./__rustybox_live.js"></script>';
  await writeFile(
    join(web, 'index.html'),
    html.match(/<\/head>/i) ? html.replace(/<\/head>/i, tag + '</head>') : html + tag
  );
  await writeFile(join(web, '__rustybox_live.js'), '(' + client.toString() + ')();');
}
function schedule(file) {
  pending.add(file);
  clearTimeout(timer);
  timer = setTimeout(refresh, 150);
}
async function build() {
  const manifest = JSON.parse(
    (await readFile(join(root, 'rustybox.json'), 'utf8')).replace(/^\uFEFF/, '')
  );
  iconPath = manifest.icon ? resolve(root, manifest.icon).toLowerCase() : undefined;
  if (externalIcon) unwatchFile(externalIcon);
  externalIcon =
    iconPath && !iconPath.startsWith(resolve(root).toLowerCase() + sep) ? iconPath : undefined;
  if (externalIcon) watchFile(externalIcon, { interval: 450 }, () => schedule(externalIcon));
  await stopHost();
  await run(process.execPath, ['generate-commands.mjs', '.']);
  await run('powershell.exe', [
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-File',
    'build.ps1',
    '-Development',
  ]);
  await inject();
  await writeFile(
    join(web, '__rustybox_revision.json'),
    JSON.stringify({ revision: ++revision, css: false })
  );
  if (!stopping) {
    const hosts = (await readdir(live, { withFileTypes: true })).filter(
      (file) => file.isFile() && /\.exe$/i.test(file.name)
    );
    if (hosts.length !== 1) throw Error('Expected a single executable in ' + live);
    child = spawn(join(live, hosts[0].name), [], { cwd: live, stdio: 'inherit' });
    child.on('error', (error) => console.error('Cannot launch development host:', error.message));
  }
}
async function refresh() {
  clearTimeout(timer);
  if (busy || stopping) return;
  busy = true;
  const changes = pending;
  pending = new Set();
  try {
    if (
      [...changes].some((p) => !p.startsWith('ui/') || resolve(root, p).toLowerCase() === iconPath)
    )
      await build();
    else {
      for (const name of changes) {
        const source = resolve(root, name),
          destination = resolve(web, relative(join(root, 'ui'), source));
        if (!destination.startsWith(web + sep)) continue;
        try {
          await mkdir(resolve(destination, '..'), { recursive: true });
          await cp(source, destination, { recursive: true, force: true });
        } catch (e) {
          if (e.code !== 'ENOENT') throw e;
          await rm(destination, { recursive: true, force: true });
        }
      }
      await inject();
      await writeFile(
        join(web, '__rustybox_revision.json'),
        JSON.stringify({ revision: ++revision, css: [...changes].every((p) => p.endsWith('.css')) })
      );
    }
  } catch (e) {
    console.error(e.message);
  } finally {
    busy = false;
    if (pending.size) timer = setTimeout(refresh, 120);
  }
}
await build();
const watcher = watch(root, { recursive: true }, (_, file) => {
  const name = String(file).replaceAll('\\', '/');
  if (name === 'src/commands.rs' || name === 'ui/commands.js' || name === 'ui/commands.d.ts')
    return;
  if (
    !/^(ui\/|src\/|commands\.json$|Cargo\.toml$|build\.rs$|rustybox\.json$)/.test(name) &&
    resolve(root, name).toLowerCase() !== iconPath
  )
    return;
  schedule(name);
});
for (const sig of ['SIGINT', 'SIGTERM'])
  process.on(sig, async () => {
    stopping = true;
    watcher.close();
    if (externalIcon) unwatchFile(externalIcon);
    clearTimeout(timer);
    await stopHost();
    process.exit();
  });
console.log('Watching UI, Rust, commands and app branding. Press Ctrl+C to stop.');
