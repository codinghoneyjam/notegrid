// SP-2 wasm-side driver: render via wasm, write same PCM bytes as native for comparison.
import { readFileSync, writeFileSync } from 'node:fs';

const bytes = readFileSync(new URL('../../target/wasm32-unknown-unknown/release/ng_spike_sp2.wasm', import.meta.url));
const { instance } = await WebAssembly.instantiate(bytes, {});
const ex = instance.exports;
const frames = 44100;
const need = 64 + frames * 4;
const pages = Math.ceil((need - ex.memory.buffer.byteLength) / 65536);
if (pages > 0) ex.memory.grow(pages);
ex.spike_render(64, frames, 44100);
const view = new Uint8Array(ex.memory.buffer, 64, frames * 4);
writeFileSync(new URL('../sp2/wasm_sine.f32le', import.meta.url), view);
console.log(`wrote ${view.length} bytes`);
