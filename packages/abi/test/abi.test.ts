import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { loadNg } from '../src/index.ts';

test('abi exports match expected set (g08)', async () => {
  const bytes = readFileSync(new URL('../dist/ng-wasm.wasm', import.meta.url));
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const names = Object.keys(instance.exports).sort();
  assert.deepEqual(names, [
    'memory', 'ng_abi_version', 'ng_alloc', 'ng_aux_len', 'ng_aux_ptr',
    'ng_error_len', 'ng_error_ptr', 'ng_free', 'ng_gm_tables',
    'ng_project_canonical', 'ng_project_migrate', 'ng_project_validate',
    'ng_result_len', 'ng_result_ptr',
  ].sort());
});

test('wrappers drive validate/migrate/canonical/gm_tables', async () => {
  const ng = await loadNg();
  assert.equal(ng.abiVersion(), 1);
  const project = readFileSync(new URL('../../../fixture/example_project.json', import.meta.url), 'utf8');
  const report = ng.validate(project);
  assert.equal(report.errors.length, 0);
  const canonical = ng.canonical(project);
  const again = ng.canonical(canonical);
  assert.equal(canonical, again);
  const migrated = ng.migrate(project);
  assert.ok(migrated.includes('"format_version": 1'));
  const tables = ng.gmTables() as { programs: string[]; families: string[] };
  assert.equal(tables.programs.length, 128);
  assert.equal(tables.families.length, 16);
});

test('invalid payload returns error / failing report without trap', async () => {
  const ng = await loadNg();
  const bad = ng.validate('{not json');
  assert.ok(bad.errors.length >= 1);
  assert.throws(() => ng.canonical('{"format_version": 99}'));
});

test('invalid pointer/length tolerated (no trap)', async () => {
  const { instance } = await WebAssembly.instantiate(readFileSync(new URL('../dist/ng-wasm.wasm', import.meta.url)), {});
  const ex = instance.exports as Record<string, Function> & { memory: WebAssembly.Memory };
  // ptr 0 / len 0 -> error code, never a trap
  assert.notEqual(ex.ng_project_validate(0, 0), 0);
  assert.notEqual(ex.ng_project_migrate(0, 0), 0);
  assert.notEqual(ex.ng_project_canonical(0, 0), 0);
  // buffer with garbage content -> error code, never a trap
  const ptr = ex.ng_alloc(16);
  new Uint8Array(ex.memory.buffer, ptr, 16).fill(0xff);
  const rc = ex.ng_project_validate(ptr, 16);
  assert.equal(rc, 0); // validate always returns a Report (0)
  const report = JSON.parse(new TextDecoder().decode(new Uint8Array(ex.memory.buffer, ex.ng_result_ptr(), ex.ng_result_len())));
  assert.ok(report.errors.length >= 1);
  ex.ng_free(ptr, 16);
});
