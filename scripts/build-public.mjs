// Standalone SDK build. Works from the exported package as well as the workspace.
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, appendFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { homedir } from 'node:os';
import { engineRoot, rustTool } from './rust-tools.mjs';

const wasmOnly = process.argv.includes('--wasm-only');
if (process.argv.includes('--wasm') || wasmOnly) {
  const cargoHome = process.env.CARGO_HOME || join(homedir(), '.cargo');
  const flags = [...(process.env.CARGO_ENCODED_RUSTFLAGS?.split('\x1f')
    ?? process.env.RUSTFLAGS?.match(/\S+/g) ?? []),
    `--remap-path-prefix=${cargoHome}=/cargo`, `--remap-path-prefix=${engineRoot}=/kmeow`,
    `--remap-path-prefix=${process.env.CARGO_TARGET_DIR || join(engineRoot, 'target')}=/target`];
  const result = spawnSync(rustTool('cargo'), ['build', ...(process.argv.includes('--offline') ? ['--offline'] : []), '--locked', '--release',
    '--target', 'wasm32-unknown-unknown', '-p', 'khmer-engine', '--message-format=json-render-diagnostics'], {
    cwd: engineRoot, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
    env: { ...process.env, CARGO_ENCODED_RUSTFLAGS: flags.join('\x1f') },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr || 'Rust build failed.');
  const artifacts = result.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line))
    .filter(message => message.reason === 'compiler-artifact' && message.target.name === 'khmer_engine')
    .flatMap(message => message.filenames).filter(file => file.endsWith('.wasm'));
  if (artifacts.length !== 1) throw new Error('Expected exactly one Rust Wasm artifact.');
  const output = join(engineRoot, 'wasm');
  mkdirSync(output, { recursive: true });
  const staging = mkdtempSync(join(output, '.public-build-'));
  try {
    const generated = spawnSync(rustTool('wasm-bindgen'), [artifacts[0], '--target', 'web',
      '--out-dir', staging, '--out-name', 'khmer_engine'], { cwd: engineRoot, stdio: 'inherit' });
    if (generated.error) throw generated.error;
    if (generated.status !== 0) throw new Error('wasm-bindgen failed.');
    const hash = createHash('sha256').update(readFileSync(join(staging, 'khmer_engine_bg.wasm'))).digest('hex');
    appendFileSync(join(staging, 'khmer_engine.js'), `\nexport const wasmBuildFingerprint = ${JSON.stringify(hash)};\n`);
    appendFileSync(join(staging, 'khmer_engine.d.ts'), '\nexport const wasmBuildFingerprint: string;\n');
    for (const name of readdirSync(staging)) renameSync(join(staging, name), join(output, name));
    const manifestPath = join(engineRoot, 'dictionary-manifest.json');
    const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
    manifest.wasm = hash;
    writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
  } finally { rmSync(staging, { recursive: true, force: true }); }
}

if (!wasmOnly) {
  const { default: ts } = await import('typescript');
  const source = fileURLToPath(new URL('../src/', import.meta.url));
  const outputFlag = process.argv.indexOf('--out');
  const outputDirectory = outputFlag < 0 ? join(engineRoot, 'dist') : resolve(process.argv[outputFlag + 1]);
  const program = ts.createProgram(['public/node.ts', 'public/browser.ts'].map(name => join(source, name)), {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler,
    lib: ['lib.es2022.d.ts', 'lib.dom.d.ts'], types: ['node'], strict: true, skipLibCheck: true,
    declaration: true, noEmitOnError: true, rootDir: source, outDir: outputDirectory,
  });
  const emitted = program.emit();
  const diagnostics = [...ts.getPreEmitDiagnostics(program), ...emitted.diagnostics];
  if (diagnostics.length) {
    throw new Error(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
      getCurrentDirectory: () => engineRoot, getCanonicalFileName: name => name, getNewLine: () => '\n',
    }));
  }
  console.log('Built standalone JavaScript and TypeScript declarations.');
}
