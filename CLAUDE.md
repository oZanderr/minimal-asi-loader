# CLAUDE.md

Guidance for AI assistants (Claude Code) working in this repo.

## What this is

**Oxide ASI Loader** (crate/binary `oxiloader`) is a minimal ASI proxy loader for Windows games (x64). It
impersonates a system DLL, forwards that DLL's exports to the real copy in
`System32`, and loads `*.asi` plugins. It is deliberately small — no config, no
manifest, no hooks. Keep it that way; feature creep is a regression here, not
progress.

## Layout

| Path | Purpose |
| --- | --- |
| `src/lib.rs` | `DllMain`, original-DLL resolution, the ASI-loading thread. ~150 lines, the whole runtime. |
| `build.rs` | Generates the per-name export-forwarding stubs. **Holds the export-name tables** for each proxy DLL. |
| `Cargo.toml` | One cargo feature per proxy name; exactly one must be enabled per build. |
| `build-all.ps1` | Builds all five variants, copies each to `dist\<name>.dll`. |
| `.cargo/config.toml` | Forces `+crt-static` (no VCRUNTIME/redist dependency). |

The generated stubs land in `$OUT_DIR/exports_generated.rs` and are `include!`d by
`lib.rs`. They provide `TARGET_DLL`, `EXPORT_NAMES`, the naked `jmp` stubs, and
`store_ptr()`.

## Build & test

```powershell
cargo build --release --features version   # one variant
.\build-all.ps1                            # all five -> dist\
```

- Toolchain: `x86_64-pc-windows-msvc`, Rust ≥ 1.88 (`naked_asm!` stability).
- There is no unit-test suite. Validate changes with a **runtime smoke test**:
  1. Build a throwaway plugin that exports `InitializeASI` and writes a marker
     file: `rustc --edition 2021 -O --crate-type cdylib testasi.rs -o dist/test.asi`.
  2. Load a built proxy into a host process and confirm two things — a forwarded
     export returns real data, and the marker appears (plugin was loaded and
     `InitializeASI` ran). Python is convenient:
     ```python
     import ctypes; h = ctypes.WinDLL(r"dist\version.dll")
     # call e.g. h.GetFileVersionInfoSizeW(...) -> nonzero proves forwarding
     # check the marker file -> proves ASI loading
     ```
  3. Clean up `testasi.rs`, `dist/test.*`, and the marker afterward.

## Invariants — do not break these

- **x64 only.**
- **Never add an embedded manifest or manifest dependency.** An external assembly
  dependency can stop the DLL from mapping in some host processes. No
  `/manifestdependency`, no `RT_MANIFEST` resource — keep the binary
  self-contained.
- **Keep the static CRT** (`.cargo/config.toml`). The loader must load on a bare
  machine with no VC++ redist.
- **One artifact = one proxy name.** The PE export table is fixed at link time; a
  build cannot be renamed across proxy names.
- **Timing:** resolve the original DLL *synchronously in `DllMain`* (the game may
  call a forwarded export immediately). Load `.asi` files *only from the spawned
  thread* — doing it in `DllMain` risks loader-lock deadlocks.
- **Forwarding stubs are `#[unsafe(naked)]`** — their body must be a single
  `core::arch::naked_asm!`, nothing else.
- **Plugin ABI is `InitializeASI()`** (`extern "system"`) — the standard ASI
  convention. Don't rename or change its signature.

## Adding a proxy name

1. Add the feature to `[features]` in `Cargo.toml`.
2. In `build.rs`: add the DLL's export-name list, and a
   `(FEATURE, "name.dll", LIST)` row in `selected()`'s table.
3. Add the name to `$names` in `build-all.ps1`.

Get the export list from the real System32 DLL — `dumpbin /exports name.dll`, or
`pefile` in Python. Include every named export; missing one just means that single
function isn't proxied.

## Scope / policy

Single-player modding tool. Do **not** add anticheat evasion (module hiding, PEB
unlinking, signature spoofing, anti-debug). That is out of scope by design, not an
oversight.
