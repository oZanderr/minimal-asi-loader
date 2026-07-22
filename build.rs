// Generates the export forwarding stubs for the selected proxy DLL.
//
// One binary can only carry one PE export table, so the proxy name is chosen at
// build time via a cargo feature. build.rs emits, for the active feature:
//   - TARGET_DLL: the System32 DLL to forward to
//   - EXPORT_NAMES: every named export of that DLL
//   - a naked `jmp [ptr]` stub per export (signature-agnostic forwarding)
//   - store_ptr(i, addr): fills the i-th forwarding pointer at load time
//
// Export lists are the named exports of the stock Windows 11 (26100) System32
// DLLs. Missing a name only means that one function isn't proxied; the loader
// still works for everything a game actually imports.

use std::{env, fs, path::Path};

const VERSION: &[&str] = &[
    "GetFileVersionInfoA", "GetFileVersionInfoByHandle", "GetFileVersionInfoExA",
    "GetFileVersionInfoExW", "GetFileVersionInfoSizeA", "GetFileVersionInfoSizeExA",
    "GetFileVersionInfoSizeExW", "GetFileVersionInfoSizeW", "GetFileVersionInfoW",
    "VerFindFileA", "VerFindFileW", "VerInstallFileA", "VerInstallFileW",
    "VerLanguageNameA", "VerLanguageNameW", "VerQueryValueA", "VerQueryValueW",
];

const DSOUND: &[&str] = &[
    "DirectSoundCaptureCreate", "DirectSoundCaptureCreate8", "DirectSoundCaptureEnumerateA",
    "DirectSoundCaptureEnumerateW", "DirectSoundCreate", "DirectSoundCreate8",
    "DirectSoundEnumerateA", "DirectSoundEnumerateW", "DirectSoundFullDuplexCreate",
    "DllCanUnloadNow", "DllGetClassObject", "GetDeviceID",
];

const DXGI: &[&str] = &[
    "ApplyCompatResolutionQuirking", "CompatString", "CompatValue", "CreateDXGIFactory",
    "CreateDXGIFactory1", "CreateDXGIFactory2", "DXGID3D10CreateDevice",
    "DXGID3D10CreateLayeredDevice", "DXGID3D10GetLayeredDeviceSize", "DXGID3D10RegisterLayers",
    "DXGIDeclareAdapterRemovalSupport", "DXGIDisableVBlankVirtualization", "DXGIDumpJournal",
    "DXGIGetDebugInterface1", "DXGIReportAdapterConfiguration", "PIXBeginCapture",
    "PIXEndCapture", "PIXGetCaptureState", "SetAppCompatStringPointer", "UpdateHMDEmulationStatus",
];

const DINPUT8: &[&str] = &[
    "DirectInput8Create", "DllCanUnloadNow", "DllGetClassObject",
    "DllRegisterServer", "DllUnregisterServer", "GetdfDIJoystick",
];

const WINMM: &[&str] = &[
    "CloseDriver", "DefDriverProc", "DriverCallback", "DrvGetModuleHandle",
    "GetDriverModuleHandle", "OpenDriver", "PlaySound", "PlaySoundA", "PlaySoundW",
    "SendDriverMessage", "WOWAppExit", "auxGetDevCapsA", "auxGetDevCapsW", "auxGetNumDevs",
    "auxGetVolume", "auxOutMessage", "auxSetVolume", "joyConfigChanged", "joyGetDevCapsA",
    "joyGetDevCapsW", "joyGetNumDevs", "joyGetPos", "joyGetPosEx", "joyGetThreshold",
    "joyReleaseCapture", "joySetCapture", "joySetThreshold", "mciDriverNotify", "mciDriverYield",
    "mciExecute", "mciFreeCommandResource", "mciGetCreatorTask", "mciGetDeviceIDA",
    "mciGetDeviceIDFromElementIDA", "mciGetDeviceIDFromElementIDW", "mciGetDeviceIDW",
    "mciGetDriverData", "mciGetErrorStringA", "mciGetErrorStringW", "mciGetYieldProc",
    "mciLoadCommandResource", "mciSendCommandA", "mciSendCommandW", "mciSendStringA",
    "mciSendStringW", "mciSetDriverData", "mciSetYieldProc", "midiConnect", "midiDisconnect",
    "midiInAddBuffer", "midiInClose", "midiInGetDevCapsA", "midiInGetDevCapsW",
    "midiInGetErrorTextA", "midiInGetErrorTextW", "midiInGetID", "midiInGetNumDevs",
    "midiInMessage", "midiInOpen", "midiInPrepareHeader", "midiInReset", "midiInStart",
    "midiInStop", "midiInUnprepareHeader", "midiOutCacheDrumPatches", "midiOutCachePatches",
    "midiOutClose", "midiOutGetDevCapsA", "midiOutGetDevCapsW", "midiOutGetErrorTextA",
    "midiOutGetErrorTextW", "midiOutGetID", "midiOutGetNumDevs", "midiOutGetVolume",
    "midiOutLongMsg", "midiOutMessage", "midiOutOpen", "midiOutPrepareHeader", "midiOutReset",
    "midiOutSetVolume", "midiOutShortMsg", "midiOutUnprepareHeader", "midiStreamClose",
    "midiStreamOpen", "midiStreamOut", "midiStreamPause", "midiStreamPosition",
    "midiStreamProperty", "midiStreamRestart", "midiStreamStop", "mixerClose",
    "mixerGetControlDetailsA", "mixerGetControlDetailsW", "mixerGetDevCapsA", "mixerGetDevCapsW",
    "mixerGetID", "mixerGetLineControlsA", "mixerGetLineControlsW", "mixerGetLineInfoA",
    "mixerGetLineInfoW", "mixerGetNumDevs", "mixerMessage", "mixerOpen", "mixerSetControlDetails",
    "mmDrvInstall", "mmGetCurrentTask", "mmTaskBlock", "mmTaskCreate", "mmTaskSignal",
    "mmTaskYield", "mmioAdvance", "mmioAscend", "mmioClose", "mmioCreateChunk", "mmioDescend",
    "mmioFlush", "mmioGetInfo", "mmioInstallIOProcA", "mmioInstallIOProcW", "mmioOpenA",
    "mmioOpenW", "mmioRead", "mmioRenameA", "mmioRenameW", "mmioSeek", "mmioSendMessage",
    "mmioSetBuffer", "mmioSetInfo", "mmioStringToFOURCCA", "mmioStringToFOURCCW", "mmioWrite",
    "mmsystemGetVersion", "sndPlaySoundA", "sndPlaySoundW", "timeBeginPeriod", "timeEndPeriod",
    "timeGetDevCaps", "timeGetSystemTime", "timeGetTime", "timeKillEvent", "timeSetEvent",
    "waveInAddBuffer", "waveInClose", "waveInGetDevCapsA", "waveInGetDevCapsW",
    "waveInGetErrorTextA", "waveInGetErrorTextW", "waveInGetID", "waveInGetNumDevs",
    "waveInGetPosition", "waveInMessage", "waveInOpen", "waveInPrepareHeader", "waveInReset",
    "waveInStart", "waveInStop", "waveInUnprepareHeader", "waveOutBreakLoop", "waveOutClose",
    "waveOutGetDevCapsA", "waveOutGetDevCapsW", "waveOutGetErrorTextA", "waveOutGetErrorTextW",
    "waveOutGetID", "waveOutGetNumDevs", "waveOutGetPitch", "waveOutGetPlaybackRate",
    "waveOutGetPosition", "waveOutGetVolume", "waveOutMessage", "waveOutOpen", "waveOutPause",
    "waveOutPrepareHeader", "waveOutReset", "waveOutRestart", "waveOutSetPitch",
    "waveOutSetPlaybackRate", "waveOutSetVolume", "waveOutUnprepareHeader", "waveOutWrite",
];

fn selected() -> (&'static str, &'static [&'static str]) {
    let table: &[(&str, &str, &[&str])] = &[
        ("VERSION", "version.dll", VERSION),
        ("DSOUND", "dsound.dll", DSOUND),
        ("DXGI", "dxgi.dll", DXGI),
        ("WINMM", "winmm.dll", WINMM),
        ("DINPUT8", "dinput8.dll", DINPUT8),
    ];
    let active: Vec<_> = table
        .iter()
        .filter(|(feat, _, _)| env::var(format!("CARGO_FEATURE_{feat}")).is_ok())
        .collect();
    match active.len() {
        1 => (active[0].1, active[0].2),
        0 => panic!("select exactly one proxy: cargo build --release --features version|dsound|dxgi|winmm|dinput8"),
        _ => panic!("select only ONE proxy feature at a time"),
    }
}

fn main() {
    let (target_dll, names) = selected();

    let mut s = String::new();
    s.push_str(&format!("pub const TARGET_DLL: &str = \"{target_dll}\";\n"));
    s.push_str(&format!("pub const EXPORT_NAMES: [&str; {}] = [", names.len()));
    for n in names {
        s.push_str(&format!("\"{n}\","));
    }
    s.push_str("];\n");

    for i in 0..names.len() {
        s.push_str(&format!("static mut P{i}: usize = 0;\n"));
    }
    for (i, n) in names.iter().enumerate() {
        s.push_str(&format!(
            "#[unsafe(naked)]\n#[export_name = \"{n}\"]\npub extern \"system\" fn asi_fwd_{i}() {{ core::arch::naked_asm!(\"jmp qword ptr [rip + {{0}}]\", sym P{i}) }}\n"
        ));
    }
    s.push_str("pub unsafe fn store_ptr(i: usize, a: usize) { match i {\n");
    for i in 0..names.len() {
        s.push_str(&format!("    {i} => P{i} = a,\n"));
    }
    s.push_str("    _ => {} } }\n");

    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("exports_generated.rs");
    fs::write(&out, s).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
