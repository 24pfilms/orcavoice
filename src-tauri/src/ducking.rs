//! Windows Core Audio ducking/recovery.
//!
//! Ducking mutes other process sessions while recording. Recovery intentionally
//! unmutes active sessions on stop/startup so audio cannot stay stuck muted.

#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;
    use std::sync::Mutex;
    use windows_sys::core::{GUID, HRESULT};
    use windows_sys::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    static MUTED_PROCESS_IDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

    // --- COM GUIDs ---

    const CLSID_MM_DEVICE_ENUMERATOR: GUID = GUID {
        data1: 0xBCDE_0395,
        data2: 0xE52F,
        data3: 0x467C,
        data4: [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E],
    };

    const IID_IMM_DEVICE_ENUMERATOR: GUID = GUID {
        data1: 0xA956_64D2,
        data2: 0x9614,
        data3: 0x4F35,
        data4: [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6],
    };

    const IID_IAUDIO_SESSION_MANAGER2: GUID = GUID {
        data1: 0x77AA_99A0,
        data2: 0x1BD6,
        data3: 0x484F,
        data4: [0x8B, 0xC7, 0x2C, 0x65, 0x4C, 0x9A, 0x9B, 0x6F],
    };

    const IID_IAUDIO_SESSION_CONTROL2: GUID = GUID {
        data1: 0xBFB7_FF88,
        data2: 0x7239,
        data3: 0x4FC9,
        data4: [0x8F, 0xA2, 0x07, 0xC9, 0x50, 0xBE, 0x9C, 0x6D],
    };

    const IID_ISIMPLE_AUDIO_VOLUME: GUID = GUID {
        data1: 0x87CE_5498,
        data2: 0x68D6,
        data3: 0x44E5,
        data4: [0x92, 0x15, 0x6D, 0xA4, 0x7E, 0xF8, 0x83, 0xD8],
    };

    // --- COM vtable helpers ---

    type ObjPtr = *mut c_void;

    unsafe fn call_release(obj: ObjPtr) {
        if obj.is_null() {
            return;
        }
        let vtbl = *(obj as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr) -> u32 = std::mem::transmute(*vtbl.add(2));
        (func)(obj);
    }

    unsafe fn call_query_interface(obj: ObjPtr, iid: *const GUID, out: *mut ObjPtr) -> HRESULT {
        let vtbl = *(obj as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, *const GUID, *mut ObjPtr) -> HRESULT =
            std::mem::transmute(*vtbl.add(0));
        (func)(obj, iid, out)
    }

    unsafe fn get_default_endpoint(enumerator: ObjPtr) -> ObjPtr {
        let vtbl = *(enumerator as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, i32, i32, *mut ObjPtr) -> HRESULT =
            std::mem::transmute(*vtbl.add(4));
        let mut device: ObjPtr = std::ptr::null_mut();
        (func)(enumerator, 0, 0, &mut device);
        device
    }

    unsafe fn activate(device: ObjPtr, iid: *const GUID) -> ObjPtr {
        let vtbl = *(device as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, *const GUID, u32, *const c_void, *mut ObjPtr) -> HRESULT =
            std::mem::transmute(*vtbl.add(3));
        let mut out: ObjPtr = std::ptr::null_mut();
        (func)(device, iid, CLSCTX_ALL, std::ptr::null(), &mut out);
        out
    }

    unsafe fn get_session_enumerator(session_mgr: ObjPtr) -> ObjPtr {
        let vtbl = *(session_mgr as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, *mut ObjPtr) -> HRESULT =
            std::mem::transmute(*vtbl.add(5));
        let mut out: ObjPtr = std::ptr::null_mut();
        (func)(session_mgr, &mut out);
        out
    }

    unsafe fn get_session_count(session_enum: ObjPtr) -> i32 {
        let vtbl = *(session_enum as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, *mut i32) -> HRESULT =
            std::mem::transmute(*vtbl.add(3));
        let mut count: i32 = 0;
        (func)(session_enum, &mut count);
        count
    }

    unsafe fn get_session(session_enum: ObjPtr, index: i32) -> ObjPtr {
        let vtbl = *(session_enum as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, i32, *mut ObjPtr) -> HRESULT =
            std::mem::transmute(*vtbl.add(4));
        let mut out: ObjPtr = std::ptr::null_mut();
        (func)(session_enum, index, &mut out);
        out
    }

    unsafe fn get_volume(simple_vol: ObjPtr) -> f32 {
        let vtbl = *(simple_vol as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, *mut f32) -> HRESULT =
            std::mem::transmute(*vtbl.add(4));
        let mut vol: f32 = 1.0;
        (func)(simple_vol, &mut vol);
        vol
    }

    unsafe fn set_volume(simple_vol: ObjPtr, level: f32) {
        let vtbl = *(simple_vol as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, f32, *const c_void) -> HRESULT =
            std::mem::transmute(*vtbl.add(3));
        (func)(simple_vol, level, std::ptr::null());
    }

    unsafe fn get_mute(simple_vol: ObjPtr) -> bool {
        let vtbl = *(simple_vol as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, *mut i32) -> HRESULT =
            std::mem::transmute(*vtbl.add(6));
        let mut muted: i32 = 0;
        (func)(simple_vol, &mut muted);
        muted != 0
    }

    unsafe fn set_mute(simple_vol: ObjPtr, muted: bool) {
        let vtbl = *(simple_vol as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, i32, *const c_void) -> HRESULT =
            std::mem::transmute(*vtbl.add(5));
        (func)(simple_vol, i32::from(muted), std::ptr::null());
    }

    unsafe fn get_process_id(session_control2: ObjPtr) -> u32 {
        let vtbl = *(session_control2 as *const *const *const c_void);
        let func: unsafe extern "system" fn(ObjPtr, *mut u32) -> HRESULT =
            std::mem::transmute(*vtbl.add(14));
        let mut process_id: u32 = 0;
        (func)(session_control2, &mut process_id);
        process_id
    }

    struct AudioSession {
        simple_vol: ObjPtr,
        process_id: u32,
        volume: f32,
        muted: bool,
    }

    /// Enumerates all active audio render sessions.
    ///
    /// **Caller must call `CoInitializeEx` before and `CoUninitialize` after,
    /// and must `call_release` every returned `simple_vol` while COM is still alive.**
    unsafe fn enumerate_audio_sessions() -> Vec<AudioSession> {
        let mut results: Vec<AudioSession> = Vec::new();

        let mut enumerator: ObjPtr = std::ptr::null_mut();
        let hr = CoCreateInstance(
            &CLSID_MM_DEVICE_ENUMERATOR,
            std::ptr::null_mut(),
            CLSCTX_ALL,
            &IID_IMM_DEVICE_ENUMERATOR,
            &mut enumerator,
        );
        if hr < 0 || enumerator.is_null() {
            return results;
        }

        let device = get_default_endpoint(enumerator);
        if device.is_null() {
            call_release(enumerator);
            return results;
        }

        let session_mgr = activate(device, &IID_IAUDIO_SESSION_MANAGER2);
        if session_mgr.is_null() {
            call_release(device);
            call_release(enumerator);
            return results;
        }

        let session_enum = get_session_enumerator(session_mgr);
        if session_enum.is_null() {
            call_release(session_mgr);
            call_release(device);
            call_release(enumerator);
            return results;
        }

        let count = get_session_count(session_enum);
        for i in 0..count {
            let session = get_session(session_enum, i);
            if session.is_null() {
                continue;
            }
            let mut simple_vol: ObjPtr = std::ptr::null_mut();
            let mut session_control2: ObjPtr = std::ptr::null_mut();
            call_query_interface(session, &IID_ISIMPLE_AUDIO_VOLUME, &mut simple_vol);
            call_query_interface(session, &IID_IAUDIO_SESSION_CONTROL2, &mut session_control2);
            call_release(session);
            if simple_vol.is_null() || session_control2.is_null() {
                call_release(simple_vol);
                call_release(session_control2);
                continue;
            }
            let process_id = get_process_id(session_control2);
            call_release(session_control2);
            let volume = get_volume(simple_vol);
            let muted = get_mute(simple_vol);
            results.push(AudioSession {
                simple_vol,
                process_id,
                volume,
                muted,
            });
        }

        call_release(session_enum);
        call_release(session_mgr);
        call_release(device);
        call_release(enumerator);
        results
    }

    pub fn duck_audio() {
        let _ = std::thread::spawn(|| unsafe {
            let current_process_id = std::process::id();
            let mut muted_process_ids = Vec::new();

            CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32);

            let sessions = enumerate_audio_sessions();
            for session in sessions {
                if session.process_id != 0 && session.process_id != current_process_id && !session.muted {
                    set_mute(session.simple_vol, true);
                    if !muted_process_ids.contains(&session.process_id) {
                        muted_process_ids.push(session.process_id);
                    }
                }
                call_release(session.simple_vol);
            }

            CoUninitialize();
            *MUTED_PROCESS_IDS.lock().unwrap() = muted_process_ids;
        })
        .join();
    }

    pub fn unduck_audio() {
        let _ = std::thread::spawn(|| unsafe {
            let muted_process_ids = MUTED_PROCESS_IDS.lock().unwrap().clone();
            if muted_process_ids.is_empty() {
                return;
            }

            CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32);

            let sessions = enumerate_audio_sessions();
            for session in sessions {
                if muted_process_ids.contains(&session.process_id) {
                    set_mute(session.simple_vol, false);
                }
                call_release(session.simple_vol);
            }

            CoUninitialize();
            MUTED_PROCESS_IDS.lock().unwrap().clear();
        })
        .join();
    }

    pub fn restore_audio() {
        let _ = std::thread::spawn(|| unsafe {
            CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32);

            let sessions = enumerate_audio_sessions();
            for session in sessions {
                if session.muted {
                    set_mute(session.simple_vol, false);
                }
                if session.volume <= 0.001 {
                    set_volume(session.simple_vol, 1.0);
                }
                call_release(session.simple_vol);
            }

            CoUninitialize();
            MUTED_PROCESS_IDS.lock().unwrap().clear();
        })
        .join();
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub fn duck_audio() {}
    pub fn unduck_audio() {}
    pub fn restore_audio() {}
}

pub fn duck_audio() {
    imp::duck_audio();
}

pub fn unduck_audio() {
    imp::unduck_audio();
}

pub fn restore_audio() {
    imp::restore_audio();
}
