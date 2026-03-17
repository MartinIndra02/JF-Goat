use serde::Serialize;
use std::sync::mpsc;
use tauri::Emitter;

/// Commands sent from Tauri command handlers to the MPV thread.
pub enum MpvCommand {
    LoadFile { url: String, start_seconds: f64 },
    TogglePause,
    Pause,
    Resume,
    SeekRelative(f64),
    SeekAbsolute(f64),
    SetVolume(f64),
    Stop,
    Shutdown,
}

/// Emitted as Tauri event payloads for time position updates.
#[derive(Debug, Clone, Serialize)]
pub struct MpvTimeUpdate {
    pub position: f64,
    pub duration: f64,
}

/// Emitted as Tauri event payloads for pause/play state changes.
#[derive(Debug, Clone, Serialize)]
pub struct MpvStateChange {
    pub paused: bool,
}

/// Managed Tauri state for the MPV player.
pub struct MpvState {
    pub cmd_tx: mpsc::Sender<MpvCommand>,
    pub child_hwnd: isize,
}

// ── Win32 child window management ───────────────────────────────────────

#[cfg(target_os = "windows")]
pub fn create_mpv_child_window(parent_hwnd: isize) -> Result<isize, String> {
    use windows_sys::Win32::Graphics::Gdi::GetStockObject;
    use windows_sys::Win32::Graphics::Gdi::BLACK_BRUSH;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        let class_name: Vec<u16> = "JfgoatMpvHost\0".encode_utf16().collect();

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(DefWindowProcW),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(std::ptr::null()),
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: GetStockObject(BLACK_BRUSH),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };

        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            std::ptr::null(),
            WS_CHILD,
            0,
            0,
            1280,
            800,
            parent_hwnd as _,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        );

        if hwnd.is_null() {
            return Err("CreateWindowExW returned null".to_string());
        }

        Ok(hwnd as isize)
    }
}

#[cfg(target_os = "windows")]
pub fn show_mpv_window(child_hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        ShowWindow(child_hwnd as _, SW_SHOW);
        SetWindowPos(
            child_hwnd as _,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

#[cfg(target_os = "windows")]
pub fn hide_mpv_window(child_hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        ShowWindow(child_hwnd as _, SW_HIDE);
    }
}

#[cfg(target_os = "windows")]
pub fn resize_mpv_window(child_hwnd: isize, width: u32, height: u32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        SetWindowPos(
            child_hwnd as _,
            HWND_BOTTOM,
            0,
            0,
            width as i32,
            height as i32,
            SWP_NOACTIVATE,
        );
    }
}

/// Set the DLL search directory so libmpv2 can find mpv-2.dll at runtime.
#[cfg(target_os = "windows")]
pub fn set_mpv_dll_directory(resource_dir: &std::path::Path) {
    use windows_sys::Win32::System::LibraryLoader::SetDllDirectoryW;

    let wide: Vec<u16> = resource_dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        SetDllDirectoryW(wide.as_ptr());
    }
}

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;

// ── MPV thread ──────────────────────────────────────────────────────────

pub fn spawn_mpv_thread(
    child_hwnd: isize,
    cmd_rx: mpsc::Receiver<MpvCommand>,
    app_handle: tauri::AppHandle,
) {
    std::thread::Builder::new()
        .name("mpv-player".to_string())
        .spawn(move || {
            run_mpv_loop(child_hwnd, cmd_rx, app_handle);
        })
        .expect("Failed to spawn mpv thread");
}

fn run_mpv_loop(
    child_hwnd: isize,
    cmd_rx: mpsc::Receiver<MpvCommand>,
    app_handle: tauri::AppHandle,
) {
    use libmpv2::Mpv;

    let mut mpv = Mpv::new().expect("Failed to create mpv instance");

    // Render into the child HWND
    mpv.set_property("wid", child_hwnd as i64).unwrap();

    // Hardware decoding
    mpv.set_property("hwdec", "auto").unwrap();

    // Keep the window open after playback ends (for "ended" state)
    mpv.set_property("keep-open", "yes").unwrap();

    // Disable mpv's own OSD — Svelte provides controls
    mpv.set_property("osc", "no").unwrap();
    mpv.set_property("osd-level", 0i64).unwrap();
    mpv.set_property("input-default-bindings", "no").unwrap();
    mpv.set_property("input-vo-keyboard", "no").unwrap();

    // Observe properties for the event loop
    mpv.event_context_mut()
        .observe_property("time-pos", libmpv2::Format::Double, 0)
        .unwrap();
    mpv.event_context_mut()
        .observe_property("duration", libmpv2::Format::Double, 0)
        .unwrap();
    mpv.event_context_mut()
        .observe_property("pause", libmpv2::Format::Flag, 0)
        .unwrap();

    let mut time_pos: f64 = 0.0;
    let mut duration: f64 = 0.0;
    let mut last_emit = std::time::Instant::now();
    let emit_interval = std::time::Duration::from_millis(250);

    loop {
        // 1. Drain the command queue (non-blocking)
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                MpvCommand::LoadFile { url, start_seconds } => {
                    if start_seconds > 0.0 {
                        mpv.set_property("start", format!("+{}", start_seconds))
                            .ok();
                    } else {
                        mpv.set_property("start", "0").ok();
                    }
                    mpv.command("loadfile", &[&url, "replace"]).ok();
                }
                MpvCommand::TogglePause => {
                    let paused: bool = mpv.get_property("pause").unwrap_or(false);
                    mpv.set_property("pause", !paused).ok();
                }
                MpvCommand::Pause => {
                    mpv.set_property("pause", true).ok();
                }
                MpvCommand::Resume => {
                    mpv.set_property("pause", false).ok();
                }
                MpvCommand::SeekRelative(secs) => {
                    mpv.command("seek", &[&secs.to_string(), "relative"]).ok();
                }
                MpvCommand::SeekAbsolute(secs) => {
                    mpv.command("seek", &[&secs.to_string(), "absolute"]).ok();
                }
                MpvCommand::SetVolume(vol) => {
                    mpv.set_property("volume", vol).ok();
                }
                MpvCommand::Stop => {
                    mpv.command("stop", &[]).ok();
                    let _ = app_handle.emit("mpv-stopped", ());
                }
                MpvCommand::Shutdown => return,
            }
        }

        // 2. Process mpv events (50ms timeout keeps the loop responsive)
        if let Some(Ok(event)) = mpv.event_context_mut().wait_event(0.05) {
            use libmpv2::events::Event;
            use libmpv2::events::PropertyData;

            match event {
                Event::PropertyChange { name, change, .. } => match (name, change) {
                    ("time-pos", PropertyData::Double(v)) => {
                        time_pos = v;
                    }
                    ("duration", PropertyData::Double(v)) => {
                        duration = v;
                    }
                    ("pause", PropertyData::Flag(p)) => {
                        let _ = app_handle.emit("mpv-state-change", MpvStateChange { paused: p });
                    }
                    _ => {}
                },
                Event::EndFile(_reason) => {
                    let _ = app_handle.emit("mpv-file-ended", ());
                }
                _ => {}
            }
        }

        // 3. Throttled time position broadcast (~4 updates/sec)
        if last_emit.elapsed() >= emit_interval && duration > 0.0 {
            let _ = app_handle.emit(
                "mpv-time-update",
                MpvTimeUpdate {
                    position: time_pos,
                    duration,
                },
            );
            last_emit = std::time::Instant::now();
        }
    }
}
