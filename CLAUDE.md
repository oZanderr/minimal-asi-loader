# CLAUDE.md

Guidance for AI assistants (Claude Code) working in this repo.

## What this is

**Oxide ASI Loader** (crate/binary `oxiloader`) is a minimal ASI proxy loader for Windows games (x64). It
impersonates a system DLL, forwards that DLL's exports to the real copy in
`System32`, and loads `*.asi` plugins. It is deliberately small — no config, no
manifest, and no *game-API* hooks. The single exception is a one-shot hook on the
**host EXE** used purely to time plugin loading (see **Timing** below); it is not
a feature, it is what makes loading work on current Windows. Beyond that, feature
creep is a regression here, not progress.

## Layout

| Path | Purpose |
| --- | --- |
| `src/lib.rs` | `DllMain`, original-DLL resolution, the IAT and entry-point trampolines, and the ASI-loading. ~630 lines, the whole runtime. |
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
     Pass a path with a directory component. A bare `WinDLL("version.dll")` hits
     the normal search order and silently loads the real System32 copy, so the
     test passes while proving nothing.
  3. `WinDLL`/`LoadLibrary` exercises only the **dynamic-load path**
     (`reserved == null`, spawned thread). The static-import path is the one that
     matters for real games, and its branches need host EXEs that *statically
     import* the proxy, placed next to it. Have both the plugin and the host
     append to one marker file: **the assertion is ordering** — `plugin` must
     appear before `host-main`. That ordering is precisely what the 25H2 loader
     change broke, and "the plugin loaded" alone does not test it.

     Four hosts cover every branch. Which one a given host exercises is decided by
     its TLS callbacks and its imports, so check those first (`OriginalFirstThunk`
     walk) rather than assuming:

     | host | TLS callbacks | candidate imports | exercises |
     | --- | --- | --- | --- |
     | Rust bin, `#[link(name = "version")]` | 1 (std registers one) | CRT list only | IAT hook, CRT list |
     | C, plain `main` | 0 | early list | IAT hook, early list |
     | C + `.CRT$XLB` callback calling `QueryPerformanceCounter` | 1 | CRT list | that the callback does **not** trigger us — no load under the loader lock |
     | C, `/NODEFAULTLIB /ENTRY:… /GS-` | 0 | none | fallback to the entry-point trampoline |

     The sharpest test is a TLS callback that reads its own
     `AddressOfEntryPoint` and reports whether it starts with `FF 25`. That runs
     exactly where a protector's integrity check runs: 0.2.1 reports `PATCHED`,
     the IAT hook reports `clean`. Build the C hosts with `clang-cl` and the
     BuildTools/SDK `INCLUDE`/`LIB` set; `cl.exe` is not on `PATH`.
  4. Clean up `testasi.rs`, `dist/test.*`, the hosts, and the marker afterward.

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
      yet, so arrange a callback on the host's own thread after process init and
      before any host code. Deterministic, no race. This is the path real games
      take, and the one that newer Windows 11 loader behavior (25H2 / KB5095093)
      broke for the old "spawn a thread from `DllMain`" approach — an async thread
      with no ordering guarantee that current Windows lets lose the race against
      host startup. Two mechanisms, tried in order:
        1. **IAT trampoline (preferred).** Repoint up to four of the host's import
           slots at naked thunks. The write lands in the import table — which the
           loader itself fills in — so a host that checksums its own `.text` at
           startup sees nothing. Anti-tamper and packers routinely do exactly that,
           and it is why this is tier 1. It also needs only `PAGE_READWRITE`, so it
           survives Arbitrary Code Guard, which blocks the entry-point patch.
        2. **Entry-point trampoline (fallback).** A one-shot absolute-jump
           trampoline over `AddressOfEntryPoint`. Only reached when the host has no
           usable import descriptors.
    - Dynamic load (`reserved == null`, i.e. `LoadLibrary`) — the host is already
      past its entry point, so there is nothing to hook; load from a spawned
      thread, which is safe here.
  A single interlocked latch keeps the plugin sweep to exactly once. If neither
  hook can be installed (odd PE), the static path falls back to the thread.
- **Which imports get hooked depends on the host's TLS callbacks, and that choice
  is load-bearing.** TLS callbacks run *under the loader lock*, before the entry
  point, so being called from one would mean loading plugins under that lock.
  Three lists, first one with a match wins:
    - `EARLY_CANDIDATES` (`GetSystemTimeAsFileTime`, `QueryPerformanceCounter`, …)
      fire inside `__security_init_cookie`, the earliest reachable point. **Only
      used when the host declares no TLS callbacks** — a protector's callback does
      timing and CRC work and plausibly calls exactly these.
    - `UCRT_CANDIDATES` (`_initterm_e`, `_initterm`, `_set_app_type`, …) are the
      CRT's own startup plumbing, present whenever the host links the CRT
      dynamically. First choice for a host *with* TLS callbacks: at TLS-callback
      time the CRT is not initialized, so calling one would be a bug and no
      protector does. Also puts us ahead of static constructors by construction —
      `_initterm(__xc_a, __xc_z)` is the function that runs them.
    - `CRT_CANDIDATES` (`GetStartupInfoW`, `GetCommandLineW`, …) only when the host
      has TLS callbacks *and* a static CRT. Weaker: `SetUnhandledExceptionFilter`
      in particular is something a protector's TLS callback might genuinely call,
      which is why it is last rather than first.
  Every entry must be a function the CRT calls **unconditionally**. Hooking a slot
  nothing calls means `install_iat_hook` reports success, we never fall back, and
  plugins silently never load. That is why `_seh_filter_exe` (exception-only) and
  `_register_onexit_function` / `__p___argc` (lazy) are deliberately excluded.
  Do not "simplify" this into one list.
- **Forwarding stubs and both thunk kinds are `#[unsafe(naked)]`** — their body
  must be a single `core::arch::naked_asm!`, nothing else. The entry-point thunk is
  entered via jump with a pristine entry-point stack; it preserves the volatile
  registers and flags, calls the (synchronous) plugin load, then jumps to the
  restored real entry — so the host proceeds as if untouched. The IAT thunks are
  entered by a real `call` with the caller's arguments live, so they additionally
  preserve `xmm0`–`xmm3` and must keep the stack exactly as the callee expects
  (`sub rsp, 0x68` after the pushes; see the comments in `lib.rs`).
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
