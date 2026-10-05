// <META - FILE SUMMARY - Typed TS wrapper over the C ABI (C4/R07). Hides pointer management; copies buffers.>

export class NgError extends Error {
  code: string;
  path?: string;
  constructor(code: string, message: string, path?: string) {
    super(message);
    this.code = code;
    this.path = path;
  }
}

interface WasmExports {
  ng_abi_version(): number;
  ng_alloc(len: number): number;
  ng_free(ptr: number, len: number): void;
  ng_result_ptr(): number;
  ng_result_len(): number;
  ng_aux_ptr(): number;
  ng_aux_len(): number;
  ng_error_ptr(): number;
  ng_error_len(): number;
  ng_project_validate(ptr: number, len: number): number;
  ng_project_migrate(ptr: number, len: number): number;
  ng_project_canonical(ptr: number, len: number): number;
  ng_gm_tables(): number;
  memory: WebAssembly.Memory;
}

export interface Report {
  errors: Array<{ code: string; message: string; path: string }>;
  warnings: Array<{ code: string; message: string; path: string }>;
}

export class Ng {
  private exports: WasmExports;
  private td = new TextDecoder();

  constructor(exports: WasmExports) {
    this.exports = exports;
  }

  abiVersion(): number {
    return this.exports.ng_abi_version();
  }

  private call(fn: (ptr: number, len: number) => number, input: Uint8Array): { out: Uint8Array; aux: Uint8Array } {
    const ex = this.exports;
    const inPtr = ex.ng_alloc(input.length);
    try {
      new Uint8Array(ex.memory.buffer, inPtr, input.length).set(input);
      const rc = fn(inPtr, input.length);
      if (rc !== 0) {
        throw this.readError(rc);
      }
      const outPtr = ex.ng_result_ptr();
      const outLen = ex.ng_result_len();
      const auxPtr = ex.ng_aux_ptr();
      const auxLen = ex.ng_aux_len();
      const out = new Uint8Array(outLen);
      out.set(new Uint8Array(ex.memory.buffer, outPtr, outLen));
      const aux = new Uint8Array(auxLen);
      aux.set(new Uint8Array(ex.memory.buffer, auxPtr, auxLen));
      return { out, aux };
    } finally {
      ex.ng_free(inPtr, input.length);
    }
  }

  private readError(rc: number): NgError {
    const ex = this.exports;
    const ptr = ex.ng_error_ptr();
    const len = ex.ng_error_len();
    const bytes = new Uint8Array(ex.memory.buffer, ptr, len);
    try {
      const parsed = JSON.parse(this.td.decode(bytes));
      return new NgError(parsed.code ?? `E_RC_${rc}`, parsed.message ?? "unknown");
    } catch {
      return new NgError(`E_RC_${rc}`, "unknown");
    }
  }

  validate(json: string): Report {
    const { out } = this.call(this.exports.ng_project_validate.bind(this.exports), new TextEncoder().encode(json));
    return JSON.parse(this.td.decode(out)) as Report;
  }

  migrate(json: string): string {
    const { out } = this.call(this.exports.ng_project_migrate.bind(this.exports), new TextEncoder().encode(json));
    return this.td.decode(out);
  }

  canonical(json: string): string {
    const { out } = this.call(this.exports.ng_project_canonical.bind(this.exports), new TextEncoder().encode(json));
    return this.td.decode(out);
  }

  gmTables(): unknown {
    const ex = this.exports;
    const rc = ex.ng_gm_tables();
    if (rc !== 0) throw this.readError(rc);
    const ptr = ex.ng_result_ptr();
    const len = ex.ng_result_len();
    const bytes = new Uint8Array(ex.memory.buffer, ptr, len);
    return JSON.parse(this.td.decode(bytes));
  }
}

export async function loadNg(module?: WebAssembly.Module | BufferSource): Promise<Ng> {
  let instance: WebAssembly.Instance;
  if (module instanceof WebAssembly.Module) {
    instance = await WebAssembly.instantiate(module);
  } else if (module) {
    ({ instance } = await WebAssembly.instantiate(module as BufferSource, {}));
  } else {
    const bytes = await (await import("node:fs/promises")).readFile(
      new URL("../dist/ng-wasm.wasm", import.meta.url),
    );
    ({ instance } = await WebAssembly.instantiate(bytes as unknown as BufferSource, {}));
  }
  const exports = instance.exports as unknown as WasmExports;
  const ng = new Ng(exports);
  if (ng.abiVersion() !== 1) {
    throw new NgError("E_VERSION_MISMATCH", `ng_abi_version ${ng.abiVersion()} != 1`);
  }
  return ng;
}
