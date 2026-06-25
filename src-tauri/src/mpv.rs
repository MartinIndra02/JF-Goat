use serde::Serialize;
use std::sync::mpsc;
use tauri::Emitter;

/// Commands sent from Tauri command handlers to the MPV thread.
pub enum MpvCommand {
    LoadFile {
        url: String,
        start_seconds: f64,
        audio_track: Option<i64>,
        subtitle_track: Option<i64>,
    },
    TogglePause,
    SeekRelative(f64),
    SeekAbsolute(f64),
    SetVolume(f64),
    SetMute(bool),
    SetRate(f64),
    SetSubtitlePosition(i64),
    SetVideoScale(String),
    SetAudioTrack(i64),
    SetSubtitleTrack(Option<i64>),
    AddSubtitle { url: String, select: bool },
    Stop,
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

/// Emitted as Tauri event payloads for mutable playback settings.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MpvPlaybackSettings {
    pub volume: f64,
    pub muted: bool,
    pub playback_rate: f64,
    pub video_scale_mode: String,
    pub audio_track: Option<i64>,
    pub subtitle_track: Option<i64>,
}

/// Managed Tauri state for the MPV player.
pub struct MpvState {
    pub cmd_tx: mpsc::Sender<MpvCommand>,
    pub child_hwnd: isize,
}

fn emit_playback_settings_if_changed(
    app: &tauri::AppHandle,
    last_emitted: &mut Option<MpvPlaybackSettings>,
    volume: f64,
    muted: bool,
    playback_rate: f64,
    video_scale_mode: &str,
    audio_track: Option<i64>,
    subtitle_track: Option<i64>,
) {
    let next = MpvPlaybackSettings {
        volume,
        muted,
        playback_rate,
        video_scale_mode: video_scale_mode.to_string(),
        audio_track,
        subtitle_track,
    };

    if last_emitted.as_ref() == Some(&next) {
        return;
    }

    let _ = app.emit("mpv-playback-settings", next.clone());
    *last_emitted = Some(next);
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
    mpv.event_context_mut()
        .observe_property("volume", libmpv2::Format::Double, 0)
        .unwrap();
    mpv.event_context_mut()
        .observe_property("mute", libmpv2::Format::Flag, 0)
        .unwrap();
    mpv.event_context_mut()
        .observe_property("speed", libmpv2::Format::Double, 0)
        .unwrap();
    mpv.event_context_mut()
        .observe_property("aid", libmpv2::Format::Int64, 0)
        .unwrap();
    mpv.event_context_mut()
        .observe_property("sid", libmpv2::Format::Int64, 0)
        .unwrap();

    let mut time_pos: f64 = 0.0;
    let mut duration: f64 = 0.0;
    let mut volume: f64 = 100.0;
    let mut muted = false;
    let mut playback_rate: f64 = 1.0;
    let mut video_scale_mode = String::from("contain");
    let mut audio_track: Option<i64> = None;
    let mut subtitle_track: Option<i64> = None;
    let mut last_emitted_settings: Option<MpvPlaybackSettings> = None;
    let mut last_emit = std::time::Instant::now();
    let emit_interval = std::time::Duration::from_millis(250);

    emit_playback_settings_if_changed(
        &app_handle,
        &mut last_emitted_settings,
        volume,
        muted,
        playback_rate,
        &video_scale_mode,
        audio_track,
        subtitle_track,
    );

    loop {
        // 1. Drain the command queue (non-blocking)
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                MpvCommand::LoadFile {
                    url,
                    start_seconds,
                    audio_track: initial_audio_track,
                    subtitle_track: initial_subtitle_track,
                } => {
                    if start_seconds > 0.0 {
                        mpv.set_property("start", format!("+{}", start_seconds))
                            .ok();
                    } else {
                        mpv.set_property("start", "0").ok();
                    }
                    mpv.command("loadfile", &[&url, "replace"]).ok();

                    if let Some(track) = initial_audio_track {
                        audio_track = Some(track);
                        mpv.set_property("aid", track).ok();
                    }

                    if let Some(track) = initial_subtitle_track {
                        subtitle_track = if track < 0 { None } else { Some(track) };
                        if track < 0 {
                            mpv.set_property("sid", -1i64).ok();
                        } else {
                            mpv.set_property("sid", track).ok();
                        }
                    }

                    emit_playback_settings_if_changed(
                        &app_handle,
                        &mut last_emitted_settings,
                        volume,
                        muted,
                        playback_rate,
                        &video_scale_mode,
                        audio_track,
                        subtitle_track,
                    );
                }
                MpvCommand::TogglePause => {
                    let paused: bool = mpv.get_property("pause").unwrap_or(false);
                    mpv.set_property("pause", !paused).ok();
                }
                MpvCommand::SeekRelative(secs) => {
                    mpv.command("seek", &[&secs.to_string(), "relative"]).ok();
                }
                MpvCommand::SeekAbsolute(secs) => {
                    mpv.command("seek", &[&secs.to_string(), "absolute"]).ok();
                }
                MpvCommand::SetVolume(vol) => {
                    mpv.set_property("volume", vol).ok();
                    volume = vol;
                    emit_playback_settings_if_changed(
                        &app_handle,
                        &mut last_emitted_settings,
                        volume,
                        muted,
                        playback_rate,
                        &video_scale_mode,
                        audio_track,
                        subtitle_track,
                    );
                }
                MpvCommand::SetMute(should_mute) => {
                    mpv.set_property("mute", should_mute).ok();
                    muted = should_mute;
                    emit_playback_settings_if_changed(
                        &app_handle,
                        &mut last_emitted_settings,
                        volume,
                        muted,
                        playback_rate,
                        &video_scale_mode,
                        audio_track,
                        subtitle_track,
                    );
                }
                MpvCommand::SetRate(rate) => {
                    let safe_rate = if rate.is_finite() {
                        rate.clamp(0.25, 3.0)
                    } else {
                        1.0
                    };
                    mpv.set_property("speed", safe_rate).ok();
                    playback_rate = safe_rate;
                    emit_playback_settings_if_changed(
                        &app_handle,
                        &mut last_emitted_settings,
                        volume,
                        muted,
                        playback_rate,
                        &video_scale_mode,
                        audio_track,
                        subtitle_track,
                    );
                }
                MpvCommand::SetSubtitlePosition(position) => {
                    let clamped = position.clamp(0, 100);
                    mpv.set_property("sub-pos", clamped).ok();
                }
                MpvCommand::SetVideoScale(mode) => {
                    match mode.as_str() {
                        "cover" => {
                            mpv.set_property("keepaspect", true).ok();
                            mpv.set_property("panscan", 1.0f64).ok();
                            video_scale_mode = "cover".to_string();
                        }
                        "stretch" => {
                            mpv.set_property("keepaspect", false).ok();
                            mpv.set_property("panscan", 0.0f64).ok();
                            video_scale_mode = "stretch".to_string();
                        }
                        _ => {
                            mpv.set_property("keepaspect", true).ok();
                            mpv.set_property("panscan", 0.0f64).ok();
                            video_scale_mode = "contain".to_string();
                        }
                    }
                    emit_playback_settings_if_changed(
                        &app_handle,
                        &mut last_emitted_settings,
                        volume,
                        muted,
                        playback_rate,
                        &video_scale_mode,
                        audio_track,
                        subtitle_track,
                    );
                }
                MpvCommand::SetAudioTrack(track) => {
                    audio_track = Some(track);
                    mpv.set_property("aid", track).ok();
                    emit_playback_settings_if_changed(
                        &app_handle,
                        &mut last_emitted_settings,
                        volume,
                        muted,
                        playback_rate,
                        &video_scale_mode,
                        audio_track,
                        subtitle_track,
                    );
                }
                MpvCommand::SetSubtitleTrack(track) => {
                    subtitle_track = track;
                    if let Some(track_idx) = track {
                        mpv.set_property("sid", track_idx).ok();
                    } else {
                        mpv.set_property("sid", "no").ok();
                    }
                    emit_playback_settings_if_changed(
                        &app_handle,
                        &mut last_emitted_settings,
                        volume,
                        muted,
                        playback_rate,
                        &video_scale_mode,
                        audio_track,
                        subtitle_track,
                    );
                }
                MpvCommand::AddSubtitle { url, select } => {
                    let flag = if select { "select" } else { "none" };
                    mpv.command("sub-add", &[&url, flag]).ok();
                }
                MpvCommand::Stop => {
                    mpv.command("stop", &[]).ok();
                    let _ = app_handle.emit("mpv-stopped", ());
                }
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
                    ("volume", PropertyData::Double(v)) => {
                        volume = v;
                        emit_playback_settings_if_changed(
                            &app_handle,
                            &mut last_emitted_settings,
                            volume,
                            muted,
                            playback_rate,
                            &video_scale_mode,
                            audio_track,
                            subtitle_track,
                        );
                    }
                    ("mute", PropertyData::Flag(v)) => {
                        muted = v;
                        emit_playback_settings_if_changed(
                            &app_handle,
                            &mut last_emitted_settings,
                            volume,
                            muted,
                            playback_rate,
                            &video_scale_mode,
                            audio_track,
                            subtitle_track,
                        );
                    }
                    ("speed", PropertyData::Double(v)) => {
                        playback_rate = v;
                        emit_playback_settings_if_changed(
                            &app_handle,
                            &mut last_emitted_settings,
                            volume,
                            muted,
                            playback_rate,
                            &video_scale_mode,
                            audio_track,
                            subtitle_track,
                        );
                    }
                    ("aid", PropertyData::Int64(v)) => {
                        audio_track = Some(v);
                        emit_playback_settings_if_changed(
                            &app_handle,
                            &mut last_emitted_settings,
                            volume,
                            muted,
                            playback_rate,
                            &video_scale_mode,
                            audio_track,
                            subtitle_track,
                        );
                    }
                    ("sid", PropertyData::Int64(v)) => {
                        subtitle_track = if v < 0 { None } else { Some(v) };
                        emit_playback_settings_if_changed(
                            &app_handle,
                            &mut last_emitted_settings,
                            volume,
                            muted,
                            playback_rate,
                            &video_scale_mode,
                            audio_track,
                            subtitle_track,
                        );
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
