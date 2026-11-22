// Run against an app directory. The checked-in output can be used without Node.
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';

const primitives = {
  string: ['String', 'string'],
  u32: ['u32', 'number'],
  f64: ['f64', 'number'],
  bool: ['bool', 'boolean'],
  void: ['()', 'void'],
};
const snake = (name) => name.replace(/[A-Z]/g, (c) => '_' + c.toLowerCase());
function identifier(name, pattern) {
  if (!pattern.test(name)) throw new Error('Invalid contract identifier: ' + name);
}
export function generate({ types, commands }) {
  function typeName(spec, language) {
    if (spec.endsWith('[]')) {
      const item = typeName(spec.slice(0, -2), language);
      return language === 0 ? 'Vec<' + item + '>' : 'Array<' + item + '>';
    }
    if (spec.endsWith('?')) {
      const item = typeName(spec.slice(0, -1), language);
      return language === 0 ? 'Option<' + item + '>' : item + ' | null';
    }
    if (Object.hasOwn(primitives, spec)) return primitives[spec][language];
    if (!Object.hasOwn(types, spec)) throw new Error('Unknown contract type: ' + spec);
    return spec;
  }
  const rust = [
    '// Generated from commands.json by generate-commands.mjs.',
    'use rustybox_sdk::{Commands, Result, View};',
    'use serde::{Deserialize, Serialize};',
    '',
  ];
  const ts = [
    '// Generated from commands.json.',
    "import type { RustyBoxClient, InvokeOptions } from './sdk/rustybox.js';",
    '',
  ];
  const js = [
    '// Generated from commands.json.',
    'export function bindCommands(client) {',
    '  return {',
  ];
  for (const [name, fields] of Object.entries(types)) {
    identifier(name, /^[A-Z][a-zA-Z0-9]*$/);
    rust.push(
      '#[derive(Clone, Debug, Deserialize, Serialize)]',
      '#[serde(rename_all = "camelCase", deny_unknown_fields)]',
      'pub struct ' + name + ' {'
    );
    ts.push('export interface ' + name + ' {');
    for (const [field, kind] of Object.entries(fields)) {
      identifier(field, /^[a-z][a-zA-Z0-9]*$/);
      rust.push('    pub ' + snake(field) + ': ' + typeName(kind, 0) + ',');
      ts.push('  ' + field + ': ' + typeName(kind, 1) + ';');
    }
    rust.push('}', '');
    ts.push('}', '');
  }
  rust.push("pub trait AppCommands: Sized + 'static {");
  ts.push('export interface AppCommands {');
  for (const [name, { input, output }] of Object.entries(commands)) {
    identifier(name, /^[a-z][a-zA-Z0-9]*$/);
    const optional = Object.hasOwn(types, input) && Object.keys(types[input]).length === 0;
    const rustInput = typeName(input, 0);
    const rustOutput = typeName(output, 0);
    const tsInput = typeName(input, 1);
    const tsOutput = typeName(output, 1);
    rust.push(
      `    fn ${snake(name)}(&mut self, view: &View, args: ${rustInput}) -> Result<${rustOutput}>;`
    );
    ts.push(
      `  ${name}(params${optional ? '?' : ''}: ${tsInput}, options?: InvokeOptions): Promise<${tsOutput}>;`
    );
    js.push(
      `    ${name}(params${optional ? ' = {}' : ''}, options) {`,
      `      return client.invoke('host.${name}', params, options);`,
      '    },'
    );
  }
  rust.push(
    '}',
    '',
    'pub fn register<S: AppCommands>() -> Commands<S> {',
    '    let mut commands = Commands::default();'
  );
  for (const name of Object.keys(commands))
    rust.push('    commands.register("host.' + name + '", S::' + snake(name) + ');');
  rust.push('    commands', '}');
  ts.push('}', 'export function bindCommands(client: RustyBoxClient): AppCommands;');
  js.push('  };', '}');
  return {
    'src/commands.rs': rust.join('\n') + '\n',
    'ui/commands.js': js.join('\n') + '\n',
    'ui/commands.d.ts': ts.join('\n') + '\n',
  };
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const app = resolve(process.argv[2] || '.');
  const files = generate(JSON.parse(readFileSync(join(app, 'commands.json'), 'utf8')));
  for (const [path, contents] of Object.entries(files)) {
    const target = join(app, path);
    if (process.argv.includes('--check')) {
      if (readFileSync(target, 'utf8').replaceAll('\r\n', '\n') !== contents)
        throw new Error('Outdated contract: ' + target);
    } else writeFileSync(target, contents);
  }
}
