#!/usr/bin/env python3
# <META - FILE SUMMARY - Tooling script; see docstring below.>
"""g05: same inputs through ng-cli and wasm-in-Node produce byte-identical output."""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EXAMPLE = ROOT / "fixture" / "example_project.json"

native = subprocess.run(
    ["cargo", "run", "-p", "ng-cli", "--release", "--", "canonical", str(EXAMPLE)],
    cwd=ROOT, capture_output=True, encoding="utf-8", errors="replace", check=True,
).stdout

wasm = subprocess.run(
    ["node", "-e", """
import('./packages/abi/src/index.ts').then(async ({ loadNg }) => {
  const fs = await import('node:fs');
  const ng = await loadNg();
  process.stdout.write(ng.canonical(fs.readFileSync(process.argv[1], 'utf8')));
});""", str(EXAMPLE)],
    cwd=ROOT, capture_output=True, encoding="utf-8", errors="replace", check=True,
).stdout

if native != wasm:
    print("g05 FAILED: native != wasm output")
    for i, (a, b) in enumerate(zip(native.encode(), wasm.encode())):
        if a != b:
            print(f"first difference at offset {i}: {a:#x} vs {b:#x}")
            break
    sys.exit(1)
print(f"g05 OK ({len(native)} bytes identical)")