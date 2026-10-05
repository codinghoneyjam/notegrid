# <META - FILE SUMMARY - Centralised repository and workspace paths for tooling scripts.>
import pathlib
import shutil

# Repository & workspace root anchors
SCRIPTS_DIR = pathlib.Path(__file__).resolve().parent
ROOT = SCRIPTS_DIR.parent
REPO = ROOT.parent.parent  # repository root holding tools/notegrid (if embedded in game repo)

# Workspaces
CRATES_DIR = ROOT / "crates"
PACKAGES_DIR = ROOT / "packages"
XTASK_DIR = ROOT / "xtask"
DOCS_DIR = ROOT / "docs"

# Specification & validation fixtures (consolidated under spec/)
SPEC_ROOT = ROOT / "spec"
SPEC_DIR = SPEC_ROOT / "dsl"
SCHEMA_DIR = SPEC_ROOT / "schema"
PROJECT_SCHEMA = SCHEMA_DIR / "project.schema.json"

FIXTURE_DIR = SPEC_ROOT / "fixture"
EXAMPLE_PROJECT = FIXTURE_DIR / "example_project.json"
VALIDATION_DIR = FIXTURE_DIR / "validation"
GOLDEN_DIR = FIXTURE_DIR / "golden"

# Audio sources and outputs
MUSIC_SOURCE_DIR = ROOT / "music_source"
MUSIC_OUTPUT_DIR = ROOT / "music_output"

# Toolchain binaries (CI images vary; fall back to the rustup default location)
CARGO_BIN = shutil.which("cargo") or str(pathlib.Path.home() / ".cargo" / "bin" / "cargo.exe")
