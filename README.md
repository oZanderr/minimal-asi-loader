# Oxide ASI Loader

**`oxiloader`** — a minimal ASI proxy loader, in Rust.

It impersonates one Windows system DLL, forwards every export to the genuine copy
in `System32`, and loads every `*.asi` plugin next to it. That is the entire
feature set — no config files, no embedded manifest, no hooks. Each build is
~200 KB and links only against `kernel32`/`ntdll`.

## Why another loader?

A lean alternative to [Ultimate ASI Loader](https://github.com/ThirteenAG/Ultimate-ASI-Loader).
Recent UAL builds embed a `Microsoft.Windows.Common-Controls` v6 side-by-side
manifest dependency (for their TaskDialog UI). In some game processes that
dependency fails to resolve, and the DLL then fails to map **before `DllMain`
runs** — no log, no plugins, game launches fine. oxiloader ships **no manifest**
and statically links the CRT (**no VC++ redist dependency**), so it maps anywhere
the old, lean UAL builds did.

## How it works

- **Proxy DLL, x64 only.** Drop it in the game folder under a name the game
  imports; Windows loads it and runs its `DllMain`.
- **Target chosen at compile time.** A PE export table is fixed at link time, so
  each binary carries exactly one DLL's exports. A cargo feature selects both the
  exports and the forward target — no runtime self-name detection.
- **Signature-agnostic forwarding.** `build.rs` reads the target DLL's export list
  and emits one `#[naked]` `jmp qword ptr [ptr]` stub per export. At load, the real
  System32 DLL is resolved and each pointer is filled in, so we never hand-write
  hundreds of function signatures.
- **Loader-lock safe.** The original DLL is resolved *synchronously* in `DllMain`,
  so the game's imports are valid the instant it calls them. The `.asi` plugins —
  which may run heavy code in their own `DllMain` — are loaded from a *fresh
  thread*, off the loader lock.
- **Plugin ABI:** identical to Ultimate ASI Loader. Each `.asi` may export
  `InitializeASI()`, which is called after it loads, so existing ASI mods work
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

Use the artifact whose name matches — a `version.dll` build only works when named
`version.dll` (the export table is baked in at build time).

## Scope / anticheat

oxiloader is a clean, low-footprint loader for **single-player modding**: it only
`LoadLibrary`s DLLs — no memory patching, no hooks, no remote threads. It does
**not** attempt to hide from or defeat kernel-mode anticheat (EAC / BattlEye /
Vanguard); loading unsigned DLLs into a protected multiplayer game is detectable
regardless of vector, and evasion is deliberately out of scope.

## License

MIT
