//! Small, stable C ABI used by the HarmonyOS N-API adapter.
//!
//! Flutter's generated bridge has a Dart-specific ABI and must not be called
//! directly from ArkTS. Keep this boundary limited to plain C-compatible types.

use std::{
    collections::HashMap,
    ffi::{c_char, CStr},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Condvar, Mutex,
    },
    time::Duration,
};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static RENDER_DISPLAY: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static RENDERER_RESET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PRESENTED_VIDEO_FRAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PRESENTED_VIDEO_WIDTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static PRESENTED_VIDEO_HEIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static PRESENTED_VIDEO_DISPLAY: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static MOUSE_MOVE_WORKER_STARTED: AtomicBool = AtomicBool::new(false);
lazy_static::lazy_static! {
    static ref NATIVE_RENDER_SESSION: std::sync::Mutex<(String, i32, i32)> = Default::default();
    /// Absolute pointer motion is lossy by design: only the newest coordinate
    /// per session matters. Coalescing here prevents stale moves from building
    /// up behind video/network work and producing visible cursor trails.
    static ref PENDING_MOUSE_MOVES: (Mutex<HashMap<uuid::Uuid, (i32, i32)>>, Condvar) =
        (Mutex::new(HashMap::new()), Condvar::new());
    static ref MOUSE_SEND_MUTEX: Mutex<()> = Mutex::new(());
}

#[no_mangle]
pub extern "C" fn rustdesk_ohos_report_video_output(display: usize) {
    PRESENTED_VIDEO_DISPLAY.store(display, Ordering::Relaxed);
    PRESENTED_VIDEO_FRAMES.fetch_add(1, Ordering::Relaxed);
}

#[no_mangle]
pub extern "C" fn rustdesk_ohos_report_video_format(width: u32, height: u32) {
    PRESENTED_VIDEO_WIDTH.store(width, Ordering::Relaxed);
    PRESENTED_VIDEO_HEIGHT.store(height, Ordering::Relaxed);
}

#[no_mangle]
pub extern "C" fn rustdesk_ohos_report_video_stall(display: usize) {
    crate::flutter::ohos_push_event(format!(
        "{{\"name\":\"native_video_refresh_needed\",\"display\":{display},\"reason\":\"decoder_stall\"}}"
    ));
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_get_video_stats(
    frames: *mut u64,
    width: *mut u32,
    height: *mut u32,
    display: *mut usize,
) {
    if !frames.is_null() { *frames = PRESENTED_VIDEO_FRAMES.load(Ordering::Relaxed); }
    if !width.is_null() { *width = PRESENTED_VIDEO_WIDTH.load(Ordering::Relaxed); }
    if !height.is_null() { *height = PRESENTED_VIDEO_HEIGHT.load(Ordering::Relaxed); }
    if !display.is_null() { *display = PRESENTED_VIDEO_DISPLAY.load(Ordering::Relaxed); }
}

fn send_mouse_message(session_id: uuid::Uuid, event_type: &str, x: i32, y: i32) {
    let (message_type, button) = match event_type {
        "right-down" => ("down", "right"),
        "right-up" => ("up", "right"),
        "wheel" => ("wheel", "wheel"),
        "back-down" => ("down", "back"),
        "back-up" => ("up", "back"),
        "wheel-down" => ("down", "wheel"),
        "wheel-up" => ("up", "wheel"),
        _ => (event_type, "left"),
    };
    let message = if event_type == "move" {
        format!(r#"{{"x":"{x}","y":"{y}"}}"#)
    } else {
        format!(r#"{{"type":"{message_type}","buttons":"{button}","x":"{x}","y":"{y}"}}"#)
    };
    crate::flutter_ffi::session_send_mouse(session_id, message);
}

/// Convert the paths returned by the OHOS document picker into paths accepted
/// by RustDesk's file-transfer API. The picker may return `file://` URIs and
/// percent-escaped UTF-8 names; ordinary paths remain unchanged.
fn normalize_ohos_path(value: &str) -> String {
    let value = value.strip_prefix("file://").unwrap_or(value);
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hi = (bytes[index + 1] as char).to_digit(16);
            let lo = (bytes[index + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                decoded.push((hi * 16 + lo) as u8);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(decoded).unwrap_or_else(|_| value.to_owned())
}

fn ensure_mouse_move_worker() {
    if MOUSE_MOVE_WORKER_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(|| loop {
        let (pending, ready) = &*PENDING_MOUSE_MOVES;
        let guard = pending.lock().unwrap();
        let (guard, _) = ready
            .wait_timeout_while(guard, Duration::from_millis(4), |moves| moves.is_empty())
            .unwrap();
        if guard.is_empty() {
            continue;
        }
        drop(guard);
        // Collect bursts from high-polling-rate mice into one latest-position
        // update. Do not sleep again here: the previous 8 ms wait plus this
        // second sleep added a visible 16 ms of cursor latency.
        let _send_guard = MOUSE_SEND_MUTEX.lock().unwrap();
        let moves: Vec<_> = pending.lock().unwrap().drain().collect();
        for (session_id, (x, y)) in moves {
            send_mouse_message(session_id, "move", x, y);
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_set_renderer_session(
    session_id: *const c_char, width: i32, height: i32,
) -> bool {
    if session_id.is_null() || width <= 0 || height <= 0 { return false; }
    let Ok(session_id) = CStr::from_ptr(session_id).to_str() else { return false; };
    *NATIVE_RENDER_SESSION.lock().unwrap() = (session_id.to_owned(), width, height);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_get_renderer_session(
    session_id: *mut c_char, capacity: usize, width: *mut i32, height: *mut i32,
) -> bool {
    let state = NATIVE_RENDER_SESSION.lock().unwrap();
    if state.0.is_empty() { return false; }
    copy_string(&state.0, session_id, capacity);
    if !width.is_null() { *width = state.1; }
    if !height.is_null() { *height = state.2; }
    true
}

#[no_mangle]
pub extern "C" fn rustdesk_ohos_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_initialize(app_dir: *const c_char) -> bool {
    if app_dir.is_null() {
        return false;
    }

    let Ok(app_dir) = CStr::from_ptr(app_dir).to_str() else {
        return false;
    };
    if app_dir.is_empty() {
        return false;
    }

    if !INITIALIZED.swap(true, Ordering::AcqRel) {
        crate::flutter_ffi::main_init(app_dir.to_owned(), String::new());
    }
    true
}

/// Copies the local RustDesk ID to `buffer` and always NUL-terminates it when
/// capacity is non-zero. Returns the full byte length, excluding the terminator.
#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_get_id(buffer: *mut c_char, capacity: usize) -> usize {
    let id = crate::flutter_ffi::main_get_my_id();
    let bytes = id.as_bytes();

    if !buffer.is_null() && capacity > 0 {
        let copy_len = bytes.len().min(capacity - 1);
        ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast(), copy_len);
        *buffer.add(copy_len) = 0;
    }
    bytes.len()
}

#[no_mangle]
pub extern "C" fn rustdesk_ohos_is_initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_configure_server(
    server: *const c_char,
    key: *const c_char,
) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) || server.is_null() || key.is_null() {
        return false;
    }
    let (Ok(server), Ok(key)) = (
        CStr::from_ptr(server).to_str(),
        CStr::from_ptr(key).to_str(),
    ) else {
        return false;
    };
    if server.is_empty() {
        return false;
    }
    crate::ui_interface::set_option("custom-rendezvous-server".to_owned(), server.to_owned());
    crate::ui_interface::set_option("relay-server".to_owned(), server.to_owned());
    crate::ui_interface::set_option("key".to_owned(), key.to_owned());
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_get_option(
    key: *const c_char,
    buffer: *mut c_char,
    capacity: usize,
) -> usize {
    if key.is_null() {
        return 0;
    }
    let Ok(key) = CStr::from_ptr(key).to_str() else {
        return 0;
    };
    let value = crate::ui_interface::get_option(key);
    copy_string(&value, buffer, capacity);
    value.len()
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_set_option(
    key: *const c_char,
    value: *const c_char,
) -> bool {
    if key.is_null() || value.is_null() {
        return false;
    }
    let (Ok(key), Ok(value)) = (
        CStr::from_ptr(key).to_str(),
        CStr::from_ptr(value).to_str(),
    ) else {
        return false;
    };
    if key.is_empty() {
        return false;
    }
    crate::ui_interface::set_option(key.to_owned(), value.to_owned());
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_query_onlines(ids: *const c_char) -> bool {
    if ids.is_null() { return false; }
    let Ok(ids) = CStr::from_ptr(ids).to_str() else { return false; };
    let ids = ids.split(',').map(str::trim).filter(|id| !id.is_empty()).map(str::to_owned).collect();
    crate::flutter_ffi::query_onlines(ids);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_set_image_quality(
    session_id: *const c_char,
    quality: *const c_char,
    image_quality: i32,
    fps: i32,
) -> bool {
    if quality.is_null() { return false; }
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    let Ok(quality) = CStr::from_ptr(quality).to_str() else { return false; };
    crate::flutter_ffi::session_set_image_quality(session_id, quality.to_owned());
    if quality == "custom" {
        // RustDesk 1.4.9 treats this as a quality value, not Kbps. Flutter's
        // normal range is 10..100 and its explicit "more quality" range ends
        // at 2000. Keeping the same bound also prevents the 12-bit wire value
        // from wrapping into an unintended, enormous bitrate ratio.
        crate::flutter_ffi::session_set_custom_image_quality(
            session_id,
            image_quality.clamp(10, 2000),
        );
    }
    // FPS is independent from the image-quality preset. The desktop server
    // otherwise keeps its hardware encoder default (30 FPS) even when the
    // peer negotiated a 60 FPS QoS target.
    crate::flutter_ffi::session_set_custom_fps(session_id, fps.clamp(5, 120));
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_change_resolution(
    session_id: *const c_char, display: i32, width: i32, height: i32,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    crate::flutter_ffi::session_change_resolution(session_id, display, width, height);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_change_codec(
    session_id: *const c_char, codec: *const c_char,
) -> bool {
    if codec.is_null() { return false; }
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    let Ok(codec) = CStr::from_ptr(codec).to_str() else { return false; };
    // 1.4.9 reads codec preference from the active peer's PeerConfig, not
    // from the global Config option. Persist it through the session so the
    // next SupportedDecoding message carries PreferCodec::H264/H265.
    crate::flutter_ffi::session_peer_option(
        session_id.clone(),
        "codec-preference".to_owned(),
        codec.to_owned(),
    );
    crate::flutter_ffi::session_change_prefer_codec(session_id);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_session_login(
    session_id: *const c_char,
    password: *const c_char,
    remember: bool,
) -> bool {
    if password.is_null() {
        return false;
    }
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    let Ok(password) = CStr::from_ptr(password).to_str() else { return false; };
    if password.is_empty() {
        return false;
    }
    // Match the 1.4.9 Flutter client's password-dialog path. Reusing the
    // active session is important: restarting it loses the peer's challenge
    // and can leave the video page waiting forever on a new handshake.
    crate::flutter_ffi::session_login(
        session_id,
        String::new(),
        String::new(),
        password.to_owned(),
        remember,
    );
    true
}

/// Creates a RustDesk client session and starts its network transport loop.
/// Returns 0 on success, 1 for invalid input, 2 when uninitialized and 3 when
/// the core rejects the session.
#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_start_session(
    peer_id: *const c_char,
    password: *const c_char,
    mode: *const c_char,
    session_id: *mut c_char,
    session_id_capacity: usize,
) -> i32 {
    if !INITIALIZED.load(Ordering::Acquire) {
        return 2;
    }
    if peer_id.is_null() || password.is_null() || mode.is_null() {
        return 1;
    }

    let (Ok(peer_id), Ok(password), Ok(mode)) = (
        CStr::from_ptr(peer_id).to_str(),
        CStr::from_ptr(password).to_str(),
        CStr::from_ptr(mode).to_str(),
    ) else {
        return 1;
    };
    // Keep the FFI entry point consistent with the 1.4.9 UI path: RustDesk
    // IDs are stored and negotiated without visual grouping whitespace.
    let peer_id: String = peer_id.chars().filter(|c| !c.is_whitespace()).collect();
    if peer_id.is_empty() {
        return 1;
    }

    let id = uuid::Uuid::new_v4();
    let is_file_transfer = mode == "file_transfer";
    let is_view_camera = mode == "view_camera";
    // Use the 1.4.9 automatic codec preference. SupportedDecoding advertises
    // the OHOS hardware capabilities, allowing auto negotiation to select
    // H.264 when both endpoints support it instead of forcing a codec here.
    let codec_key = "codec-preference".to_owned();
    crate::ui_interface::set_peer_option(peer_id.clone(), codec_key, "auto".to_owned());
    if crate::flutter::session_add(
        &id,
        &peer_id,
        is_file_transfer,
        is_view_camera,
        false,
        false,
        false,
        "",
        false,
        password.to_owned(),
        false,
        None,
    )
    .and_then(|_| crate::flutter::session_start_ohos(&id, &peer_id))
    .is_err()
    {
        return 3;
    }
    // Re-send the OHOS codec capability after transport startup so automatic
    // negotiation can use the freshly detected decoder capabilities.
    #[cfg(target_env = "ohos")]
    crate::flutter_ffi::session_change_prefer_codec(id.clone());

    copy_string(&id.to_string(), session_id, session_id_capacity);
    0
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_poll_event(
    buffer: *mut c_char,
    capacity: usize,
) -> usize {
    let Some(event) = crate::flutter::ohos_pop_event() else {
        return 0;
    };
    copy_string(&event, buffer, capacity);
    event.len()
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_pop_encoded_video(
    buffer: *mut u8,
    capacity: usize,
    codec: *mut c_char,
    codec_capacity: usize,
    display: *mut usize,
    key: *mut bool,
    pts: *mut i64,
) -> usize {
    let display_to_render = RENDER_DISPLAY.load(Ordering::Acquire);
    let Some(packet) = crate::flutter::ohos_pop_encoded_video(display_to_render) else { return 0; };
    // Never pass a truncated NAL unit to the native decoder. The renderer
    // normally provides a 32 MiB buffer; an undersized caller should simply
    // drop this packet and wait for the next recovery point.
    if packet.data.len() > capacity { return 0; }
    let copy_len = packet.data.len();
    if !buffer.is_null() && copy_len > 0 {
        ptr::copy_nonoverlapping(packet.data.as_ptr(), buffer, copy_len);
    }
    copy_string(&packet.codec, codec, codec_capacity);
    if !display.is_null() { *display = packet.display; }
    if !key.is_null() { *key = packet.key; }
    if !pts.is_null() { *pts = packet.pts; }
    copy_len
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_copy_frame(
    session_id: *const c_char,
    display: usize,
    buffer: *mut u8,
    capacity: usize,
) -> usize {
    if session_id.is_null() {
        return 0;
    }
    let Ok(session_id_str) = CStr::from_ptr(session_id).to_str() else {
        return 0;
    };
    let Ok(session_id) = uuid::Uuid::parse_str(session_id_str) else {
        return 0;
    };
    if !buffer.is_null() {
        return crate::flutter::session_copy_rgba(session_id, display, buffer, capacity);
    }
    crate::flutter::session_get_rgba_size(session_id, display)
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_copy_frame_strided(
    session_id: *const c_char,
    display: usize,
    buffer: *mut u8,
    capacity: usize,
    row_stride: usize,
) -> usize {
    if session_id.is_null() || buffer.is_null() {
        return 0;
    }
    let Ok(session_id) = CStr::from_ptr(session_id).to_str() else { return 0; };
    let Ok(session_id) = uuid::Uuid::parse_str(session_id) else { return 0; };
    crate::flutter::session_copy_rgba_strided(session_id, display, buffer, capacity, row_stride)
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_copy_audio(
    buffer: *mut u8,
    capacity: usize,
    sample_rate: *mut u32,
    channels: *mut u16,
) -> usize {
    let Some(chunk) = crate::flutter::ohos_pop_audio(capacity / std::mem::size_of::<i16>()) else { return 0; };
    if !sample_rate.is_null() { *sample_rate = chunk.sample_rate; }
    if !channels.is_null() { *channels = chunk.channels; }
    let bytes = chunk.pcm.len() * std::mem::size_of::<i16>();
    if !buffer.is_null() && capacity >= bytes {
        ptr::copy_nonoverlapping(chunk.pcm.as_ptr().cast::<u8>(), buffer, bytes);
    }
    bytes
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_send_mouse(
    session_id: *const c_char,
    event_type: *const c_char,
    x: i32,
    y: i32,
) -> bool {
    if session_id.is_null() || event_type.is_null() {
        return false;
    }
    let (Ok(session_id), Ok(event_type)) = (
        CStr::from_ptr(session_id).to_str(),
        CStr::from_ptr(event_type).to_str(),
    ) else {
        return false;
    };
    let Ok(session_id) = uuid::Uuid::parse_str(session_id) else {
        return false;
    };
    if event_type == "move" {
        ensure_mouse_move_worker();
        let (pending, ready) = &*PENDING_MOUSE_MOVES;
        pending.lock().unwrap().insert(session_id, (x, y));
        ready.notify_one();
        return true;
    }
    if !matches!(event_type, "down" | "up" | "right-down" | "right-up" | "wheel" |
        "back-down" | "back-up" | "wheel-down" | "wheel-up") {
        return false;
    }
    // Serialize button transitions with the coalescer and flush the newest
    // coordinate first so a click can never overtake pointer movement.
    let _send_guard = MOUSE_SEND_MUTEX.lock().unwrap();
    if let Some((move_x, move_y)) = PENDING_MOUSE_MOVES.0.lock().unwrap().remove(&session_id) {
        send_mouse_message(session_id, "move", move_x, move_y);
    }
    send_mouse_message(session_id, event_type, x, y);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_send_text(
    session_id: *const c_char,
    text: *const c_char,
) -> bool {
    if session_id.is_null() || text.is_null() {
        return false;
    }
    let (Ok(session_id), Ok(text)) = (
        CStr::from_ptr(session_id).to_str(),
        CStr::from_ptr(text).to_str(),
    ) else {
        return false;
    };
    let Ok(session_id) = uuid::Uuid::parse_str(session_id) else {
        return false;
    };
    crate::flutter_ffi::session_input_string(session_id, text.to_owned());
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_send_chat(session_id: *const c_char, text: *const c_char) -> bool {
    if text.is_null() { return false; }
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    let Ok(text) = CStr::from_ptr(text).to_str() else { return false; };
    crate::flutter_ffi::session_send_chat(session_id, text.to_owned());
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_open_terminal(session_id: *const c_char, terminal_id: i32, rows: u32, cols: u32) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    crate::flutter_ffi::session_open_terminal(session_id, terminal_id, rows, cols);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_send_terminal_input(session_id: *const c_char, terminal_id: i32, data: *const c_char) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    if data.is_null() { return false; }
    let Ok(data) = CStr::from_ptr(data).to_str() else { return false; };
    crate::flutter_ffi::session_send_terminal_input(session_id, terminal_id, data.to_owned());
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_resize_terminal(session_id: *const c_char, terminal_id: i32, rows: u32, cols: u32) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    if rows == 0 || cols == 0 { return false; }
    crate::flutter_ffi::session_resize_terminal(session_id, terminal_id, rows, cols);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_close_terminal(session_id: *const c_char, terminal_id: i32) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    crate::flutter_ffi::session_close_terminal(session_id, terminal_id);
    true
}

/// Start a file-transfer job for a local path. The full desktop client uses
/// the same core operation after the file picker returns a path; keeping this
/// small ABI lets ArkUI invoke that operation without depending on Flutter.
#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_send_file(
    session_id: *const c_char,
    path: *const c_char,
    destination: *const c_char,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    if path.is_null() || destination.is_null() { return false; }
    let (Ok(path), Ok(destination)) = (CStr::from_ptr(path).to_str(), CStr::from_ptr(destination).to_str()) else {
        return false;
    };
    if path.is_empty() || destination.is_empty() { return false; }
    let path = normalize_ohos_path(path);
    let destination = normalize_ohos_path(destination);
    crate::flutter_ffi::session_send_files(
        session_id,
        hbb_common::fs::get_next_job_id(),
        path,
        destination,
        1,
        false,
        false,
        false,
    );
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_download_remote_file(
    session_id: *const c_char,
    path: *const c_char,
    destination: *const c_char,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    if path.is_null() || destination.is_null() { return false; }
    let (Ok(path), Ok(destination)) = (CStr::from_ptr(path).to_str(), CStr::from_ptr(destination).to_str()) else {
        return false;
    };
    if path.is_empty() || destination.is_empty() { return false; }
    let path = normalize_ohos_path(path);
    let destination = normalize_ohos_path(destination);
    crate::flutter_ffi::session_send_files(
        session_id,
        hbb_common::fs::get_next_job_id(),
        path,
        destination,
        1,
        false,
        true,
        false,
    );
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_cancel_file_job(
    session_id: *const c_char,
    act_id: i32,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    crate::flutter_ffi::session_cancel_job(session_id, act_id);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_read_remote_dir(
    session_id: *const c_char,
    path: *const c_char,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    if path.is_null() { return false; }
    let Ok(path) = CStr::from_ptr(path).to_str() else { return false; };
    crate::flutter_ffi::session_read_remote_dir(session_id, path.to_owned(), false);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_remove_remote_file(
    session_id: *const c_char,
    act_id: i32,
    path: *const c_char,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    if path.is_null() { return false; }
    let Ok(path) = CStr::from_ptr(path).to_str() else { return false; };
    crate::flutter_ffi::session_remove_file(session_id, act_id, path.to_owned(), 1, true);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_rename_remote_file(
    session_id: *const c_char,
    act_id: i32,
    path: *const c_char,
    new_name: *const c_char,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    if path.is_null() || new_name.is_null() { return false; }
    let (Ok(path), Ok(new_name)) = (CStr::from_ptr(path).to_str(), CStr::from_ptr(new_name).to_str()) else { return false; };
    if new_name.is_empty() { return false; }
    crate::flutter_ffi::session_rename_file(session_id, act_id, path.to_owned(), new_name.to_owned(), true);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_refresh_session(
    session_id: *const c_char,
    display: usize,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else {
        return false;
    };
    crate::flutter_ffi::session_refresh(session_id, display);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_take_screenshot(
    session_id: *const c_char,
    display: i32,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    if display < 0 { return false; }
    crate::flutter_ffi::session_take_screenshot(session_id, display as usize);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_reconnect_session(session_id: *const c_char) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    crate::flutter_ffi::session_reconnect(session_id, false);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_switch_display(
    session_id: *const c_char,
    display: i32,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    if display < 0 { return false; }
    #[cfg(target_env = "ohos")]
    crate::flutter::ohos_reset_encoded_video_display(display as usize);
    crate::flutter_ffi::session_switch_display(true, session_id, vec![display]);
    crate::flutter_ffi::session_refresh(session_id, display as usize);
    RENDER_DISPLAY.store(display as usize, Ordering::Release);
    RENDERER_RESET.store(true, Ordering::Release);
    true
}

#[no_mangle]
pub extern "C" fn rustdesk_ohos_take_renderer_reset() -> bool {
    RENDERER_RESET.swap(false, Ordering::AcqRel)
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_get_renderer_display() -> usize {
    RENDER_DISPLAY.load(Ordering::Acquire)
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_toggle_session_option(
    session_id: *const c_char,
    option: *const c_char,
) -> bool {
    if option.is_null() {
        return false;
    }
    let Some(session_id) = parse_session_id(session_id) else {
        return false;
    };
    let Ok(option) = CStr::from_ptr(option).to_str() else {
        return false;
    };
    crate::flutter_ffi::session_toggle_option(session_id, option.to_owned());
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_set_recording(
    session_id: *const c_char,
    start: bool,
) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    crate::flutter_ffi::session_record_screen(session_id, start);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_get_session_option(
    session_id: *const c_char,
    option: *const c_char,
) -> bool {
    if option.is_null() { return false; }
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    let Ok(option) = CStr::from_ptr(option).to_str() else { return false; };
    crate::flutter_ffi::session_get_toggle_option(session_id, option.to_owned()).unwrap_or(false)
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_is_recording(session_id: *const c_char) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    crate::flutter_ffi::session_get_is_recording(session_id).0
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_ctrl_alt_del(session_id: *const c_char) -> bool {
    let Some(session_id) = parse_session_id(session_id) else {
        return false;
    };
    crate::flutter_ffi::session_ctrl_alt_del(session_id);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_lock_screen(session_id: *const c_char) -> bool {
    let Some(session_id) = parse_session_id(session_id) else { return false; };
    crate::flutter_ffi::session_lock_screen(session_id);
    true
}

#[no_mangle]
pub unsafe extern "C" fn rustdesk_ohos_close_session(session_id: *const c_char) -> bool {
    if session_id.is_null() {
        return false;
    }
    let Ok(session_id) = CStr::from_ptr(session_id).to_str() else {
        return false;
    };
    let Ok(session_id) = uuid::Uuid::parse_str(session_id) else {
        return false;
    };
    crate::flutter_ffi::session_close(session_id);
    true
}

unsafe fn copy_string(value: &str, buffer: *mut c_char, capacity: usize) {
    if buffer.is_null() || capacity == 0 {
        return;
    }
    let bytes = value.as_bytes();
    let copy_len = bytes.len().min(capacity - 1);
    ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast(), copy_len);
    *buffer.add(copy_len) = 0;
}

unsafe fn parse_session_id(session_id: *const c_char) -> Option<uuid::Uuid> {
    if session_id.is_null() {
        return None;
    }
    CStr::from_ptr(session_id).to_str().ok().and_then(|id| uuid::Uuid::parse_str(id).ok())
}
