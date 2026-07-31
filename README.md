# Oxide ASI Loader

**`oxiloader`**: a minimal ASI proxy loader, in Rust.

It impersonates one Windows system DLL, forwards every export to the genuine copy
in `System32`, and loads every `*.asi` plugin next to it. That is the entire
feature set: no config files, no embedded manifest, and no *game-API* hooks. The
one thing it does plant is a throwaway entry-point trampoline on the host EXE,
used purely to time plugin loading (see [How it works](#how-it-works)). Each build
is ~200 KB and links only against `kernel32`/`ntdll`.

## Why

Focused on reliability and a minimal footprint. It ships **no embedded manifest**
and statically links the CRT (**no VC++ redist dependency**), so it carries nothing
that could stop it from mapping into a host process. Everything it needs is in the
DLL itself.

## How it works

- **Proxy DLL, x64 only.** Drop it in the game folder under a name the game
  imports; Windows loads it and runs its `DllMain`.
- **Target chosen at compile time.** A PE export table is fixed at link time, so
  each binary carries exactly one DLL's exports. A cargo feature selects both the
  exports and the forward target, with no runtime self-name detection.
- **Signature-agnostic forwarding.** `build.rs` reads the target DLL's export list
  and emits one `#[naked]` `jmp qword ptr [ptr]` stub per export. At load, the real
  System32 DLL is resolved and each pointer is filled in, so we never hand-write
  hundreds of function signatures.
- **Loader-lock safe.** The original DLL is resolved *synchronously* in `DllMain`,
  so the game's imports are valid the instant it calls them. The `.asi` plugins may
  run heavy code in their own `DllMain`, so they are never loaded under the loader
  lock. *How* the load is deferred depends on how the proxy was loaded:
    - *Static import* (the real-game case). The host EXE's entry point hasn't run
      yet, so the loader plants a one-shot absolute-jump trampoline over it. That
      trampoline fires on the host's own thread, after process init but before any
      host code, loads the plugins once, then jumps to the real entry point — so
      the host proceeds as if untouched. Deterministic, with no startup race (which
      is what current Windows 11 broke for the old "spawn a thread from `DllMain`"
      approach).
    - *Dynamic load* (`LoadLibrary`). Already past the entry point, so plugins load
      from a *fresh thread*, off the loader lock.
- **Deduplicated plugins.** A plugin present in both the loader's own folder and
  `plugins\` is loaded once (the copy next to the loader wins), matched by file
  name case-insensitively.
- **Plugin ABI:** the standard ASI convention. Each `.asi` may export
  `InitializeASI()`, which is called after it loads, so existing ASI plugins work
  unchanged.

## Supported proxy names

`version.dll`, `dsound.dll`, `dxgi.dll`, `winmm.dll`, `dinput8.dll`.

## Build

Requires the `x86_64-pc-windows-msvc` Rust toolchain (Rust ≥ 1.88 for stable naked
functions).

```powershell
# all five variants, correctly named, into dist\
.\build-all.ps1

# or one at a time
cargo build --release --features version   # -> target\release\oxiloader.dll
```

Exactly one proxy feature must be selected: `version`, `dsound`, `dxgi`, `winmm`,
or `dinput8`.

## Install

1. Copy the `dist\<name>.dll` matching a DLL the game imports into the game folder
   (next to the game exe).
2. Put your `.asi` mods in that same folder, or in a `plugins\` subfolder.

Use the artifact whose name matches: a `version.dll` build only works when named
`version.dll` (the export table is baked in at build time).

## License

MIT
