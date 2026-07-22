// Generates ordinal-accurate export forwarding for the selected proxy DLL.
//
// A game may import a system DLL by ordinal rather than by name (Marvel Rivals
// imports dsound.dll entirely by ordinal, for example), so our export table must
// place each function at the SAME ordinal as the genuine DLL. We do that with a
// generated `.def` file: the naked `jmp [ptr]` stubs are internal symbols, and the
// `.def` exports the real name at the real ordinal for each. Ordinal-only (NONAME)
// exports are forwarded by ordinal.
//
// Tables are the exports of the stock Windows 11 (26100) System32 DLLs, as
// (name, ordinal) pairs so the ordinals are exact.

use std::{env, fs, path::Path};

const VERSION: &[(&str, u16)] = &[
    ("GetFileVersionInfoA", 1), ("GetFileVersionInfoByHandle", 2),
    ("GetFileVersionInfoExA", 3), ("GetFileVersionInfoExW", 4), ("GetFileVersionInfoSizeA", 5),
    ("GetFileVersionInfoSizeExA", 6), ("GetFileVersionInfoSizeExW", 7),
    ("GetFileVersionInfoSizeW", 8), ("GetFileVersionInfoW", 9), ("VerFindFileA", 10),
    ("VerFindFileW", 11), ("VerInstallFileA", 12), ("VerInstallFileW", 13),
    ("VerLanguageNameA", 14), ("VerLanguageNameW", 15), ("VerQueryValueA", 16),
    ("VerQueryValueW", 17),
];

const DSOUND: &[(&str, u16)] = &[
    ("DirectSoundCreate", 1), ("DirectSoundEnumerateA", 2), ("DirectSoundEnumerateW", 3),
    ("DllCanUnloadNow", 4), ("DllGetClassObject", 5), ("DirectSoundCaptureCreate", 6),
    ("DirectSoundCaptureEnumerateA", 7), ("DirectSoundCaptureEnumerateW", 8),
    ("GetDeviceID", 9), ("DirectSoundFullDuplexCreate", 10), ("DirectSoundCreate8", 11),
    ("DirectSoundCaptureCreate8", 12),
];

const DXGI: &[(&str, u16)] = &[
    ("ApplyCompatResolutionQuirking", 1), ("CompatString", 2), ("CompatValue", 3),
    ("DXGIDumpJournal", 4), ("PIXBeginCapture", 5), ("PIXEndCapture", 6),
    ("PIXGetCaptureState", 7), ("SetAppCompatStringPointer", 8),
    ("UpdateHMDEmulationStatus", 9), ("CreateDXGIFactory", 10), ("CreateDXGIFactory1", 11),
    ("CreateDXGIFactory2", 12), ("DXGID3D10CreateDevice", 13),
    ("DXGID3D10CreateLayeredDevice", 14), ("DXGID3D10GetLayeredDeviceSize", 15),
    ("DXGID3D10RegisterLayers", 16), ("DXGIDeclareAdapterRemovalSupport", 17),
    ("DXGIDisableVBlankVirtualization", 18), ("DXGIGetDebugInterface1", 19),
    ("DXGIReportAdapterConfiguration", 20),
];

const DINPUT8: &[(&str, u16)] = &[
    ("DirectInput8Create", 1), ("DllCanUnloadNow", 2), ("DllGetClassObject", 3),
    ("DllRegisterServer", 4), ("DllUnregisterServer", 5), ("GetdfDIJoystick", 6),
];

const WINMM: &[(&str, u16)] = &[
    ("mciExecute", 3), ("CloseDriver", 4), ("DefDriverProc", 5), ("DriverCallback", 6),
    ("DrvGetModuleHandle", 7), ("GetDriverModuleHandle", 8), ("OpenDriver", 9),
    ("PlaySound", 10), ("PlaySoundA", 11), ("PlaySoundW", 12), ("SendDriverMessage", 13),
    ("WOWAppExit", 14), ("auxGetDevCapsA", 15), ("auxGetDevCapsW", 16), ("auxGetNumDevs", 17),
    ("auxGetVolume", 18), ("auxOutMessage", 19), ("auxSetVolume", 20),
    ("joyConfigChanged", 21), ("joyGetDevCapsA", 22), ("joyGetDevCapsW", 23),
    ("joyGetNumDevs", 24), ("joyGetPos", 25), ("joyGetPosEx", 26), ("joyGetThreshold", 27),
    ("joyReleaseCapture", 28), ("joySetCapture", 29), ("joySetThreshold", 30),
    ("mciDriverNotify", 31), ("mciDriverYield", 32), ("mciFreeCommandResource", 33),
    ("mciGetCreatorTask", 34), ("mciGetDeviceIDA", 35), ("mciGetDeviceIDFromElementIDA", 36),
    ("mciGetDeviceIDFromElementIDW", 37), ("mciGetDeviceIDW", 38), ("mciGetDriverData", 39),
    ("mciGetErrorStringA", 40), ("mciGetErrorStringW", 41), ("mciGetYieldProc", 42),
    ("mciLoadCommandResource", 43), ("mciSendCommandA", 44), ("mciSendCommandW", 45),
    ("mciSendStringA", 46), ("mciSendStringW", 47), ("mciSetDriverData", 48),
    ("mciSetYieldProc", 49), ("midiConnect", 50), ("midiDisconnect", 51),
    ("midiInAddBuffer", 52), ("midiInClose", 53), ("midiInGetDevCapsA", 54),
    ("midiInGetDevCapsW", 55), ("midiInGetErrorTextA", 56), ("midiInGetErrorTextW", 57),
    ("midiInGetID", 58), ("midiInGetNumDevs", 59), ("midiInMessage", 60), ("midiInOpen", 61),
    ("midiInPrepareHeader", 62), ("midiInReset", 63), ("midiInStart", 64), ("midiInStop", 65),
    ("midiInUnprepareHeader", 66), ("midiOutCacheDrumPatches", 67),
    ("midiOutCachePatches", 68), ("midiOutClose", 69), ("midiOutGetDevCapsA", 70),
    ("midiOutGetDevCapsW", 71), ("midiOutGetErrorTextA", 72), ("midiOutGetErrorTextW", 73),
    ("midiOutGetID", 74), ("midiOutGetNumDevs", 75), ("midiOutGetVolume", 76),
    ("midiOutLongMsg", 77), ("midiOutMessage", 78), ("midiOutOpen", 79),
    ("midiOutPrepareHeader", 80), ("midiOutReset", 81), ("midiOutSetVolume", 82),
    ("midiOutShortMsg", 83), ("midiOutUnprepareHeader", 84), ("midiStreamClose", 85),
    ("midiStreamOpen", 86), ("midiStreamOut", 87), ("midiStreamPause", 88),
    ("midiStreamPosition", 89), ("midiStreamProperty", 90), ("midiStreamRestart", 91),
    ("midiStreamStop", 92), ("mixerClose", 93), ("mixerGetControlDetailsA", 94),
    ("mixerGetControlDetailsW", 95), ("mixerGetDevCapsA", 96), ("mixerGetDevCapsW", 97),
    ("mixerGetID", 98), ("mixerGetLineControlsA", 99), ("mixerGetLineControlsW", 100),
    ("mixerGetLineInfoA", 101), ("mixerGetLineInfoW", 102), ("mixerGetNumDevs", 103),
    ("mixerMessage", 104), ("mixerOpen", 105), ("mixerSetControlDetails", 106),
    ("mmDrvInstall", 107), ("mmGetCurrentTask", 108), ("mmTaskBlock", 109),
    ("mmTaskCreate", 110), ("mmTaskSignal", 111), ("mmTaskYield", 112), ("mmioAdvance", 113),
    ("mmioAscend", 114), ("mmioClose", 115), ("mmioCreateChunk", 116), ("mmioDescend", 117),
    ("mmioFlush", 118), ("mmioGetInfo", 119), ("mmioInstallIOProcA", 120),
    ("mmioInstallIOProcW", 121), ("mmioOpenA", 122), ("mmioOpenW", 123), ("mmioRead", 124),
    ("mmioRenameA", 125), ("mmioRenameW", 126), ("mmioSeek", 127), ("mmioSendMessage", 128),
    ("mmioSetBuffer", 129), ("mmioSetInfo", 130), ("mmioStringToFOURCCA", 131),
    ("mmioStringToFOURCCW", 132), ("mmioWrite", 133), ("mmsystemGetVersion", 134),
    ("sndPlaySoundA", 135), ("sndPlaySoundW", 136), ("timeBeginPeriod", 137),
    ("timeEndPeriod", 138), ("timeGetDevCaps", 139), ("timeGetSystemTime", 140),
    ("timeGetTime", 141), ("timeKillEvent", 142), ("timeSetEvent", 143),
    ("waveInAddBuffer", 144), ("waveInClose", 145), ("waveInGetDevCapsA", 146),
    ("waveInGetDevCapsW", 147), ("waveInGetErrorTextA", 148), ("waveInGetErrorTextW", 149),
    ("waveInGetID", 150), ("waveInGetNumDevs", 151), ("waveInGetPosition", 152),
    ("waveInMessage", 153), ("waveInOpen", 154), ("waveInPrepareHeader", 155),
    ("waveInReset", 156), ("waveInStart", 157), ("waveInStop", 158),
    ("waveInUnprepareHeader", 159), ("waveOutBreakLoop", 160), ("waveOutClose", 161),
    ("waveOutGetDevCapsA", 162), ("waveOutGetDevCapsW", 163), ("waveOutGetErrorTextA", 164),
    ("waveOutGetErrorTextW", 165), ("waveOutGetID", 166), ("waveOutGetNumDevs", 167),
    ("waveOutGetPitch", 168), ("waveOutGetPlaybackRate", 169), ("waveOutGetPosition", 170),
    ("waveOutGetVolume", 171), ("waveOutMessage", 172), ("waveOutOpen", 173),
    ("waveOutPause", 174), ("waveOutPrepareHeader", 175), ("waveOutReset", 176),
    ("waveOutRestart", 177), ("waveOutSetPitch", 178), ("waveOutSetPlaybackRate", 179),
    ("waveOutSetVolume", 180), ("waveOutUnprepareHeader", 181), ("waveOutWrite", 182),
];

// Ordinal-only (NONAME) exports, forwarded by ordinal.
const WINMM_NONAME: &[u16] = &[2];

#[allow(clippy::type_complexity)]
fn selected() -> (&'static str, &'static [(&'static str, u16)], &'static [u16]) {
    let table: &[(&str, &str, &[(&str, u16)], &[u16])] = &[
        ("VERSION", "version.dll", VERSION, &[]),
        ("DSOUND", "dsound.dll", DSOUND, &[]),
        ("DXGI", "dxgi.dll", DXGI, &[]),
        ("WINMM", "winmm.dll", WINMM, WINMM_NONAME),
        ("DINPUT8", "dinput8.dll", DINPUT8, &[]),
    ];
    let active: Vec<_> = table
        .iter()
        .filter(|(feat, _, _, _)| env::var(format!("CARGO_FEATURE_{feat}")).is_ok())
        .collect();
    match active.len() {
        1 => (active[0].1, active[0].2, active[0].3),
        0 => panic!("select exactly one proxy: cargo build --release --features version|dsound|dxgi|winmm|dinput8"),
        _ => panic!("select only ONE proxy feature at a time"),
    }
}

fn main() {
    let (target_dll, named, noname) = selected();
    let total = named.len() + noname.len();
    let out_dir = env::var("OUT_DIR").unwrap();

    // ---- generated Rust: TARGET_DLL, forwarding pointers, naked stubs, resolve_exports ----
    let mut rs = String::new();
    rs.push_str(&format!("pub const TARGET_DLL: &str = \"{target_dll}\";\n"));
    for i in 0..total {
        rs.push_str(&format!("static mut P{i}: usize = 0;\n"));
    }
    for i in 0..total {
        rs.push_str(&format!(
            "#[unsafe(naked)]\n#[no_mangle]\npub extern \"system\" fn asi_fwd_{i}() {{ core::arch::naked_asm!(\"jmp qword ptr [rip + {{0}}]\", sym P{i}) }}\n"
        ));
    }
    rs.push_str("pub unsafe fn resolve_exports(m: HMODULE) {\n");
    for (i, (name, _)) in named.iter().enumerate() {
        rs.push_str(&format!(
            "    if let Some(f) = GetProcAddress(m, b\"{name}\\0\".as_ptr()) {{ P{i} = f as usize; }}\n"
        ));
    }
    for (k, ord) in noname.iter().enumerate() {
        let i = named.len() + k;
        rs.push_str(&format!(
            "    if let Some(f) = GetProcAddress(m, {ord}usize as *const u8) {{ P{i} = f as usize; }}\n"
        ));
    }
    rs.push_str("}\n");
    fs::write(Path::new(&out_dir).join("exports_generated.rs"), rs).unwrap();

    // ---- generated .def: real names at real ordinals; NONAME exports ordinal-only ----
    let mut def = String::from("EXPORTS\n");
    for (i, (name, ord)) in named.iter().enumerate() {
        def.push_str(&format!("{name}=asi_fwd_{i} @{ord}\n"));
    }
    for (k, ord) in noname.iter().enumerate() {
        let i = named.len() + k;
        def.push_str(&format!("noname_{ord}=asi_fwd_{i} @{ord} NONAME\n"));
    }
    let def_path = Path::new(&out_dir).join("exports.def");
    fs::write(&def_path, def).unwrap();
    println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def_path.display());

    println!("cargo:rerun-if-changed=build.rs");
}
