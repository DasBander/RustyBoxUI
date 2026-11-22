import { execFileSync } from 'node:child_process';
import { copyFileSync, cpSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// The two-argument form also accepts metadata saved by earlier packaging tools.
const [first, second] = process.argv.slice(2);
if (!first)
  throw new Error('Usage: node scripts/notices.mjs [cargo-metadata.json] output-directory');
const root = fileURLToPath(new URL('../', import.meta.url));
const output = resolve(second ?? first);
const raw = second
  ? readFileSync(first, 'utf8')
  : execFileSync('cargo', ['metadata', '--format-version', '1', '--locked'], {
      cwd: root,
      encoding: 'utf8',
      maxBuffer: 32 * 1024 * 1024,
      windowsHide: true,
    });
const metadata = JSON.parse(raw.replace(/^\uFEFF/, ''));
const workspace = new Set(metadata.workspace_members);
const packages = metadata.packages
  .filter((p) => !workspace.has(p.id))
  .sort((a, b) => a.name.localeCompare(b.name));
const lines = [
  'RustyBoxUI third-party dependencies',
  '',
  'These are the packages in Cargo.lock, including target-specific and build dependencies.',
  'The WebView2 Rust bindings and the Microsoft SDK loader notices are listed in README.md.',
  'The Evergreen Runtime is installed separately.',
  '',
];

mkdirSync(output, { recursive: true });
cpSync(join(root, 'licenses'), output, { recursive: true });
for (const pkg of packages) {
  lines.push(
    pkg.name + ' ' + pkg.version,
    'License: ' + (pkg.license || 'See package source'),
    'Source: ' + (pkg.repository || pkg.source || ''),
    ''
  );
  const source = dirname(pkg.manifest_path);
  for (const file of readdirSync(source, { withFileTypes: true })) {
    if (!file.isFile() || !/^(license|copying|notice)([-.]|$)/i.test(file.name)) continue;
    const directory = join(output, pkg.name + '-' + pkg.version);
    mkdirSync(directory, { recursive: true });
    copyFileSync(join(source, file.name), join(directory, file.name));
  }
}
writeFileSync(join(output, 'DEPENDENCIES.txt'), lines.join('\n') + '\n');
console.log('Collected notices for ' + packages.length + ' locked dependencies.');
