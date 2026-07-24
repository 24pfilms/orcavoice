#[cfg(target_os = "windows")]
mod imp {
    use std::fs;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};

    const START_SFX: &[u8] = include_bytes!("../resources/sfx-start.wav");
    const STOP_SFX: &[u8] = include_bytes!("../resources/sfx-stop.wav");

    static START_SFX_PATH: OnceLock<PathBuf> = OnceLock::new();
    static STOP_SFX_PATH: OnceLock<PathBuf> = OnceLock::new();

    pub fn play_start_tone() {
        let path = START_SFX_PATH
            .get_or_init(|| write_sfx_file("orcavoice-start.wav", START_SFX))
            .clone();
        play_file(path);
    }

    pub fn play_stop_tone() {
        let path = STOP_SFX_PATH
            .get_or_init(|| write_sfx_file("orcavoice-stop.wav", STOP_SFX))
            .clone();
        play_file(path);
    }

    fn write_sfx_file(file_name: &str, bytes: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(file_name);
        if let Err(error) = fs::write(&path, bytes) {
            eprintln!("OrcaVoice feedback sound failed: {error}");
        }
        path
    }

    fn play_file(path: PathBuf) {
        let wide_path: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            // SND_ASYNC: return immediately instead of blocking until the
            // tone finishes, so feedback never adds latency to start/stop.
            PlaySoundW(
                wide_path.as_ptr(),
                std::ptr::null_mut(),
                SND_FILENAME | SND_NODEFAULT | SND_ASYNC,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub fn play_start_tone() {}
    pub fn play_stop_tone() {}
}

pub fn play_start_tone() {
    imp::play_start_tone();
}

pub fn play_stop_tone() {
    imp::play_stop_tone();
}
