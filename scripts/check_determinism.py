#!/usr/bin/env python3
# <META - FILE SUMMARY - Tooling script; see docstring below.>
"""g05: same inputs through ng-cli and wasm-in-Node produce byte-identical output.

Covers both the canonical JSON path and, since T1.6, a full offline render of the example
project: native CLI render vs wasm render must match, and both must match the checked-in
golden WAV (drift between the engine and its golden is exactly what g12 guards)."""
import subprocess
import sys
import tempfile
from pathlib import Path

from paths import CARGO_BIN, GOLDEN_DIR, ROOT, EXAMPLE_PROJECT as EXAMPLE

ABI_INDEX = (ROOT / "packages" / "abi" / "src" / "index.ts").resolve().as_uri()

def run_node(script: str, *argv: str) -> bytes:
    """Write script to a temp .mjs and run it with Node (strips TS types on Node 24)."""
    with tempfile.NamedTemporaryFile("w", suffix=".mjs", delete=False, encoding="utf-8") as tmp:
        tmp.write(script)
        script_path = Path(tmp.name)
    try:
        proc = subprocess.run(
            ["node", str(script_path), *argv],
            cwd=ROOT, capture_output=True, encoding="utf-8", errors="replace",
        )
    finally:
        script_path.unlink(missing_ok=True)
    if proc.returncode != 0:
        print(proc.stderr)
        sys.exit(proc.returncode)
    return proc.stdout.encode("utf-8") if isinstance(proc.stdout, str) else proc.stdout

# --- canonical (text) path -------------------------------------------------
native = subprocess.run(
    [CARGO_BIN, "run", "-p", "ng-cli", "--release", "--", "canonical", str(EXAMPLE)],
    cwd=ROOT, capture_output=True, encoding="utf-8", errors="replace", check=True,
).stdout

wasm = run_node(f"""
import({ABI_INDEX!r}).then(async ({{ loadNg }}) => {{
  const fs = await import('node:fs');
  const ng = await loadNg();
  process.stdout.write(ng.canonical(fs.readFileSync(process.argv[2], 'utf8')));
}});""", str(EXAMPLE)).decode("utf-8")

if native != wasm:
    print("g05 FAILED: native != wasm (canonical)")
    for i, (a, b) in enumerate(zip(native.encode(), wasm.encode())):
        if a != b:
            print(f"first difference at offset {i}: {a:#x} vs {b:#x}")
            break
    sys.exit(1)
print(f"g05 canonical OK ({len(native)} bytes identical)")

# --- full render path ------------------------------------------------------
proc = subprocess.run(
    [CARGO_BIN, "run", "-p", "ng-cli", "--release", "--", "render", str(EXAMPLE), "--depth", "16"],
    cwd=ROOT, capture_output=True, encoding="utf-8", errors="replace", check=True,
)
print("native render ->", proc.stdout.strip())
native_wav_path = Path(proc.stdout.strip().splitlines()[-1])
native_wav = native_wav_path.read_bytes()

wasm_tmp = Path(tempfile.mktemp(suffix=".wav"))
run_node(f"""
import({ABI_INDEX!r}).then(async ({{ loadNg }}) => {{
  const fs = await import('node:fs');
  const ng = await loadNg();
  const bytes = ng.renderWav(fs.readFileSync({str(EXAMPLE).replace(chr(92), '/')!r}, 'utf8'), 44100, 0);
  fs.writeFileSync({str(wasm_tmp).replace(chr(92), '/')!r}, Buffer.from(bytes));
}});""")
wasm_wav = wasm_tmp.read_bytes()
wasm_tmp.unlink()

golden_path = GOLDEN_DIR / "render" / "example_44100.wav"
golden = golden_path.read_bytes() if golden_path.exists() else None

if wasm_wav != native_wav:
    print(f"g05 FAILED: native WAV != wasm WAV ({len(native_wav)} vs {len(wasm_wav)} bytes)")
    for i, (a, b) in enumerate(zip(native_wav, wasm_wav)):
        if a != b:
            print(f"first difference at offset {i}: {a:#x} vs {b:#x}")
            break
    sys.exit(1)
if golden is not None and wasm_wav != golden:
    print(f"g05 FAILED: render output drifted from golden ({len(wasm_wav)} vs {len(golden)} bytes)")
    for i, (a, b) in enumerate(zip(golden, wasm_wav)):
        if a != b:
            print(f"first difference at offset {i}: golden {a:#x} vs rendered {b:#x}")
            break
    sys.exit(1)
if golden is None:
    print(f"g05 NOTE: no golden at {golden_path}; commit one via docs/golden_changes.md")

print(f"g05 OK ({len(native_wav)} bytes native == wasm{' == golden' if golden == wasm_wav else ''})")
