use std::path::Path;

/// `generate_context!` embeds the built frontend into the binary at compile
/// time, but `tauri_build::build()` only registers the Tauri config as a build
/// input. Without the walk below, changing only files under `dist/` leaves
/// cargo believing the crate is fresh, so the binary keeps serving a stale GUI.
fn watch_frontend_dist(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());

    let Ok(entries) = std::fs::read_dir(dir) else {
        // dist/ is absent on a clean checkout; the rerun-if-changed above still
        // makes cargo rebuild once the frontend is built.
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            watch_frontend_dist(&path);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

/// Tauri decides between "load the embedded frontend" and "load `devUrl`" via
/// `dev = !custom-protocol`. A bare `cargo build --release` omits that feature,
/// so it silently produces a *dev* binary that points at http://localhost:1420
/// instead of its own bundled assets. Shipped or autostarted, that binary shows
/// whatever a stray dev server happens to serve — or a blank error page once the
/// dev server stops. Fail the build loudly instead of emitting that trap.
fn reject_dev_release_build() {
    let is_release = matches!(std::env::var("PROFILE").as_deref(), Ok("release"));
    if is_release && tauri_build::is_dev() {
        panic!(
            "\n\n\
             OrcaVoice: refusing to build a release binary in dev mode.\n\
             \n\
             This binary would ignore its embedded frontend and load devUrl\n\
             (http://localhost:1420), which fails whenever no dev server runs.\n\
             \n\
             Use the Tauri CLI, which sets the required feature:\n\
             \n\
             npm run tauri:build          # installers + exe\n\
             npm run tauri:build -- --no-bundle   # exe only, faster\n\
             \n\
             For a plain cargo build, add: --features tauri/custom-protocol\n\n"
        );
    }
}

fn main() {
    reject_dev_release_build();
    watch_frontend_dist(Path::new("../dist"));
    tauri_build::build();
}
