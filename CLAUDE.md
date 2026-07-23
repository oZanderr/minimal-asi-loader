# CLAUDE.md

Guidance for AI assistants (Claude Code) working in this repo.

## What this is

**Oxide ASI Loader** (crate/binary `oxiloader`) is a minimal ASI proxy loader for Windows games (x64). It
impersonates a system DLL, forwards that DLL's exports to the real copy in
`System32`, and loads `*.asi` plugins. It is deliberately small — no config, no
manifest, and no *game-API* hooks. The single exception is a one-shot
entry-point trampoline over the **host EXE** used purely to time plugin loading
(see **Timing** below); it is not a feature, it is what makes loading work on
current Windows. Beyond that, feature creep is a regression here, not progress.

## Layout

| Path | Purpose |
| --- | --- |
| `src/lib.rs` | `DllMain`, original-DLL resolution, the entry-point trampoline, and the ASI-loading. ~340 lines, the whole runtime. |
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
     Note: `WinDLL`/`LoadLibrary` exercises only the **dynamic-load path**
     (`reserved == null`, spawned thread). To exercise the **entry-point
     trampoline** — the static-import path that matters for real games — you need
     a host EXE that *statically imports* the proxy (e.g. a tiny Rust bin with
     `#[link(name = "version")]` calling a forwarded export), placed next to the
     proxy DLL. If plugins load before that host's `main()` runs, the trampoline
     works.
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
  call a forwarded export immediately). **Never load `.asi` files under the loader
  lock** — they run arbitrary `DllMain` code. *How* we defer is chosen from
  `DllMain`'s `reserved` argument, which tells us how we were loaded:
    - Static import (`reserved != null`) — the host EXE's entry point has not run
      yet. Plant a one-shot absolute-jump trampoline over its entry point and load
      plugins from there: host thread, after process init, before any host code.
      Deterministic, no race. This is the path real games take, and the one that
      newer Windows 11 loader behavior (25H2 / KB5095093) broke for the old
      "spawn a thread from `DllMain`" approach — an async thread with no ordering
      guarantee that current Windows lets lose the race against host startup.
    - Dynamic load (`reserved == null`, i.e. `LoadLibrary`) — the host is already
      past its entry point, so trampolining is pointless; load from a spawned
      thread, which is safe here.
  A single interlocked latch keeps the plugin sweep to exactly once. If the
  trampoline can't be planted (odd PE), the static path falls back to the thread.
- **Forwarding stubs and the entry-point thunk are `#[unsafe(naked)]`** — their
  body must be a single `core::arch::naked_asm!`, nothing else. The thunk is
  entered via jump with a pristine entry-point stack; it preserves the volatile
  registers and flags, calls the (synchronous) plugin load, then jumps to the
  restored real entry — so the host proceeds as if untouched.
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
