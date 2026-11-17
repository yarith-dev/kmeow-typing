import { initSync, type InitInput, type SyncInitInput } from '../../wasm/khmer_engine.js';

let ready = false;
let initialization: Promise<void> | undefined;

/** Install the shared instance synchronously, with no await between the guard and installation. */
export function initializeWasmSync(source: () => SyncInitInput): void {
  if (ready) return;
  initSync({ module: source() });
  ready = true;
}

/** Async loading/compilation never installs another instance over existing engine pointers. */
export async function initializeWasm(source: () => InitInput | Promise<InitInput>): Promise<void> {
  if (ready) return;
  initialization ??= (async () => {
    let input = await source();
    if (ready) return;
    if (typeof input === 'string' || (typeof URL === 'function' && input instanceof URL)
      || (typeof Request === 'function' && input instanceof Request)) input = await fetch(input);
    if (ready) return;
    let module: WebAssembly.Module;
    if (input instanceof WebAssembly.Module) module = input;
    else if (typeof Response === 'function' && input instanceof Response) {
      if (!input.ok) throw new Error(`Could not load Wasm: HTTP ${input.status}.`);
      module = typeof WebAssembly.compileStreaming === 'function' && input.headers.get('Content-Type') === 'application/wasm'
        ? await WebAssembly.compileStreaming(input)
        : await WebAssembly.compile(await input.arrayBuffer());
    } else module = await WebAssembly.compile(input as BufferSource);
    // A sync caller may have initialized while fetching or compiling. Reuse its instance.
    initializeWasmSync(() => module);
  })().catch(error => {
    initialization = undefined;
    if (!ready) throw error;
  });
  await initialization;
}
