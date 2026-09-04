use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    fs::OpenOptions,
    hash::{DefaultHasher, Hasher},
    io,
    io::Read,
    io::Write,
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{utils::config::Color, AppHandle, Emitter, Manager, State, WindowEvent};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use libloading::Library;

#[cfg(target_os = "windows")]
const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x0000_0100;
#[cfg(target_os = "windows")]
const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x0000_1000;

#[cfg(target_os = "windows")]
const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
#[cfg(target_os = "windows")]
const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;
#[cfg(target_os = "windows")]
const ES_CONTINUOUS: u32 = 0x8000_0000;

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "SetThreadExecutionState"]
    fn set_thread_execution_state(flags: u32) -> u32;
}

#[cfg(target_os = "macos")]
const K_CFSTRING_ENCODING_UTF8: u32 = 0x0800_0100;
#[cfg(target_os = "macos")]
const K_IOPM_ASSERTION_LEVEL_ON: u32 = 255;
#[cfg(target_os = "macos")]
const K_IO_RETURN_SUCCESS: i32 = 0;

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    #[link_name = "CFStringCreateWithCString"]
    fn cf_string_create_with_c_string(
        allocator: *const c_void,
        value: *const c_char,
        encoding: u32,
    ) -> *const c_void;

    #[link_name = "CFRelease"]
    fn cf_release(value: *const c_void);
}

#[cfg(target_os = "macos")]
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    #[link_name = "IOPMAssertionCreateWithName"]
    fn iopm_assertion_create_with_name(
        assertion_type: *const c_void,
        assertion_level: u32,
        assertion_name: *const c_void,
        assertion_id: *mut u32,
    ) -> i32;

    #[link_name = "IOPMAssertionRelease"]
    fn iopm_assertion_release(assertion_id: u32) -> i32;
}

static DOWNLOAD_CANCEL_FLAGS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

const PR_BRIDGE_PORT: u16 = 32145;
const PR_BRIDGE_PROTOCOL_VERSION: u8 = 1;
const PR_BRIDGE_MAX_REQUEST_BYTES: usize = 64 * 1024;
const PR_BRIDGE_MAX_XML_BYTES: u64 = 8 * 1024 * 1024;
const PR_BRIDGE_EVENT_NAME: &str = "pr-template-exported";

type PrBridgeState = Arc<Mutex<Option<PrBridgeRuntime>>>;
type PrBridgeInbox = Arc<Mutex<VecDeque<PrTemplateExportEvent>>>;

struct PrBridgeRuntime {
    session_id: String,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrBridgeInfo {
    service: &'static str,
    protocol_version: u8,
    page: &'static str,
    session_id: String,
    port: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrTemplateExportRequest {
    event_id: String,
    plugin_version: Option<String>,
    exported_at: Option<String>,
    template_path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrTemplateExportEvent {
    event_id: String,
    plugin_version: Option<String>,
    exported_at: Option<String>,
    template_path: String,
    project_root: String,
    xml_content: String,
}

struct PrBridgeHttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn pr_bridge_info(session_id: &str) -> PrBridgeInfo {
    PrBridgeInfo {
        service: "aicut-template-bridge",
        protocol_version: PR_BRIDGE_PROTOCOL_VERSION,
        page: "create-template",
        session_id: session_id.to_string(),
        port: PR_BRIDGE_PORT,
    }
}

fn new_pr_bridge_session_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{timestamp}", std::process::id())
}

fn http_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn read_pr_bridge_http_request(stream: &mut TcpStream) -> Result<PrBridgeHttpRequest, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;

    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8192];
    let mut expected_length = None;

    loop {
        let read_count = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read_count == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read_count]);
        if buffer.len() > PR_BRIDGE_MAX_REQUEST_BYTES {
            return Err("request is too large".to_string());
        }

        if expected_length.is_none() {
            if let Some(header_end) = http_header_end(&buffer) {
                let header_text = std::str::from_utf8(&buffer[..header_end])
                    .map_err(|_| "request headers are not UTF-8".to_string())?;
                let content_length = header_text
                    .lines()
                    .skip(1)
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.trim()
                            .eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>())
                    })
                    .transpose()
                    .map_err(|_| "invalid content-length".to_string())?
                    .unwrap_or(0);
                expected_length = Some(header_end + 4 + content_length);
            }
        }

        if expected_length.is_some_and(|length| buffer.len() >= length) {
            break;
        }
    }

    let header_end = http_header_end(&buffer).ok_or_else(|| "incomplete headers".to_string())?;
    let header_text = std::str::from_utf8(&buffer[..header_end])
        .map_err(|_| "request headers are not UTF-8".to_string())?;
    let mut lines = header_text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| "missing path".to_string())?
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let content_length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|_| "invalid content-length".to_string())?
        .unwrap_or(0);
    let body_start = header_end + 4;
    let body_end = body_start.saturating_add(content_length);
    if buffer.len() < body_end {
        return Err("incomplete request body".to_string());
    }

    Ok(PrBridgeHttpRequest {
        method,
        path,
        headers,
        body: buffer[body_start..body_end].to_vec(),
    })
}

fn write_pr_bridge_http_response(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        _ => "Internal Server Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, X-AICut-Protocol, X-AICut-Session\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| error.to_string())
}

fn pr_bridge_error_body(message: impl AsRef<str>) -> String {
    serde_json::json!({ "ok": false, "error": message.as_ref() }).to_string()
}

fn prepare_pr_template_export(
    request: PrTemplateExportRequest,
) -> Result<PrTemplateExportEvent, String> {
    let event_id = request.event_id.trim();
    if event_id.is_empty() || event_id.len() > 128 {
        return Err("eventId is invalid".to_string());
    }

    let template_path_text = request.template_path.trim();
    let template_path = PathBuf::from(template_path_text);
    if !template_path.is_absolute() {
        return Err("templatePath must be absolute".to_string());
    }
    if template_path.file_name().and_then(|value| value.to_str()) != Some("template.xml") {
        return Err("templatePath must point to template.xml".to_string());
    }

    let metadata = fs::metadata(&template_path)
        .map_err(|error| format!("template.xml is unavailable: {error}"))?;
    if !metadata.is_file() {
        return Err("templatePath is not a file".to_string());
    }
    if metadata.len() > PR_BRIDGE_MAX_XML_BYTES {
        return Err("template.xml is too large".to_string());
    }

    let canonical_template = fs::canonicalize(&template_path)
        .map_err(|error| format!("failed to resolve template.xml: {error}"))?;
    let canonical_root = canonical_template
        .parent()
        .ok_or_else(|| "template.xml has no project directory".to_string())?;
    if !canonical_root.join("assets").is_dir() {
        return Err("the exported project is missing its assets directory".to_string());
    }

    let xml_content = fs::read_to_string(&canonical_template)
        .map_err(|error| format!("failed to read template.xml: {error}"))?;
    if !xml_content.contains("<template") {
        return Err("template.xml does not contain a template node".to_string());
    }

    let project_root = template_path
        .parent()
        .ok_or_else(|| "template.xml has no project directory".to_string())?
        .to_string_lossy()
        .to_string();

    Ok(PrTemplateExportEvent {
        event_id: event_id.to_string(),
        plugin_version: request.plugin_version,
        exported_at: request.exported_at,
        template_path: template_path_text.to_string(),
        project_root,
        xml_content,
    })
}

fn handle_pr_bridge_connection(
    mut stream: TcpStream,
    app: &AppHandle,
    info: &PrBridgeInfo,
    processed_event_ids: &Arc<Mutex<HashSet<String>>>,
) {
    let request = match read_pr_bridge_http_request(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            let status = if error == "request is too large" {
                413
            } else {
                400
            };
            let _ =
                write_pr_bridge_http_response(&mut stream, status, &pr_bridge_error_body(error));
            return;
        }
    };

    if request.method == "OPTIONS" {
        let _ = write_pr_bridge_http_response(&mut stream, 204, "");
        return;
    }

    if request.method == "GET" && request.path == "/v1/health" {
        let body = serde_json::to_string(info).unwrap_or_else(|_| "{}".to_string());
        let _ = write_pr_bridge_http_response(&mut stream, 200, &body);
        return;
    }

    if request.method != "POST" || request.path != "/v1/template-exports" {
        let _ = write_pr_bridge_http_response(
            &mut stream,
            404,
            &pr_bridge_error_body("endpoint not found"),
        );
        return;
    }

    let protocol_ok = request
        .headers
        .get("x-aicut-protocol")
        .is_some_and(|value| value == &PR_BRIDGE_PROTOCOL_VERSION.to_string());
    let session_ok = request
        .headers
        .get("x-aicut-session")
        .is_some_and(|value| value == &info.session_id);
    if !protocol_ok || !session_ok {
        let _ = write_pr_bridge_http_response(
            &mut stream,
            401,
            &pr_bridge_error_body("bridge protocol or session is invalid"),
        );
        return;
    }

    let content_type_ok = request
        .headers
        .get("content-type")
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("application/json"));
    if !content_type_ok {
        let _ = write_pr_bridge_http_response(
            &mut stream,
            400,
            &pr_bridge_error_body("content-type must be application/json"),
        );
        return;
    }

    let export_request = match serde_json::from_slice::<PrTemplateExportRequest>(&request.body) {
        Ok(request) => request,
        Err(error) => {
            let _ = write_pr_bridge_http_response(
                &mut stream,
                400,
                &pr_bridge_error_body(format!("invalid JSON: {error}")),
            );
            return;
        }
    };

    let export_event = match prepare_pr_template_export(export_request) {
        Ok(event) => event,
        Err(error) => {
            let _ = write_pr_bridge_http_response(&mut stream, 422, &pr_bridge_error_body(error));
            return;
        }
    };

    let duplicate = match processed_event_ids.lock() {
        Ok(mut event_ids) => {
            if event_ids.contains(&export_event.event_id) {
                true
            } else {
                if event_ids.len() >= 256 {
                    event_ids.clear();
                }
                event_ids.insert(export_event.event_id.clone());
                false
            }
        }
        Err(_) => {
            let _ = write_pr_bridge_http_response(
                &mut stream,
                500,
                &pr_bridge_error_body("failed to lock bridge event state"),
            );
            return;
        }
    };

    if !duplicate {
        let pending_count = match app.try_state::<PrBridgeInbox>() {
            Some(inbox) => match inbox.lock() {
                Ok(mut inbox) => {
                    if inbox.len() >= 16 {
                        inbox.pop_front();
                    }
                    inbox.push_back(export_event.clone());
                    inbox.len()
                }
                Err(_) => {
                    let _ = write_pr_bridge_http_response(
                        &mut stream,
                        500,
                        &pr_bridge_error_body("failed to lock bridge inbox"),
                    );
                    return;
                }
            },
            None => {
                let _ = write_pr_bridge_http_response(
                    &mut stream,
                    500,
                    &pr_bridge_error_body("bridge inbox is unavailable"),
                );
                return;
            }
        };
        app_log_info(format!(
            "[pr-bridge] accepted event={} template={} pending={pending_count}",
            export_event.event_id, export_event.template_path
        ));
        if let Err(error) = app.emit(PR_BRIDGE_EVENT_NAME, export_event.clone()) {
            let _ = write_pr_bridge_http_response(
                &mut stream,
                500,
                &pr_bridge_error_body(format!("failed to notify the app: {error}")),
            );
            return;
        }
    }

    let body = serde_json::json!({
        "ok": true,
        "accepted": true,
        "duplicate": duplicate,
        "eventId": export_event.event_id,
    })
    .to_string();
    let _ = write_pr_bridge_http_response(&mut stream, 200, &body);
}

fn run_pr_bridge_server(
    listener: TcpListener,
    app: AppHandle,
    info: PrBridgeInfo,
    running: Arc<AtomicBool>,
) {
    let processed_event_ids = Arc::new(Mutex::new(HashSet::new()));
    app_log_info(format!(
        "[pr-bridge] listening on 127.0.0.1:{} session={}",
        info.port, info.session_id
    ));
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                handle_pr_bridge_connection(stream, &app, &info, &processed_event_ids)
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(40));
            }
            Err(error) => {
                app_log_error(format!("[pr-bridge] accept failed: {error}"));
                break;
            }
        }
    }
    app_log_info("[pr-bridge] stopped");
}

fn shutdown_pr_bridge(runtime: &mut PrBridgeRuntime) {
    runtime.running.store(false, Ordering::SeqCst);
    if let Some(thread) = runtime.thread.take() {
        let _ = thread.join();
    }
}

#[tauri::command]
fn start_pr_bridge(
    app: AppHandle,
    state: State<'_, PrBridgeState>,
    inbox: State<'_, PrBridgeInbox>,
) -> Result<PrBridgeInfo, String> {
    let mut runtime = state
        .lock()
        .map_err(|_| "failed to lock PR bridge state".to_string())?;
    if let Some(existing) = runtime.as_mut() {
        shutdown_pr_bridge(existing);
        *runtime = None;
    }
    inbox
        .lock()
        .map_err(|_| "failed to lock PR bridge inbox".to_string())?
        .clear();

    let listener = TcpListener::bind(("127.0.0.1", PR_BRIDGE_PORT))
        .map_err(|error| format!("无法监听 127.0.0.1:{PR_BRIDGE_PORT}：{error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("无法配置 PR 对接服务：{error}"))?;

    let session_id = new_pr_bridge_session_id();
    let info = pr_bridge_info(&session_id);
    let running = Arc::new(AtomicBool::new(true));
    let thread_running = running.clone();
    let thread_info = info.clone();
    let server_thread = thread::Builder::new()
        .name("aicut-pr-bridge".to_string())
        .spawn(move || run_pr_bridge_server(listener, app, thread_info, thread_running))
        .map_err(|error| format!("无法启动 PR 对接服务线程：{error}"))?;

    *runtime = Some(PrBridgeRuntime {
        session_id,
        running,
        thread: Some(server_thread),
    });
    Ok(info)
}

#[tauri::command]
fn stop_pr_bridge(session_id: String, state: State<'_, PrBridgeState>) -> Result<(), String> {
    let mut runtime = state
        .lock()
        .map_err(|_| "failed to lock PR bridge state".to_string())?;
    let Some(active) = runtime.as_mut() else {
        return Ok(());
    };
    if active.session_id != session_id {
        return Ok(());
    }
    shutdown_pr_bridge(active);
    *runtime = None;
    Ok(())
}

#[tauri::command]
fn take_pr_template_exports(
    inbox: State<'_, PrBridgeInbox>,
) -> Result<Vec<PrTemplateExportEvent>, String> {
    let mut inbox = inbox
        .lock()
        .map_err(|_| "failed to lock PR bridge inbox".to_string())?;
    let exports = inbox.drain(..).collect::<Vec<_>>();
    if !exports.is_empty() {
        app_log_info(format!(
            "[pr-bridge] frontend took {} pending export(s)",
            exports.len()
        ));
    }
    Ok(exports)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreparedTemplate {
    template_dir: String,
    template_file_path: String,
    material_package_path: String,
    assets_dir: String,
    xml_content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWorkspace {
    project_dir: String,
    template_file_path: String,
    project_xml: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectAssetImport {
    copied_path: String,
    project_filepath: String,
    project_xml: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectGeneratedAsset {
    generate_path: String,
    project_xml: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalProjectWorkspace {
    project_dir: String,
    template_file_path: String,
    assets_dir: String,
    template_xml: String,
    project_file_xml: String,
    existing_asset_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectAreaOffsetUpdate {
    area_id: String,
    offset_ms: u64,
}

#[derive(Deserialize)]
struct ProjectAssetProperties {
    whiteness: f64,
    smoothing: f64,
    saturation: f64,
    skin_tone: f64,
    face_detect: i32,
    rotation: f64,
    lut_style: String,
    lut_intensity: f64,
    #[serde(rename = "positionX")]
    position_x: f64,
    #[serde(rename = "positionY")]
    position_y: f64,
    scale: f64,
    canvas_width: u32,
    canvas_height: u32,
    transform_origin: String,
    stabilization: bool,
    one_click_beauty: bool,
    #[serde(default)]
    generatepath: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateDownloadProgress {
    download_id: String,
    progress: u8,
    status: String,
    phase: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    resumed_bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComposerExportProgress {
    export_id: String,
    progress: u8,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComposerExportResult {
    output_path: String,
}

#[derive(Deserialize, Serialize, Default)]
struct ComposerBeautyFrameParams {
    whiteness: f64,
    smoothing: f64,
    saturation: f64,
    skin_tone: f64,
    face_detect: i32,
    rotation: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    lut_file: Option<String>,
    lut_intensity: f64,
    #[serde(rename = "positionX")]
    position_x: f64,
    #[serde(rename = "positionY")]
    position_y: f64,
    scale: f64,
    canvas_width: u32,
    canvas_height: u32,
    transform_origin: String,
    stabilization: bool,
    one_click_beauty: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComposerBeautyFrameResult {
    output_image_path: String,
    params_json_path: String,
    timestamp_ms: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComposerBeautyFileResult {
    output_video_path: String,
    params_json_path: String,
    start_time_ms: i64,
    duration_ms: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalInfo {
    terminal_type: u8,
    terminal_name: String,
}

struct ExportWakeGuard {
    #[cfg(target_os = "windows")]
    active: bool,
    #[cfg(target_os = "macos")]
    assertion_id: Option<u32>,
}

impl ExportWakeGuard {
    fn acquire() -> Result<Self, String> {
        #[cfg(target_os = "windows")]
        {
            let flags = ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED;
            let previous_state = unsafe { set_thread_execution_state(flags) };
            if previous_state == 0 {
                return Err(format!(
                    "SetThreadExecutionState failed: {}",
                    io::Error::last_os_error()
                ));
            }

            Ok(Self { active: true })
        }

        #[cfg(target_os = "macos")]
        {
            let assertion_type =
                CString::new("PreventUserIdleDisplaySleep").map_err(|error| error.to_string())?;
            let assertion_name =
                CString::new("AICut video export").map_err(|error| error.to_string())?;

            let assertion_type = unsafe {
                cf_string_create_with_c_string(
                    std::ptr::null(),
                    assertion_type.as_ptr(),
                    K_CFSTRING_ENCODING_UTF8,
                )
            };
            if assertion_type.is_null() {
                return Err("Failed to create macOS power assertion type".to_string());
            }

            let assertion_name = unsafe {
                cf_string_create_with_c_string(
                    std::ptr::null(),
                    assertion_name.as_ptr(),
                    K_CFSTRING_ENCODING_UTF8,
                )
            };
            if assertion_name.is_null() {
                unsafe { cf_release(assertion_type) };
                return Err("Failed to create macOS power assertion name".to_string());
            }

            let mut assertion_id = 0_u32;
            let result = unsafe {
                iopm_assertion_create_with_name(
                    assertion_type,
                    K_IOPM_ASSERTION_LEVEL_ON,
                    assertion_name,
                    &mut assertion_id,
                )
            };
            unsafe {
                cf_release(assertion_name);
                cf_release(assertion_type);
            }

            if result != K_IO_RETURN_SUCCESS {
                return Err(format!(
                    "IOPMAssertionCreateWithName failed: 0x{:08x}",
                    result as u32
                ));
            }

            Ok(Self {
                assertion_id: Some(assertion_id),
            })
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            Ok(Self {})
        }
    }

    fn is_active(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            self.active
        }

        #[cfg(target_os = "macos")]
        {
            self.assertion_id.is_some()
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            false
        }
    }
}

impl Drop for ExportWakeGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if self.active {
                let previous_state = unsafe { set_thread_execution_state(ES_CONTINUOUS) };
                if previous_state == 0 {
                    app_log_error(format!(
                        "[power] failed to release Windows export wake lock: {}",
                        io::Error::last_os_error()
                    ));
                } else {
                    app_log_info("[power] released Windows export wake lock");
                }
                self.active = false;
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(assertion_id) = self.assertion_id.take() {
                let result = unsafe { iopm_assertion_release(assertion_id) };
                if result != K_IO_RETURN_SUCCESS {
                    app_log_error(format!(
                        "[power] failed to release macOS export wake lock: 0x{:08x}",
                        result as u32
                    ));
                } else {
                    app_log_info("[power] released macOS export wake lock");
                }
            }
        }
    }
}

type ComposerState = Arc<Mutex<ComposerRuntime>>;

struct ComposerRuntime {
    init_error: Option<String>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    _library: Option<Library>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    compose: Option<ComposerComposeFn>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    cleanup: Option<ComposerCleanupFn>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    get_last_error: Option<ComposerGetLastErrorFn>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    get_last_cmd: Option<ComposerGetLastCmdFn>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    beauty_process_frame: Option<ComposerBeautyProcessFrameFn>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    beauty_process_file: Option<ComposerBeautyProcessFileFn>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    beauty_get_last_error: Option<ComposerBeautyGetLastErrorFn>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    initialized: bool,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
type ComposerInitFn = unsafe extern "C" fn(*const c_char) -> c_int;
#[cfg(any(target_os = "macos", target_os = "windows"))]
type ComposerCleanupFn = unsafe extern "C" fn();
#[cfg(any(target_os = "macos", target_os = "windows"))]
type ComposerProgressCallback = extern "C" fn(c_int, c_int, *const c_char, *mut c_void);
#[cfg(any(target_os = "macos", target_os = "windows"))]
type ComposerComposeFn = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    *const c_char,
    Option<ComposerProgressCallback>,
    *mut c_void,
) -> c_int;
#[cfg(any(target_os = "macos", target_os = "windows"))]
type ComposerGetLastErrorFn = unsafe extern "C" fn(*mut c_void) -> *const c_char;
#[cfg(any(target_os = "macos", target_os = "windows"))]
type ComposerGetLastCmdFn = unsafe extern "C" fn() -> *const c_char;
#[cfg(any(target_os = "macos", target_os = "windows"))]
type ComposerBeautyProcessFrameFn =
    unsafe extern "C" fn(*const c_char, i64, *const c_char, *const c_char) -> c_int;
#[cfg(any(target_os = "macos", target_os = "windows"))]
type ComposerBeautyProcessFileFn =
    unsafe extern "C" fn(*const c_char, *const c_char, i64, i64, *const c_char) -> c_int;
#[cfg(any(target_os = "macos", target_os = "windows"))]
type ComposerBeautyGetLastErrorFn = unsafe extern "C" fn() -> *const c_char;

#[cfg(any(target_os = "macos", target_os = "windows"))]
struct ComposerCallbackContext {
    app: AppHandle,
    export_id: String,
}

fn download_tasks() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    DOWNLOAD_CANCEL_FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_download_task(download_id: &str) -> Result<Arc<AtomicBool>, String> {
    let flag = Arc::new(AtomicBool::new(false));
    let mut tasks = download_tasks().lock().map_err(|error| error.to_string())?;
    tasks.insert(download_id.to_string(), flag.clone());
    Ok(flag)
}

fn remove_download_task(download_id: &str) -> Result<(), String> {
    let mut tasks = download_tasks().lock().map_err(|error| error.to_string())?;
    tasks.remove(download_id);
    Ok(())
}

fn ensure_not_cancelled(cancel_flag: &AtomicBool) -> Result<(), String> {
    if cancel_flag.load(Ordering::Relaxed) {
        Err("Download canceled".to_string())
    } else {
        Ok(())
    }
}

fn emit_progress(app: &AppHandle, download_id: &str, progress: u8, status: &str) {
    emit_transfer_progress(app, download_id, progress, status, "", 0, None, 0);
}

#[allow(clippy::too_many_arguments)]
fn emit_transfer_progress(
    app: &AppHandle,
    download_id: &str,
    progress: u8,
    status: &str,
    phase: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    resumed_bytes: u64,
) {
    let payload = TemplateDownloadProgress {
        download_id: download_id.to_string(),
        progress: progress.min(100),
        status: status.to_string(),
        phase: phase.to_string(),
        downloaded_bytes,
        total_bytes,
        resumed_bytes,
    };
    let _ = app.emit("template-download-progress", payload);
}

fn emit_composer_progress(app: &AppHandle, export_id: &str, progress: u8, status: &str) {
    app_log_info(format!(
        "[composer] progress export_id={export_id} progress={progress} status={status}"
    ));
    let payload = ComposerExportProgress {
        export_id: export_id.to_string(),
        progress: progress.min(100),
        status: status.to_string(),
    };
    let _ = app.emit("composer-export-progress", payload);
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn composer_error_message(code: i32) -> String {
    match code {
        0 => "合成成功".to_string(),
        -1 => "XML 文件无效".to_string(),
        -2 => "文件未找到".to_string(),
        -3 => "MLT 初始化失败".to_string(),
        -4 => "视频合成失败".to_string(),
        -5 => "合成已取消".to_string(),
        value => format!("Composer 调用失败，错误码 {value}"),
    }
}

impl ComposerRuntime {
    fn initialize() -> Self {
        match Self::try_initialize() {
            Ok(runtime) => runtime,
            Err(error) => {
                app_log_error(format!(
                    "[composer] initialization failed but app will continue: {error}"
                ));
                Self::disabled(error)
            }
        }
    }

    fn disabled(error: String) -> Self {
        Self {
            init_error: Some(error),
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            _library: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            compose: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            cleanup: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            get_last_error: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            get_last_cmd: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            beauty_process_frame: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            beauty_process_file: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            beauty_get_last_error: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            initialized: false,
        }
    }

    fn try_initialize() -> Result<Self, String> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            app_log_info("[composer] initializing runtime");
            let library_path = composer_library_path()?;
            app_log_info(format!(
                "[composer] loading dynamic library: {}",
                library_path.display()
            ));
            #[cfg(target_os = "macos")]
            let beauty_resource_path = {
                let library_resource_path = library_path
                    .parent()
                    .map(|directory| directory.join("share").join("composer"));
                let current_exe = std::env::current_exe().ok();
                let dev_resource_path = current_exe.as_ref().and_then(|exe| {
                    exe.parent()
                        .map(|directory| directory.join("share").join("composer"))
                });
                let bundled_resource_path = current_exe.as_ref().and_then(|exe| {
                    exe.parent()
                        .and_then(|macos_dir| macos_dir.parent())
                        .map(|contents_dir| {
                            contents_dir
                                .join("Resources")
                                .join("share")
                                .join("composer")
                        })
                });
                [
                    library_resource_path,
                    dev_resource_path,
                    bundled_resource_path,
                ]
                .into_iter()
                .flatten()
                .find_map(|path| fs::canonicalize(path).ok())
                .map(path_to_xml_filepath)
                .ok_or_else(|| "Composer 美颜资源目录 share/composer 不可用".to_string())?
            };
            #[cfg(target_os = "windows")]
            let beauty_resource_path = String::new();
            app_log_info(format!(
                "[composer] beauty resource path: {}",
                if beauty_resource_path.is_empty() {
                    "<auto>"
                } else {
                    &beauty_resource_path
                }
            ));
            let library = load_composer_library(&library_path)
                .map_err(|error| format!("加载 Composer 动态库失败: {error}"))?;
            app_log_info("[composer] resolving composer_init");
            let init: ComposerInitFn = unsafe {
                *library
                    .get(b"composer_init\0")
                    .map_err(|error| format!("读取 composer_init 失败: {error}"))?
            };
            app_log_info("[composer] resolving composer_compose");
            let compose: ComposerComposeFn = unsafe {
                *library
                    .get(b"composer_compose\0")
                    .map_err(|error| format!("读取 composer_compose 失败: {error}"))?
            };
            app_log_info("[composer] resolving composer_cleanup");
            let cleanup: ComposerCleanupFn = unsafe {
                *library
                    .get(b"composer_cleanup\0")
                    .map_err(|error| format!("读取 composer_cleanup 失败: {error}"))?
            };
            app_log_info("[composer] resolving composer_get_last_error");
            let get_last_error: Option<ComposerGetLastErrorFn> = unsafe {
                match library.get(b"composer_get_last_error\0") {
                    Ok(symbol) => {
                        app_log_info("[composer] composer_get_last_error resolved");
                        Some(*symbol)
                    }
                    Err(error) => {
                        app_log_error(format!(
                            "[composer] composer_get_last_error unavailable: {error}"
                        ));
                        None
                    }
                }
            };
            app_log_info("[composer] resolving composer_get_last_cmd");
            let get_last_cmd: Option<ComposerGetLastCmdFn> = unsafe {
                match library.get(b"composer_get_last_cmd\0") {
                    Ok(symbol) => {
                        app_log_info("[composer] composer_get_last_cmd resolved");
                        Some(*symbol)
                    }
                    Err(error) => {
                        app_log_error(format!(
                            "[composer] composer_get_last_cmd unavailable: {error}"
                        ));
                        None
                    }
                }
            };
            app_log_info("[composer] resolving composer_beauty_process_frame");
            let beauty_process_frame: Option<ComposerBeautyProcessFrameFn> = unsafe {
                match library.get(b"composer_beauty_process_frame\0") {
                    Ok(symbol) => {
                        app_log_info("[composer] composer_beauty_process_frame resolved");
                        Some(*symbol)
                    }
                    Err(error) => {
                        app_log_error(format!(
                            "[composer] composer_beauty_process_frame unavailable: {error}"
                        ));
                        None
                    }
                }
            };
            app_log_info("[composer] resolving composer_beauty_process_file");
            let beauty_process_file: Option<ComposerBeautyProcessFileFn> = unsafe {
                match library.get(b"composer_beauty_process_file\0") {
                    Ok(symbol) => {
                        app_log_info("[composer] composer_beauty_process_file resolved");
                        Some(*symbol)
                    }
                    Err(error) => {
                        app_log_error(format!(
                            "[composer] composer_beauty_process_file unavailable: {error}"
                        ));
                        None
                    }
                }
            };
            app_log_info("[composer] resolving composer_beauty_get_last_error");
            let beauty_get_last_error: Option<ComposerBeautyGetLastErrorFn> = unsafe {
                match library.get(b"composer_beauty_get_last_error\0") {
                    Ok(symbol) => {
                        app_log_info("[composer] composer_beauty_get_last_error resolved");
                        Some(*symbol)
                    }
                    Err(error) => {
                        app_log_error(format!(
                            "[composer] composer_beauty_get_last_error unavailable: {error}"
                        ));
                        None
                    }
                }
            };
            app_log_info("[composer] calling composer_init");
            let beauty_resource_path = CString::new(beauty_resource_path)
                .map_err(|_| "GPU Pixel 资源路径包含非法字符".to_string())?;
            let init_result = unsafe { init(beauty_resource_path.as_ptr()) };

            if init_result != 0 {
                app_log_error(format!(
                    "[composer] composer_init failed: {}",
                    composer_error_message(init_result)
                ));
                return Err(composer_error_message(init_result));
            }
            app_log_info("[composer] composer_init success");

            Ok(Self {
                init_error: None,
                _library: Some(library),
                compose: Some(compose),
                cleanup: Some(cleanup),
                get_last_error,
                get_last_cmd,
                beauty_process_frame,
                beauty_process_file,
                beauty_get_last_error,
                initialized: true,
            })
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            app_log_info("[composer] runtime is disabled on this platform");
            Ok(Self {
                init_error: Some("Composer 动态库当前只支持 macOS 和 Windows".to_string()),
            })
        }
    }

    fn compose_video(
        &self,
        template_path: &str,
        project_path: &str,
        output_path: &str,
        app: AppHandle,
        export_id: String,
    ) -> Result<(), String> {
        if let Some(error) = &self.init_error {
            app_log_error(format!(
                "[composer] compose skipped because runtime is unavailable: {error}"
            ));
            return Err(error.clone());
        }

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            app_log_info(format!("[composer] compose start export_id={export_id}"));
            app_log_info(format!("[composer] template_path={template_path}"));
            app_log_info(format!("[composer] project_path={project_path}"));
            app_log_info(format!("[composer] output_path={output_path}"));
            let template_path_text = template_path.to_string();
            let project_path_text = project_path.to_string();
            let output_path_text = output_path.to_string();
            let export_id_text = export_id.clone();
            let Some(compose) = self.compose else {
                let error = "composer_compose 函数未加载".to_string();
                app_log_error(format!("[composer] {error}"));
                return Err(error);
            };
            let template_path =
                CString::new(template_path).map_err(|_| "模板路径包含非法字符".to_string())?;
            let project_path =
                CString::new(project_path).map_err(|_| "工程路径包含非法字符".to_string())?;
            let output_path =
                CString::new(output_path).map_err(|_| "输出路径包含非法字符".to_string())?;
            let mut context = ComposerCallbackContext { app, export_id };
            let result = unsafe {
                compose(
                    template_path.as_ptr(),
                    project_path.as_ptr(),
                    output_path.as_ptr(),
                    Some(composer_progress_callback),
                    (&mut context as *mut ComposerCallbackContext).cast::<c_void>(),
                )
            };

            if result == 0 {
                app_log_info("[composer] compose success");
                Ok(())
            } else {
                let error_message = composer_error_message(result);
                let composer_last_error = self.composer_last_error_text();
                let composer_last_cmd = self.composer_last_cmd_text();
                app_log_error(format!("[composer] compose failed: {error_message}"));
                app_log_error(format!(
                    "[composer] composer_get_last_error(NULL): {composer_last_error}"
                ));
                app_log_error(format!(
                    "[composer] composer_get_last_cmd(): {composer_last_cmd}"
                ));
                append_composer_error_log(&format!(
                    "export_id: {export_id_text}\n\
                     template_path: {template_path_text}\n\
                     project_path: {project_path_text}\n\
                     output_path: {output_path_text}\n\
                     error_code: {result}\n\
                     error_message: {error_message}\n\
                     composer_get_last_error(NULL): {composer_last_error}\n\
                     composer_get_last_cmd(): {composer_last_cmd}"
                ));

                if composer_last_error.trim().is_empty()
                    || composer_last_error == "composer_get_last_error 函数未加载"
                    || composer_last_error == "composer_get_last_error 返回空指针"
                {
                    Err(error_message)
                } else {
                    Err(format!("{error_message}: {composer_last_error}"))
                }
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (template_path, project_path, output_path, app, export_id);
            app_log_error("[composer] compose requested on unsupported platform");
            Err("Composer 动态库当前只支持 macOS 和 Windows".to_string())
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn composer_last_error_text(&self) -> String {
        let Some(get_last_error) = self.get_last_error else {
            return "composer_get_last_error 函数未加载".to_string();
        };

        let error = unsafe { get_last_error(std::ptr::null_mut()) };
        if error.is_null() {
            return "composer_get_last_error 返回空指针".to_string();
        }

        unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .trim()
            .to_string()
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn composer_last_cmd_text(&self) -> String {
        let Some(get_last_cmd) = self.get_last_cmd else {
            return "composer_get_last_cmd function is not loaded".to_string();
        };

        let cmd = unsafe { get_last_cmd() };
        if cmd.is_null() {
            return "composer_get_last_cmd returned null".to_string();
        }

        unsafe { CStr::from_ptr(cmd) }
            .to_string_lossy()
            .trim()
            .to_string()
    }

    fn beauty_process_frame(
        &self,
        input_video_path: &str,
        timestamp_ms: i64,
        output_image_path: &str,
        json_params: &str,
    ) -> Result<(), String> {
        if let Some(error) = &self.init_error {
            return Err(error.clone());
        }

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let Some(process_frame) = self.beauty_process_frame else {
                return Err("composer_beauty_process_frame 函数未加载".to_string());
            };
            let input_video_path = CString::new(input_video_path)
                .map_err(|_| "输入视频路径包含非法字符".to_string())?;
            let output_image_path = CString::new(output_image_path)
                .map_err(|_| "预览图片路径包含非法字符".to_string())?;
            let json_params =
                CString::new(json_params).map_err(|_| "美颜参数包含非法字符".to_string())?;
            let result = unsafe {
                process_frame(
                    input_video_path.as_ptr(),
                    timestamp_ms,
                    output_image_path.as_ptr(),
                    json_params.as_ptr(),
                )
            };

            if result == 0 {
                return Ok(());
            }

            let last_error = self.beauty_last_error_text();
            if last_error.is_empty() {
                Err(composer_error_message(result))
            } else {
                Err(format!("{}: {last_error}", composer_error_message(result)))
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (
                input_video_path,
                timestamp_ms,
                output_image_path,
                json_params,
            );
            Err("Composer 动态库当前只支持 macOS 和 Windows".to_string())
        }
    }

    fn beauty_process_file(
        &self,
        input_video_path: &str,
        output_video_path: &str,
        start_time_ms: i64,
        duration_ms: i64,
        json_params: &str,
    ) -> Result<(), String> {
        if let Some(error) = &self.init_error {
            return Err(error.clone());
        }

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let Some(process_file) = self.beauty_process_file else {
                return Err("composer_beauty_process_file 函数未加载".to_string());
            };
            let input_video_path = CString::new(input_video_path)
                .map_err(|_| "输入视频路径包含非法字符".to_string())?;
            let output_video_path = CString::new(output_video_path)
                .map_err(|_| "预览视频路径包含非法字符".to_string())?;
            let json_params =
                CString::new(json_params).map_err(|_| "美颜参数包含非法字符".to_string())?;
            let result = unsafe {
                process_file(
                    input_video_path.as_ptr(),
                    output_video_path.as_ptr(),
                    start_time_ms,
                    duration_ms,
                    json_params.as_ptr(),
                )
            };

            if result == 0 {
                return Ok(());
            }

            let last_error = self.beauty_last_error_text();
            if last_error.is_empty() {
                Err(composer_error_message(result))
            } else {
                Err(format!("{}: {last_error}", composer_error_message(result)))
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (
                input_video_path,
                output_video_path,
                start_time_ms,
                duration_ms,
                json_params,
            );
            Err("Composer 动态库当前只支持 macOS 和 Windows".to_string())
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn beauty_last_error_text(&self) -> String {
        let Some(get_last_error) = self.beauty_get_last_error else {
            return String::new();
        };
        let error = unsafe { get_last_error() };
        if error.is_null() {
            return String::new();
        }

        unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .trim()
            .to_string()
    }

    fn cleanup(&mut self) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            if self.initialized {
                let Some(cleanup) = self.cleanup else {
                    app_log_error("[composer] composer_cleanup 函数未加载，跳过清理");
                    self.initialized = false;
                    return;
                };
                app_log_info("[composer] calling composer_cleanup");
                unsafe {
                    cleanup();
                }
                self.initialized = false;
                app_log_info("[composer] composer_cleanup complete");
            } else if let Some(error) = &self.init_error {
                app_log_error(format!(
                    "[composer] cleanup skipped because runtime is unavailable: {error}"
                ));
            }
        }
    }
}

impl Drop for ComposerRuntime {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn composer_step_status(step: i32) -> &'static str {
    match step {
        0 => "初始化",
        1 => "预处理片段",
        2 => "合成画中画",
        3 => "合并转场",
        4 => "构建最终视频",
        5 => "添加字幕",
        6 => "混流音频",
        7 => "合成完成",
        _ => "正在合成视频...",
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
extern "C" fn composer_progress_callback(
    percent: c_int,
    step: c_int,
    _message: *const c_char,
    userdata: *mut c_void,
) {
    if userdata.is_null() {
        app_log_error("[composer] progress callback skipped: userdata is null");
        return;
    }

    let context = unsafe { &*(userdata.cast::<ComposerCallbackContext>()) };
    let status = composer_step_status(step);
    let progress = percent.clamp(0, 100) as u8;

    emit_composer_progress(&context.app, &context.export_id, progress, status);
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn load_composer_library(library_path: &Path) -> Result<Library, libloading::Error> {
    #[cfg(target_os = "windows")]
    {
        let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS;
        return unsafe {
            libloading::os::windows::Library::load_with_flags(library_path, flags).map(Into::into)
        };
    }

    #[cfg(target_os = "macos")]
    unsafe {
        Library::new(library_path)
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn composer_library_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    let library_name = "libcomposer.dylib";
    #[cfg(target_os = "windows")]
    let library_name = "libcomposer.dll";

    app_log_info(format!("[composer] resolving {library_name} path"));

    #[cfg(target_os = "macos")]
    let bundled_path = std::env::current_exe().ok().and_then(|exe| {
        exe.parent()
            .and_then(|macos_dir| macos_dir.parent())
            .map(|contents_dir| contents_dir.join("Frameworks").join(library_name))
    });
    #[cfg(target_os = "windows")]
    let bundled_path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|app_dir| app_dir.join(library_name)));

    if let Some(path) = bundled_path {
        app_log_info(format!(
            "[composer] checking bundled dynamic library: {}",
            path.display()
        ));
        if path.is_file() {
            return Ok(path);
        }
    }

    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("libs")
        .join(std::env::consts::OS)
        .join(library_name);
    app_log_info(format!(
        "[composer] checking dev dynamic library: {}",
        dev_path.display()
    ));
    if dev_path.is_file() {
        return Ok(dev_path);
    }

    app_log_error(format!("[composer] {library_name} not found"));
    Err(format!("未找到 {library_name}"))
}

fn aicut_root_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let base_dir = dirs::data_local_dir().or_else(dirs::data_dir);

    #[cfg(target_os = "macos")]
    let base_dir = dirs::data_dir().or_else(dirs::data_local_dir);

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base_dir = dirs::data_local_dir().or_else(dirs::data_dir);

    base_dir
        .map(|path| path.join("aicut"))
        .ok_or_else(|| "Unable to resolve local app data directory".to_string())
}

fn ensure_aicut_dirs() -> Result<(PathBuf, PathBuf), String> {
    let root = aicut_root_dir()?;
    let template_dir = root.join("template");
    let project_dir = root.join("project");
    let logs_dir = root.join("logs");

    fs::create_dir_all(&template_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&project_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&logs_dir).map_err(|error| error.to_string())?;

    Ok((template_dir, project_dir))
}

fn ensure_aicut_logs_dir() -> Result<PathBuf, String> {
    let logs_dir = aicut_root_dir()?.join("logs");
    fs::create_dir_all(&logs_dir).map_err(|error| error.to_string())?;
    Ok(logs_dir)
}

fn aicut_log_file_path() -> Result<PathBuf, String> {
    Ok(ensure_aicut_logs_dir()?.join("app.log"))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn aicut_composer_error_log_file_path() -> Result<PathBuf, String> {
    Ok(ensure_aicut_logs_dir()?.join("composer-error.log"))
}

fn append_log_line(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("[log] failed to create log dir: {error}");
            return;
        }
    }

    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut file) => {
            if let Err(error) = file.write_all(line.as_bytes()) {
                eprintln!("[log] failed to write log: {error}");
            }
        }
        Err(error) => {
            eprintln!("[log] failed to open log {}: {error}", path.display());
        }
    }
}

fn append_app_log(level: &str, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("{timestamp} [{level}] {message}\n");

    match aicut_log_file_path() {
        Ok(path) => append_log_line(&path, &line),
        Err(error) => {
            eprintln!("[log] failed to resolve app log path: {error}");
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn append_composer_error_log(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("{timestamp} [COMPOSER_ERROR]\n{message}\n\n");

    match aicut_composer_error_log_file_path() {
        Ok(path) => append_log_line(&path, &line),
        Err(error) => {
            eprintln!("[log] failed to resolve composer error log path: {error}");
        }
    }
}

fn app_log_info(message: impl AsRef<str>) {
    let message = message.as_ref();
    println!("{message}");
    append_app_log("INFO", message);
}

fn app_log_error(message: impl AsRef<str>) {
    let message = message.as_ref();
    eprintln!("{message}");
    append_app_log("ERROR", message);
}

fn ensure_aicut_output_dir() -> Result<PathBuf, String> {
    let output_dir = aicut_root_dir()?.join("output");
    app_log_info(format!(
        "[composer] ensuring default output dir: {}",
        output_dir.display()
    ));
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    Ok(output_dir)
}

fn sanitize_name(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "template".to_string()
    } else {
        sanitized
    }
}

fn resolve_url(base_url: &str, url: &str) -> Result<String, String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        return Ok(url.to_string());
    }

    let base = base_url.trim_end_matches('/');
    if base.is_empty() {
        return Err("Download URL is relative but API base URL is empty".to_string());
    }

    Ok(format!("{base}/{}", url.trim_start_matches('/')))
}

fn decode_percent_encoded(value: &str) -> String {
    fn hex_value(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let source = value.as_bytes();
    let mut decoded = Vec::with_capacity(source.len());
    let mut index = 0;

    while index < source.len() {
        if source[index] == b'%' && index + 2 < source.len() {
            if let (Some(high), Some(low)) =
                (hex_value(source[index + 1]), hex_value(source[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }

        decoded.push(source[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).to_string()
}

fn sanitize_manual_filename(encoded_name: &str) -> String {
    let decoded = decode_percent_encoded(encoded_name);
    let file_name = decoded
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim();
    let sanitized: String = file_name
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches([' ', '.']);

    if sanitized.is_empty() {
        "AICut使用手册.docx".to_string()
    } else {
        sanitized.to_string()
    }
}

fn available_download_path(directory: &Path, file_name: &str) -> PathBuf {
    let requested_path = directory.join(file_name);
    if !requested_path.exists() {
        return requested_path;
    }

    let file_path = Path::new(file_name);
    let stem = file_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("AICut使用手册");
    let extension = file_path.extension().and_then(|value| value.to_str());

    for index in 1..10_000 {
        let candidate_name = match extension {
            Some(extension) if !extension.is_empty() => format!("{stem} ({index}).{extension}"),
            _ => format!("{stem} ({index})"),
        };
        let candidate_path = directory.join(candidate_name);
        if !candidate_path.exists() {
            return candidate_path;
        }
    }

    directory.join(format!(
        "AICut使用手册-{}.docx",
        Local::now().format("%Y%m%d%H%M%S")
    ))
}

fn download_help_guide_blocking(
    api_base_url: String,
    authorization_token: String,
    output_dir: String,
) -> Result<String, String> {
    if authorization_token.trim().is_empty() {
        return Err("未登录或 Token 已失效".to_string());
    }

    let output_dir = PathBuf::from(output_dir);
    if !output_dir.is_dir() {
        return Err("选择的保存目录无效".to_string());
    }

    let url = resolve_url(&api_base_url, "/aicut/manual/download")?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| format!("指南下载失败：{error}"))?;
    let response = client
        .get(&url)
        .bearer_auth(authorization_token.trim())
        .send()
        .map_err(|error| format!("指南下载失败：{error}"))?;
    let status = response.status();

    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => "未登录或 Token 已失效".to_string(),
            404 => "当前登录端对应的使用手册尚未配置".to_string(),
            500 => "使用手册文件读取或下载异常".to_string(),
            _ => format!("指南下载失败（HTTP {}）", status.as_u16()),
        });
    }

    let encoded_name = response
        .headers()
        .get("download-filename")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("AICut%E4%BD%BF%E7%94%A8%E6%89%8B%E5%86%8C.docx");
    let file_name = sanitize_manual_filename(encoded_name);
    let output_path = available_download_path(&output_dir, &file_name);
    let bytes = response
        .bytes()
        .map_err(|error| format!("指南下载失败：{error}"))?;

    fs::write(&output_path, bytes).map_err(|error| format!("指南保存失败：{error}"))?;
    Ok(output_path.to_string_lossy().to_string())
}

fn progress_between(start: u8, end: u8, completed: u64, total: Option<u64>) -> u8 {
    let Some(total) = total.filter(|value| *value > 0) else {
        return start;
    };
    let ratio = (completed as f64 / total as f64).clamp(0.0, 1.0);
    start + ((end - start) as f64 * ratio).round() as u8
}

fn cached_template_paths(template_id: &str) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let (template_root, _) = ensure_aicut_dirs()?;
    let template_dir = template_root.join(sanitize_name(template_id));
    let template_file_path = template_dir.join("template.xml");
    let assets_dir = template_dir.join("assets");

    Ok((template_dir, template_file_path, assets_dir))
}

fn is_url_resource_path(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://") || value.starts_with("file://")
}

fn is_absolute_resource_path(value: &str) -> bool {
    let normalized = value.replace('\\', "/");

    Path::new(value).is_absolute()
        || normalized.starts_with('/')
        || normalized.starts_with("//")
        || (normalized.len() > 2
            && normalized.as_bytes().get(1) == Some(&b':')
            && normalized.as_bytes().get(2) == Some(&b'/'))
}

fn path_to_xml_filepath(path: PathBuf) -> String {
    let filepath = path.to_string_lossy().to_string();

    if cfg!(windows) {
        let filepath = filepath.replace('/', "\\");

        if let Some(network_path) = filepath.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{network_path}")
        } else {
            filepath
                .strip_prefix(r"\\?\")
                .unwrap_or(&filepath)
                .to_string()
        }
    } else {
        filepath.replace('\\', "/")
    }
}

fn join_resource_relative(base: &Path, relative: &str) -> PathBuf {
    let mut path = base.to_path_buf();

    for segment in relative.replace('\\', "/").split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        path.push(segment);
    }

    path
}

fn resolve_template_resource_filepath(
    template_dir: &Path,
    assets_dir: &Path,
    filepath: &str,
) -> String {
    let trimmed = filepath.trim();
    if trimmed.is_empty() || is_url_resource_path(trimmed) || is_absolute_resource_path(trimmed) {
        return trimmed.to_string();
    }

    let normalized = trimmed
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string();

    if let Some(relative) = normalized.strip_prefix("template/assets/") {
        return path_to_xml_filepath(join_resource_relative(assets_dir, relative));
    }
    if let Some(relative) = normalized.strip_prefix("assets/") {
        return path_to_xml_filepath(join_resource_relative(assets_dir, relative));
    }
    if let Some(relative) = normalized.strip_prefix("template/") {
        return path_to_xml_filepath(join_resource_relative(template_dir, relative));
    }

    path_to_xml_filepath(join_resource_relative(assets_dir, &normalized))
}

fn xml_attribute_value(tag: &str, attribute: &str) -> Option<String> {
    let mut search_start = 0;

    loop {
        let relative_start = tag[search_start..].find(attribute)?;
        let attribute_start = search_start + relative_start;
        let attribute_end = attribute_start + attribute.len();
        let before_attribute = tag[..attribute_start].chars().next_back();
        let after_attribute_name = tag[attribute_end..].chars().next();
        let has_valid_start = before_attribute
            .map(|ch| ch.is_whitespace() || ch == '<' || ch == '/')
            .unwrap_or(true);
        let has_valid_end = after_attribute_name
            .map(|ch| ch.is_whitespace() || ch == '=')
            .unwrap_or(false);

        if has_valid_start && has_valid_end {
            let after_attribute = tag[attribute_end..].trim_start();
            let value_start = after_attribute.strip_prefix('=')?.trim_start();
            return parse_xml_attribute_value(value_start);
        }

        search_start = attribute_end;
    }
}

fn parse_xml_attribute_value(value_start: &str) -> Option<String> {
    let quote = value_start.chars().next()?;

    if quote == '"' || quote == '\'' {
        let value = &value_start[quote.len_utf8()..];
        let value_end = value.find(quote)?;
        Some(unescape_xml_value(&value[..value_end]))
    } else {
        let value_end = value_start
            .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
            .unwrap_or(value_start.len());
        Some(unescape_xml_value(&value_start[..value_end]))
    }
}

fn template_tag_version(xml_content: &str) -> Option<String> {
    let mut search_start = 0;

    while let Some(relative_start) = xml_content[search_start..].find("<template") {
        let tag_start = search_start + relative_start;
        let after_name = xml_content[tag_start + "<template".len()..]
            .chars()
            .next()?;

        if !after_name.is_whitespace() && after_name != '>' && after_name != '/' {
            search_start = tag_start + "<template".len();
            continue;
        }

        let tag_end = xml_content[tag_start..].find('>')? + tag_start;
        let tag = &xml_content[tag_start..=tag_end];
        return xml_attribute_value(tag, "version");
    }

    None
}

#[derive(Clone)]
struct TemplateAsset {
    id: String,
    filepath: String,
}

#[derive(Clone)]
struct TemplateMediaAsset {
    id: String,
    assets: Vec<TemplateAsset>,
}

#[derive(Clone)]
struct TemplateClipArea {
    id: String,
    asset_id: String,
}

#[derive(Clone)]
struct TemplateClip {
    id: String,
    areas: Vec<TemplateClipArea>,
}

#[derive(Clone)]
struct TemplateClips {
    id: String,
    target_track: String,
    clips: Vec<TemplateClip>,
}

struct TemplateSubtitle {
    clip_id: String,
    id: String,
    absolute_start_time: Option<String>,
    duration: Option<String>,
}

fn is_xml_name_boundary(ch: Option<char>) -> bool {
    ch.map(|value| value.is_whitespace() || value == '>' || value == '/')
        .unwrap_or(false)
}

fn find_xml_element_blocks(xml_content: &str, tag_name: &str) -> Vec<(String, String)> {
    let open_pattern = format!("<{tag_name}");
    let close_pattern = format!("</{tag_name}>");
    let mut blocks = Vec::new();
    let mut search_start = 0;

    while let Some(relative_start) = xml_content[search_start..].find(&open_pattern) {
        let tag_start = search_start + relative_start;
        let after_name = xml_content[tag_start + open_pattern.len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            search_start = tag_start + open_pattern.len();
            continue;
        }

        let Some(relative_tag_end) = xml_content[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end;
        let start_tag = xml_content[tag_start..=tag_end].to_string();

        if start_tag.trim_end().ends_with("/>") {
            blocks.push((start_tag, String::new()));
            search_start = tag_end + 1;
            continue;
        }

        let content_start = tag_end + 1;
        let Some(relative_close_start) = xml_content[content_start..].find(&close_pattern) else {
            break;
        };
        let close_start = content_start + relative_close_start;
        let inner = xml_content[content_start..close_start].to_string();
        blocks.push((start_tag, inner));
        search_start = close_start + close_pattern.len();
    }

    blocks
}

fn find_xml_start_tags(xml_content: &str, tag_name: &str) -> Vec<String> {
    let open_pattern = format!("<{tag_name}");
    let mut tags = Vec::new();
    let mut search_start = 0;

    while let Some(relative_start) = xml_content[search_start..].find(&open_pattern) {
        let tag_start = search_start + relative_start;
        let after_name = xml_content[tag_start + open_pattern.len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            search_start = tag_start + open_pattern.len();
            continue;
        }

        let Some(relative_tag_end) = xml_content[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end;
        tags.push(xml_content[tag_start..=tag_end].to_string());
        search_start = tag_end + 1;
    }

    tags
}

fn escape_xml_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn unescape_xml_value(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn normalize_template_asset_filepaths(
    xml_content: &str,
    template_dir: &Path,
    assets_dir: &Path,
) -> String {
    let mut output = String::new();
    let mut search_start = 0;

    while let Some(relative_start) = xml_content[search_start..].find("<asset") {
        let tag_start = search_start + relative_start;
        let after_name = xml_content[tag_start + "<asset".len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            output.push_str(&xml_content[search_start..tag_start + "<asset".len()]);
            search_start = tag_start + "<asset".len();
            continue;
        }

        let Some(relative_tag_end) = xml_content[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        let tag = &xml_content[tag_start..tag_end];

        output.push_str(&xml_content[search_start..tag_start]);

        if let Some(filepath) = xml_attribute_value(tag, "filepath") {
            let absolute_filepath =
                resolve_template_resource_filepath(template_dir, assets_dir, &filepath);
            output.push_str(&replace_or_insert_xml_attribute(
                tag,
                "filepath",
                &absolute_filepath,
            ));
        } else {
            output.push_str(tag);
        }

        search_start = tag_end;
    }

    output.push_str(&xml_content[search_start..]);
    output
}

fn normalize_template_resource_element(
    xml_content: &str,
    tag_name: &str,
    template_dir: &Path,
    assets_dir: &Path,
) -> String {
    let open_pattern = format!("<{tag_name}");
    let close_pattern = format!("</{tag_name}>");
    let mut output = String::new();
    let mut search_start = 0;

    while let Some(relative_start) = xml_content[search_start..].find(&open_pattern) {
        let tag_start = search_start + relative_start;
        let after_name = xml_content[tag_start + open_pattern.len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            output.push_str(&xml_content[search_start..tag_start + open_pattern.len()]);
            search_start = tag_start + open_pattern.len();
            continue;
        }

        let Some(relative_tag_end) = xml_content[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        let tag = &xml_content[tag_start..tag_end];

        if tag.trim_end().ends_with("/>") {
            output.push_str(&xml_content[search_start..tag_end]);
            search_start = tag_end;
            continue;
        }

        let Some(relative_close_start) = xml_content[tag_end..].find(&close_pattern) else {
            break;
        };
        let close_start = tag_end + relative_close_start;
        let close_end = close_start + close_pattern.len();
        let value = &xml_content[tag_end..close_start];

        output.push_str(&xml_content[search_start..tag_end]);

        if value.contains('<') {
            output.push_str(value);
        } else {
            let value = unescape_xml_value(value);
            let absolute_filepath =
                resolve_template_resource_filepath(template_dir, assets_dir, &value);
            output.push_str(&escape_xml_text(&absolute_filepath));
        }

        output.push_str(&xml_content[close_start..close_end]);
        search_start = close_end;
    }

    output.push_str(&xml_content[search_start..]);
    output
}

fn normalize_template_resource_paths(
    xml_content: &str,
    template_dir: &Path,
    assets_dir: &Path,
) -> String {
    let xml_content = normalize_template_asset_filepaths(xml_content, template_dir, assets_dir);
    let xml_content =
        normalize_template_resource_element(&xml_content, "demo-path", template_dir, assets_dir);

    normalize_template_resource_element(&xml_content, "filepath", template_dir, assets_dir)
}

fn normalize_template_file_resource_paths(
    template_file_path: &Path,
    template_dir: &Path,
    assets_dir: &Path,
    xml_content: String,
) -> Result<String, String> {
    let normalized_xml = normalize_template_resource_paths(&xml_content, template_dir, assets_dir);

    if normalized_xml != xml_content {
        fs::write(template_file_path, normalized_xml.as_bytes())
            .map_err(|error| error.to_string())?;
    }

    Ok(normalized_xml)
}

fn sanitize_file_name(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>()
        .trim_matches(|ch| ch == '.' || ch == ' ')
        .to_string();

    if sanitized.is_empty() {
        "video.mp4".to_string()
    } else {
        sanitized
    }
}

fn file_content_hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = DefaultHasher::new();
    let mut buffer = vec![0_u8; 64 * 1024];

    loop {
        let read_count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read_count == 0 {
            break;
        }

        hasher.write(&buffer[..read_count]);
    }

    Ok(format!("{:016x}", hasher.finish()))
}

fn project_filepath_candidates_from_asset_path(
    project_dir: &Path,
    asset_path: &Path,
) -> Vec<String> {
    let mut candidates = vec![path_to_xml_filepath(asset_path.to_path_buf())];

    if let Ok(relative_path) = asset_path.strip_prefix(project_dir) {
        let normalized_path = relative_path.to_string_lossy().replace('\\', "/");
        candidates.push(format!("project/{normalized_path}"));
    }

    candidates
}

fn collect_project_asset_filepaths(project_file_xml: &str) -> HashSet<String> {
    find_xml_start_tags(project_file_xml, "asset")
        .into_iter()
        .filter_map(|asset_tag| xml_attribute_value(&asset_tag, "filepath"))
        .collect()
}

fn replace_or_insert_xml_attribute(tag: &str, attribute: &str, value: &str) -> String {
    if let Some(attribute_position) = find_xml_attribute_position(tag, attribute) {
        let after_name = &tag[attribute_position + attribute.len()..];
        let leading_space_len = after_name.len() - after_name.trim_start().len();
        let after_space = &after_name[leading_space_len..];

        if let Some(after_equals) = after_space.strip_prefix('=') {
            let equals_and_space_len = 1 + after_equals.len() - after_equals.trim_start().len();
            let value_start =
                attribute_position + attribute.len() + leading_space_len + equals_and_space_len;

            if let Some(value_end) = xml_attribute_value_end(tag, value_start) {
                return format!(
                    "{}{}=\"{}\"{}",
                    &tag[..attribute_position],
                    attribute,
                    escape_xml_attribute(value),
                    &tag[value_end..]
                );
            }
        }
    }

    let insert_position = tag
        .rfind("/>")
        .or_else(|| tag.rfind('>'))
        .unwrap_or(tag.len());
    format!(
        "{} {}=\"{}\"{}",
        &tag[..insert_position].trim_end(),
        attribute,
        escape_xml_attribute(value),
        &tag[insert_position..]
    )
}

fn remove_xml_attribute(tag: &str, attribute: &str) -> String {
    let Some(attribute_position) = find_xml_attribute_position(tag, attribute) else {
        return tag.to_string();
    };
    let after_name = &tag[attribute_position + attribute.len()..];
    let leading_space_len = after_name.len() - after_name.trim_start().len();
    let after_space = &after_name[leading_space_len..];
    let Some(after_equals) = after_space.strip_prefix('=') else {
        return tag.to_string();
    };
    let equals_and_space_len = 1 + after_equals.len() - after_equals.trim_start().len();
    let value_start =
        attribute_position + attribute.len() + leading_space_len + equals_and_space_len;
    let Some(value_end) = xml_attribute_value_end(tag, value_start) else {
        return tag.to_string();
    };

    let mut removal_start = attribute_position;
    while removal_start > 0 {
        match tag.as_bytes()[removal_start - 1] {
            b' ' | b'\t' => removal_start -= 1,
            _ => break,
        }
    }

    format!("{}{}", &tag[..removal_start], &tag[value_end..])
}

fn find_xml_attribute_position(tag: &str, attribute: &str) -> Option<usize> {
    let mut search_start = 0;

    loop {
        let relative_start = tag[search_start..].find(attribute)?;
        let attribute_start = search_start + relative_start;
        let attribute_end = attribute_start + attribute.len();
        let before_attribute = tag[..attribute_start].chars().next_back();
        let after_attribute_name = tag[attribute_end..].chars().next();
        let has_valid_start = before_attribute
            .map(|ch| ch.is_whitespace() || ch == '<' || ch == '/')
            .unwrap_or(true);
        let has_valid_end = after_attribute_name
            .map(|ch| ch.is_whitespace() || ch == '=')
            .unwrap_or(false);

        if has_valid_start && has_valid_end {
            return Some(attribute_start);
        }

        search_start = attribute_end;
    }
}

fn xml_attribute_value_end(tag: &str, value_start: usize) -> Option<usize> {
    let value = &tag[value_start..];
    let quote = value.chars().next()?;

    if quote == '"' || quote == '\'' {
        let inner_start = quote.len_utf8();
        let relative_end = value[inner_start..].find(quote)? + inner_start + quote.len_utf8();
        Some(value_start + relative_end)
    } else {
        let relative_end = value
            .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
            .unwrap_or(value.len());
        Some(value_start + relative_end)
    }
}

fn update_project_asset_filepath(
    project_file_xml: &str,
    asset_id: &str,
    project_filepath: &str,
) -> Result<String, String> {
    let mut output = String::new();
    let mut search_start = 0;
    let mut updated = false;

    while let Some(relative_start) = project_file_xml[search_start..].find("<asset") {
        let tag_start = search_start + relative_start;
        let after_name = project_file_xml[tag_start + "<asset".len()..]
            .chars()
            .next();

        if !is_xml_name_boundary(after_name) {
            output.push_str(&project_file_xml[search_start..tag_start + "<asset".len()]);
            search_start = tag_start + "<asset".len();
            continue;
        }

        let Some(relative_tag_end) = project_file_xml[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        let tag = &project_file_xml[tag_start..tag_end];

        output.push_str(&project_file_xml[search_start..tag_start]);

        if xml_attribute_value(tag, "id")
            .map(|value| value == asset_id)
            .unwrap_or(false)
        {
            output.push_str(&replace_or_insert_xml_attribute(
                tag,
                "filepath",
                project_filepath,
            ));
            updated = true;
        } else {
            output.push_str(tag);
        }

        search_start = tag_end;
    }

    output.push_str(&project_file_xml[search_start..]);

    if updated {
        Ok(output)
    } else {
        Err("projectFile.xml 中未找到对应的 asset".to_string())
    }
}

fn update_project_asset_generatepath(
    template_xml: &str,
    asset_id: &str,
    generate_path: Option<&str>,
) -> Result<String, String> {
    let mut output = String::new();
    let mut search_start = 0;
    let mut updated = false;

    while let Some(relative_start) = template_xml[search_start..].find("<asset") {
        let tag_start = search_start + relative_start;
        let after_name = template_xml[tag_start + "<asset".len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            output.push_str(&template_xml[search_start..tag_start + "<asset".len()]);
            search_start = tag_start + "<asset".len();
            continue;
        }

        let Some(relative_tag_end) = template_xml[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        let tag = &template_xml[tag_start..tag_end];

        output.push_str(&template_xml[search_start..tag_start]);
        if xml_attribute_value(tag, "id").as_deref() == Some(asset_id) {
            output.push_str(&match generate_path {
                Some(path) => replace_or_insert_xml_attribute(tag, "generatepath", path),
                None => remove_xml_attribute(tag, "generatepath"),
            });
            updated = true;
        } else {
            output.push_str(tag);
        }
        search_start = tag_end;
    }

    output.push_str(&template_xml[search_start..]);
    if updated {
        Ok(output)
    } else {
        Err("template.xml 中未找到对应的 asset".to_string())
    }
}

fn format_property_number(value: f64) -> String {
    let mut formatted = format!("{value:.6}");
    while formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.push('0');
    }
    formatted
}

fn normalize_project_asset_properties(
    mut properties: ProjectAssetProperties,
) -> Result<ProjectAssetProperties, String> {
    properties.whiteness = finite_or(properties.whiteness, 0.0).clamp(0.0, 1.0);
    properties.smoothing = finite_or(properties.smoothing, 0.0).clamp(0.0, 1.0);
    properties.saturation = finite_or(properties.saturation, 100.0).clamp(0.0, 200.0);
    properties.skin_tone = finite_or(properties.skin_tone, 0.0).clamp(-1.0, 1.0);
    properties.face_detect = i32::from(properties.face_detect != 0);
    properties.rotation = finite_or(properties.rotation, 0.0);
    properties.lut_style = match properties.lut_style.trim() {
        "" => "none".to_string(),
        value => value.to_string(),
    };
    properties.lut_intensity = finite_or(properties.lut_intensity, 0.0).clamp(0.0, 1.0);
    properties.position_x = finite_or(properties.position_x, 0.0);
    properties.position_y = finite_or(properties.position_y, 0.0);
    properties.scale = finite_or(properties.scale, 1.0).clamp(0.01, 10.0);
    properties.canvas_width = properties.canvas_width.clamp(1, 16_384);
    properties.canvas_height = properties.canvas_height.clamp(1, 16_384);
    properties.transform_origin = match properties
        .transform_origin
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "center" => "center".to_string(),
        _ => return Err("当前仅支持以 center 作为视频变换原点".to_string()),
    };
    properties.generatepath = properties
        .generatepath
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Ok(properties)
}

fn project_asset_property_xml(properties: &ProjectAssetProperties, indent: &str) -> String {
    let value_indent = format!("{indent}    ");
    let mut values = vec![
        ("whiteness", format_property_number(properties.whiteness)),
        ("smoothing", format_property_number(properties.smoothing)),
        ("saturation", format_property_number(properties.saturation)),
        ("skin_tone", format_property_number(properties.skin_tone)),
        ("face_detect", properties.face_detect.to_string()),
        ("rotation", format_property_number(properties.rotation)),
        ("lut_style", properties.lut_style.clone()),
        (
            "lut_intensity",
            format_property_number(properties.lut_intensity),
        ),
        ("positionX", format_property_number(properties.position_x)),
        ("positionY", format_property_number(properties.position_y)),
        ("scale", format_property_number(properties.scale)),
        ("canvas_width", properties.canvas_width.to_string()),
        ("canvas_height", properties.canvas_height.to_string()),
        ("transform_origin", properties.transform_origin.clone()),
        ("stabilization", properties.stabilization.to_string()),
        ("one_click_beauty", properties.one_click_beauty.to_string()),
    ];
    if let Some(generatepath) = &properties.generatepath {
        values.push(("generatepath", generatepath.clone()));
    }
    let mut output = String::from("<property>");

    for (name, value) in values {
        output.push_str(&format!(
            "\n{value_indent}<{name}>{}</{name}>",
            escape_xml_text(&value)
        ));
    }
    output.push_str(&format!("\n{indent}</property>"));
    output
}

fn replace_or_append_area_property(
    area_inner: &str,
    property_xml: &str,
    property_indent: &str,
) -> String {
    let mut area_inner = area_inner.to_string();
    while let Some(property_start) = area_inner.find("<property") {
        let after_name = area_inner[property_start + "<property".len()..]
            .chars()
            .next();
        if !is_xml_name_boundary(after_name) {
            break;
        }
        let Some(relative_tag_end) = area_inner[property_start..].find('>') else {
            break;
        };
        let content_start = property_start + relative_tag_end + 1;
        let Some(relative_close_start) = area_inner[content_start..].find("</property>") else {
            break;
        };
        let property_end = content_start + relative_close_start + "</property>".len();
        let line_start = area_inner[..property_start]
            .rfind('\n')
            .map(|position| position + 1)
            .unwrap_or(0);
        let property_line_is_indented = area_inner[line_start..property_start]
            .chars()
            .all(|character| matches!(character, ' ' | '\t' | '\r'));
        let removal_start = if property_line_is_indented {
            line_start
        } else {
            property_start
        };
        let mut removal_end = property_end;
        while matches!(area_inner.as_bytes().get(removal_end), Some(b' ' | b'\t')) {
            removal_end += 1;
        }
        if area_inner.as_bytes().get(removal_end) == Some(&b'\r') {
            removal_end += 1;
        }
        if area_inner.as_bytes().get(removal_end) == Some(&b'\n') {
            removal_end += 1;
        }
        area_inner.replace_range(removal_start..removal_end, "");
    }

    let content_end = area_inner.trim_end().len();
    let mut output = area_inner[..content_end].to_string();
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(property_indent);
    output.push_str(property_xml);
    output.push('\n');
    output.push_str(property_indent.strip_suffix("    ").unwrap_or(""));
    output
}

fn update_template_asset_properties(
    template_xml: &str,
    asset_id: &str,
    properties: &ProjectAssetProperties,
) -> Result<String, String> {
    let mut output = String::new();
    let mut search_start = 0;
    let mut updated_count = 0;

    while let Some(relative_start) = template_xml[search_start..].find("<area") {
        let area_start = search_start + relative_start;
        let after_name = template_xml[area_start + "<area".len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            output.push_str(&template_xml[search_start..area_start + "<area".len()]);
            search_start = area_start + "<area".len();
            continue;
        }

        let Some(relative_tag_end) = template_xml[area_start..].find('>') else {
            break;
        };
        let tag_end = area_start + relative_tag_end + 1;
        let area_tag = &template_xml[area_start..tag_end];
        if xml_attribute_value(area_tag, "asset-id").as_deref() != Some(asset_id) {
            output.push_str(&template_xml[search_start..tag_end]);
            search_start = tag_end;
            continue;
        }

        let line_start = template_xml[..area_start]
            .rfind('\n')
            .map(|position| position + 1)
            .unwrap_or(0);
        let line_prefix = &template_xml[line_start..area_start];
        let area_indent = if line_prefix.chars().all(char::is_whitespace) {
            line_prefix
        } else {
            ""
        };
        let property_indent = format!("{area_indent}    ");
        let property_xml = project_asset_property_xml(properties, &property_indent);
        output.push_str(&template_xml[search_start..area_start]);

        if area_tag.trim_end().ends_with("/>") {
            let opening_tag = area_tag
                .trim_end()
                .strip_suffix("/>")
                .unwrap_or(area_tag)
                .trim_end();
            output.push_str(opening_tag);
            output.push_str(">\n");
            output.push_str(&property_indent);
            output.push_str(&property_xml);
            output.push('\n');
            output.push_str(area_indent);
            output.push_str("</area>");
            search_start = tag_end;
            updated_count += 1;
            continue;
        }

        let Some(relative_close_start) = template_xml[tag_end..].find("</area>") else {
            return Err("template.xml 中的 area 节点未正确闭合".to_string());
        };
        let close_start = tag_end + relative_close_start;
        let close_end = close_start + "</area>".len();
        let area_inner = &template_xml[tag_end..close_start];
        let updated_inner =
            replace_or_append_area_property(area_inner, &property_xml, &property_indent);
        output.push_str(area_tag);
        output.push_str(&updated_inner);
        output.push_str("</area>");
        search_start = close_end;
        updated_count += 1;
    }

    output.push_str(&template_xml[search_start..]);
    if updated_count == 0 {
        Err("template.xml 中未找到使用当前 assetId 的 area".to_string())
    } else {
        Ok(output)
    }
}

fn remove_xml_child_element(content: &str, element_name: &str) -> String {
    let opening_prefix = format!("<{element_name}");
    let closing_tag = format!("</{element_name}>");
    let mut output = content.to_string();

    while let Some(element_start) = output.find(&opening_prefix) {
        let after_name = output[element_start + opening_prefix.len()..]
            .chars()
            .next();
        if !is_xml_name_boundary(after_name) {
            break;
        }
        let Some(relative_open_end) = output[element_start..].find('>') else {
            break;
        };
        let content_start = element_start + relative_open_end + 1;
        let Some(relative_close_start) = output[content_start..].find(&closing_tag) else {
            break;
        };
        let element_end = content_start + relative_close_start + closing_tag.len();
        let line_start = output[..element_start]
            .rfind('\n')
            .map(|position| position + 1)
            .unwrap_or(0);
        let line_is_indented = output[line_start..element_start]
            .chars()
            .all(|character| matches!(character, ' ' | '\t' | '\r'));
        let removal_start = if line_is_indented {
            line_start
        } else {
            element_start
        };
        let mut removal_end = element_end;
        while matches!(output.as_bytes().get(removal_end), Some(b' ' | b'\t')) {
            removal_end += 1;
        }
        if output.as_bytes().get(removal_end) == Some(&b'\r') {
            removal_end += 1;
        }
        if output.as_bytes().get(removal_end) == Some(&b'\n') {
            removal_end += 1;
        }
        output.replace_range(removal_start..removal_end, "");
    }

    output
}

fn remove_asset_area_property_element(
    xml: &str,
    asset_id: &str,
    element_name: &str,
) -> Result<String, String> {
    let mut output = String::new();
    let mut search_start = 0;
    let mut matched_count = 0;

    while let Some(relative_start) = xml[search_start..].find("<area") {
        let area_start = search_start + relative_start;
        let after_name = xml[area_start + "<area".len()..].chars().next();
        if !is_xml_name_boundary(after_name) {
            output.push_str(&xml[search_start..area_start + "<area".len()]);
            search_start = area_start + "<area".len();
            continue;
        }

        let Some(relative_tag_end) = xml[area_start..].find('>') else {
            break;
        };
        let tag_end = area_start + relative_tag_end + 1;
        let area_tag = &xml[area_start..tag_end];
        output.push_str(&xml[search_start..tag_end]);
        search_start = tag_end;
        if xml_attribute_value(area_tag, "asset-id").as_deref() != Some(asset_id) {
            continue;
        }
        matched_count += 1;
        if area_tag.trim_end().ends_with("/>") {
            continue;
        }

        let Some(relative_close_start) = xml[tag_end..].find("</area>") else {
            return Err("XML 中的 area 节点未正确闭合".to_string());
        };
        let close_start = tag_end + relative_close_start;
        let close_end = close_start + "</area>".len();
        output.push_str(&remove_xml_child_element(
            &xml[tag_end..close_start],
            element_name,
        ));
        output.push_str("</area>");
        search_start = close_end;
    }

    output.push_str(&xml[search_start..]);
    if matched_count == 0 {
        Err("XML 中未找到使用当前 assetId 的 area".to_string())
    } else {
        Ok(output)
    }
}

fn collect_xml_child_element_values(content: &str, element_name: &str) -> Vec<String> {
    let opening_prefix = format!("<{element_name}");
    let closing_tag = format!("</{element_name}>");
    let mut values = Vec::new();
    let mut search_start = 0;

    while let Some(relative_start) = content[search_start..].find(&opening_prefix) {
        let element_start = search_start + relative_start;
        let after_name = content[element_start + opening_prefix.len()..]
            .chars()
            .next();
        if !is_xml_name_boundary(after_name) {
            search_start = element_start + opening_prefix.len();
            continue;
        }
        let Some(relative_open_end) = content[element_start..].find('>') else {
            break;
        };
        let value_start = element_start + relative_open_end + 1;
        let Some(relative_close_start) = content[value_start..].find(&closing_tag) else {
            break;
        };
        let value_end = value_start + relative_close_start;
        let value = unescape_xml_value(content[value_start..value_end].trim());
        if !value.is_empty() {
            values.push(value);
        }
        search_start = value_end + closing_tag.len();
    }

    values
}

fn collect_asset_generated_paths(xml: &str, asset_id: &str) -> HashSet<String> {
    let mut paths = HashSet::new();
    let mut search_start = 0;

    while let Some(relative_start) = xml[search_start..].find("<area") {
        let area_start = search_start + relative_start;
        let after_name = xml[area_start + "<area".len()..].chars().next();
        if !is_xml_name_boundary(after_name) {
            search_start = area_start + "<area".len();
            continue;
        }
        let Some(relative_tag_end) = xml[area_start..].find('>') else {
            break;
        };
        let tag_end = area_start + relative_tag_end + 1;
        let area_tag = &xml[area_start..tag_end];
        search_start = tag_end;
        if xml_attribute_value(area_tag, "asset-id").as_deref() != Some(asset_id)
            || area_tag.trim_end().ends_with("/>")
        {
            continue;
        }
        let Some(relative_close_start) = xml[tag_end..].find("</area>") else {
            break;
        };
        let close_start = tag_end + relative_close_start;
        paths.extend(collect_xml_child_element_values(
            &xml[tag_end..close_start],
            "generatepath",
        ));
        search_start = close_start + "</area>".len();
    }

    for asset_tag in find_xml_start_tags(xml, "asset") {
        if xml_attribute_value(&asset_tag, "id").as_deref() == Some(asset_id) {
            if let Some(path) = xml_attribute_value(&asset_tag, "generatepath") {
                if !path.trim().is_empty() {
                    paths.insert(path);
                }
            }
        }
    }

    paths
}

fn update_project_clip_offsets(
    project_file_xml: &str,
    asset_id: &str,
    offset_ms: u64,
) -> Result<String, String> {
    let mut output = String::new();
    let mut search_start = 0;
    let mut updated = false;
    let offset = offset_ms.to_string();

    while let Some(relative_start) = project_file_xml[search_start..].find("<area") {
        let tag_start = search_start + relative_start;
        let after_name = project_file_xml[tag_start + "<area".len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            output.push_str(&project_file_xml[search_start..tag_start + "<area".len()]);
            search_start = tag_start + "<area".len();
            continue;
        }

        let Some(relative_tag_end) = project_file_xml[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        let tag = &project_file_xml[tag_start..tag_end];

        output.push_str(&project_file_xml[search_start..tag_start]);

        if xml_attribute_value(tag, "asset-id")
            .map(|value| value == asset_id)
            .unwrap_or(false)
        {
            output.push_str(&replace_or_insert_xml_attribute(tag, "offset", &offset));
            updated = true;
        } else {
            output.push_str(tag);
        }

        search_start = tag_end;
    }

    output.push_str(&project_file_xml[search_start..]);

    if updated {
        Ok(output)
    } else {
        Err("projectFile.xml 中未找到对应的 area".to_string())
    }
}

fn update_project_clip_area_offsets(
    project_file_xml: &str,
    asset_id: &str,
    area_offsets: &[ProjectAreaOffsetUpdate],
) -> Result<String, String> {
    let offset_by_area = area_offsets
        .iter()
        .filter(|area_offset| !area_offset.area_id.trim().is_empty())
        .map(|area_offset| {
            (
                area_offset.area_id.trim().to_string(),
                area_offset.offset_ms.to_string(),
            )
        })
        .collect::<HashMap<_, _>>();

    if offset_by_area.is_empty() {
        return Err("areaOffsets 涓嶈兘涓虹┖".to_string());
    }

    let mut output = String::new();
    let mut search_start = 0;
    let mut updated = false;

    while let Some(relative_start) = project_file_xml[search_start..].find("<area") {
        let tag_start = search_start + relative_start;
        let after_name = project_file_xml[tag_start + "<area".len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            output.push_str(&project_file_xml[search_start..tag_start + "<area".len()]);
            search_start = tag_start + "<area".len();
            continue;
        }

        let Some(relative_tag_end) = project_file_xml[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        let tag = &project_file_xml[tag_start..tag_end];

        output.push_str(&project_file_xml[search_start..tag_start]);

        let area_id = xml_attribute_value(tag, "id").unwrap_or_default();
        let area_asset_id = xml_attribute_value(tag, "asset-id").unwrap_or_default();

        if area_asset_id == asset_id {
            if let Some(offset) = offset_by_area.get(&area_id) {
                output.push_str(&replace_or_insert_xml_attribute(tag, "offset", offset));
                updated = true;
            } else {
                output.push_str(tag);
            }
        } else {
            output.push_str(tag);
        }

        search_start = tag_end;
    }

    output.push_str(&project_file_xml[search_start..]);

    if updated {
        Ok(output)
    } else {
        Err("projectFile.xml 涓湭鎵惧埌瀵瑰簲鐨?area".to_string())
    }
}

fn remove_subtitle_tags(xml_content: &str) -> String {
    let mut output = String::new();
    let mut search_start = 0;

    while let Some(relative_start) = xml_content[search_start..].find("<subtitle") {
        let tag_start = search_start + relative_start;
        let after_name = xml_content[tag_start + "<subtitle".len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            output.push_str(&xml_content[search_start..tag_start + "<subtitle".len()]);
            search_start = tag_start + "<subtitle".len();
            continue;
        }

        let Some(relative_tag_end) = xml_content[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        let tag = &xml_content[tag_start..tag_end];
        output.push_str(&xml_content[search_start..tag_start]);

        if tag.trim_end().ends_with("/>") {
            search_start = tag_end;
            continue;
        }

        if let Some(relative_close_start) = xml_content[tag_end..].find("</subtitle>") {
            search_start = tag_end + relative_close_start + "</subtitle>".len();
        } else {
            search_start = tag_end;
        }
    }

    output.push_str(&xml_content[search_start..]);
    output
}

fn find_first_template_subtitle(xml_content: &str) -> Option<TemplateSubtitle> {
    find_xml_element_blocks(xml_content, "clips")
        .into_iter()
        .find_map(|(_, clips_inner)| {
            find_xml_element_blocks(&clips_inner, "clip")
                .into_iter()
                .find_map(|(clip_tag, clip_inner)| {
                    let clip_id = xml_attribute_value(&clip_tag, "id")?;
                    let subtitle_tag = find_xml_start_tags(&clip_inner, "subtitle")
                        .into_iter()
                        .next()?;
                    let id = xml_attribute_value(&subtitle_tag, "id")?;
                    let absolute_start_time =
                        xml_attribute_value(&subtitle_tag, "absoluteStartTime");
                    let duration = xml_attribute_value(&subtitle_tag, "duration");

                    Some(TemplateSubtitle {
                        clip_id,
                        id,
                        absolute_start_time,
                        duration,
                    })
                })
        })
}

fn update_project_subtitle(
    project_file_xml: &str,
    subtitle: &TemplateSubtitle,
    text: &str,
) -> Result<String, String> {
    let mut output = String::new();
    let mut search_start = 0;
    let mut updated = false;

    while let Some(relative_start) = project_file_xml[search_start..].find("<clip") {
        let tag_start = search_start + relative_start;
        let after_name = project_file_xml[tag_start + "<clip".len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            output.push_str(&project_file_xml[search_start..tag_start + "<clip".len()]);
            search_start = tag_start + "<clip".len();
            continue;
        }

        let Some(relative_tag_end) = project_file_xml[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        let tag = &project_file_xml[tag_start..tag_end];

        if tag.trim_end().ends_with("/>") {
            output.push_str(&project_file_xml[search_start..tag_end]);
            search_start = tag_end;
            continue;
        }

        let Some(relative_close_start) = project_file_xml[tag_end..].find("</clip>") else {
            break;
        };
        let close_start = tag_end + relative_close_start;
        let close_end = close_start + "</clip>".len();
        let inner = &project_file_xml[tag_end..close_start];
        let cleaned_inner = remove_subtitle_tags(inner);

        output.push_str(&project_file_xml[search_start..tag_start]);
        output.push_str(tag);
        output.push_str(&cleaned_inner);

        if xml_attribute_value(tag, "id")
            .map(|value| value == subtitle.clip_id)
            .unwrap_or(false)
        {
            let absolute_start_time = subtitle
                .absolute_start_time
                .as_ref()
                .map(|value| format!(" absoluteStartTime=\"{}\"", escape_xml_attribute(value)))
                .unwrap_or_default();
            let duration = subtitle
                .duration
                .as_ref()
                .map(|value| format!(" duration=\"{}\"", escape_xml_attribute(value)))
                .unwrap_or_default();
            output.push_str(&format!(
                "                <subtitle id=\"{}\" text=\"{}\"{}{} />\n",
                escape_xml_attribute(&subtitle.id),
                escape_xml_attribute(text),
                absolute_start_time,
                duration
            ));
            updated = true;
        }

        output.push_str("</clip>");
        search_start = close_end;
    }

    output.push_str(&project_file_xml[search_start..]);

    if updated {
        Ok(output)
    } else {
        Err("projectFile.xml 中未找到对应的 clip".to_string())
    }
}

fn update_template_subtitle_default(
    template_xml: &str,
    subtitle: &TemplateSubtitle,
    text: &str,
) -> Result<String, String> {
    let mut output = String::new();
    let mut search_start = 0;
    let mut updated = false;

    while let Some(relative_start) = template_xml[search_start..].find("<subtitle") {
        let tag_start = search_start + relative_start;
        let after_name = template_xml[tag_start + "<subtitle".len()..].chars().next();

        if !is_xml_name_boundary(after_name) {
            output.push_str(&template_xml[search_start..tag_start + "<subtitle".len()]);
            search_start = tag_start + "<subtitle".len();
            continue;
        }

        let Some(relative_tag_end) = template_xml[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        let tag = &template_xml[tag_start..tag_end];

        if xml_attribute_value(tag, "id")
            .map(|value| value != subtitle.id)
            .unwrap_or(true)
        {
            output.push_str(&template_xml[search_start..tag_end]);
            search_start = tag_end;
            continue;
        }

        if tag.trim_end().ends_with("/>") {
            output.push_str(&template_xml[search_start..tag_start]);
            output.push_str(&replace_or_insert_xml_attribute(tag, "text", text));
            search_start = tag_end;
            updated = true;
            continue;
        }

        let Some(relative_close_start) = template_xml[tag_end..].find("</subtitle>") else {
            break;
        };
        let close_start = tag_end + relative_close_start;
        let close_end = close_start + "</subtitle>".len();
        let inner = &template_xml[tag_end..close_start];
        let updated_inner = if let Some(default_start) = inner.find("<default>") {
            let default_content_start = default_start + "<default>".len();
            if let Some(relative_default_end) = inner[default_content_start..].find("</default>") {
                let default_end = default_content_start + relative_default_end;
                format!(
                    "{}{}{}",
                    &inner[..default_content_start],
                    escape_xml_text(text),
                    &inner[default_end..]
                )
            } else {
                format!("{inner}<default>{}</default>", escape_xml_text(text))
            }
        } else {
            format!("{inner}<default>{}</default>", escape_xml_text(text))
        };

        output.push_str(&template_xml[search_start..tag_end]);
        output.push_str(&updated_inner);
        output.push_str("</subtitle>");
        search_start = close_end;
        updated = true;
    }

    output.push_str(&template_xml[search_start..]);

    if updated {
        Ok(output)
    } else {
        Err("template.xml 中未找到对应的 subtitle".to_string())
    }
}

fn parse_template_media_assets(xml_content: &str) -> Vec<TemplateMediaAsset> {
    find_xml_element_blocks(xml_content, "media-asset")
        .into_iter()
        .filter_map(|(media_tag, media_inner)| {
            let id = xml_attribute_value(&media_tag, "id")?;
            let default_assets = find_xml_element_blocks(&media_inner, "default-asset")
                .into_iter()
                .next()
                .map(|(_, inner)| inner)
                .unwrap_or_default();
            let assets = find_xml_start_tags(&default_assets, "asset")
                .into_iter()
                .filter_map(|asset_tag| {
                    Some(TemplateAsset {
                        id: xml_attribute_value(&asset_tag, "id")?,
                        filepath: xml_attribute_value(&asset_tag, "filepath").unwrap_or_default(),
                    })
                })
                .collect::<Vec<_>>();

            Some(TemplateMediaAsset { id, assets })
        })
        .collect()
}

fn parse_template_clips(xml_content: &str) -> Vec<TemplateClips> {
    find_xml_element_blocks(xml_content, "clips")
        .into_iter()
        .filter_map(|(clips_tag, clips_inner)| {
            let id = xml_attribute_value(&clips_tag, "id").unwrap_or_else(|| "clips".to_string());
            let target_track = xml_attribute_value(&clips_tag, "target-track")
                .unwrap_or_else(|| "clips".to_string());
            let clips = find_xml_element_blocks(&clips_inner, "clip")
                .into_iter()
                .filter_map(|(clip_tag, clip_inner)| {
                    let id = xml_attribute_value(&clip_tag, "id")?;
                    let areas = find_xml_element_blocks(&clip_inner, "area")
                        .into_iter()
                        .filter_map(|(area_tag, _)| {
                            Some(TemplateClipArea {
                                id: xml_attribute_value(&area_tag, "id")?,
                                asset_id: xml_attribute_value(&area_tag, "asset-id")?,
                            })
                        })
                        .collect::<Vec<_>>();

                    Some(TemplateClip { id, areas })
                })
                .collect::<Vec<_>>();

            Some(TemplateClips {
                id,
                target_track,
                clips,
            })
        })
        .collect()
}

fn format_timestamp(timestamp_ms: u128) -> String {
    i64::try_from(timestamp_ms)
        .ok()
        .and_then(DateTime::from_timestamp_millis)
        .map(|datetime| {
            datetime
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
}

fn generate_project_file_xml(
    template_xml: &str,
    project_id: &str,
    last_update_time: u128,
) -> Result<String, String> {
    let (template_tag, _) = find_xml_element_blocks(template_xml, "template")
        .into_iter()
        .next()
        .ok_or_else(|| "模板 XML 缺少 template 节点".to_string())?;
    let template_id = xml_attribute_value(&template_tag, "id")
        .ok_or_else(|| "模板 XML 缺少 template id".to_string())?;
    let template_name =
        xml_attribute_value(&template_tag, "name").unwrap_or_else(|| template_id.clone());
    let template_version = xml_attribute_value(&template_tag, "version").unwrap_or_default();
    let timeunit =
        xml_attribute_value(&template_tag, "timeunit").unwrap_or_else(|| "millisecond".to_string());
    let media_assets = parse_template_media_assets(template_xml);
    let template_clips = parse_template_clips(template_xml);
    let last_update_time = format_timestamp(last_update_time);
    let mut output = String::new();

    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<!DOCTYPE xmeml>\n");
    output.push_str("<xmeml version=\"5\">\n\n");
    output.push_str(&format!(
        "    <project id=\"{}\" name=\"{}\" version=\"{}\" timeunit=\"{}\">\n",
        escape_xml_attribute(project_id),
        escape_xml_attribute(&template_name),
        escape_xml_attribute(&template_version),
        escape_xml_attribute(&timeunit)
    ));
    output.push_str("        <meta>\n");
    output.push_str(&format!(
        "            <template id=\"{}\" version=\"{}\"></template>\n",
        escape_xml_attribute(&template_id),
        escape_xml_attribute(&template_version)
    ));
    output.push_str("            <status>0</status>\n");
    output.push_str(&format!(
        "            <last-updtime>{last_update_time}</last-updtime>\n"
    ));
    output.push_str("        </meta>\n\n");
    for media_asset in &media_assets {
        output.push_str(&format!(
            "        <media-asset id=\"{}\">\n",
            escape_xml_attribute(&media_asset.id)
        ));

        for asset in &media_asset.assets {
            output.push_str(&format!(
                "            <asset id=\"{}\" filepath=\"{}\" />\n",
                escape_xml_attribute(&asset.id),
                escape_xml_attribute(&asset.filepath)
            ));
        }

        output.push_str("        </media-asset>\n\n");
    }

    for clips in &template_clips {
        output.push_str(&format!(
            "        <clips id=\"{}\" target-track=\"{}\">\n",
            escape_xml_attribute(&clips.id),
            escape_xml_attribute(&clips.target_track)
        ));

        for clip in &clips.clips {
            output.push_str(&format!(
                "            <clip id=\"{}\">\n",
                escape_xml_attribute(&clip.id)
            ));

            for area in &clip.areas {
                output.push_str(&format!(
                    "                <area id=\"{}\" asset-id=\"{}\" offset=\"0\" />\n",
                    escape_xml_attribute(&area.id),
                    escape_xml_attribute(&area.asset_id)
                ));
            }

            output.push_str("            </clip>\n\n");
        }

        output.push_str("        </clips>\n\n");
    }

    output.push_str("    </project>\n\n");
    output.push_str("</xmeml>\n");

    Ok(output)
}

fn xml_matches_template_version(xml_content: &str, template_version: &str) -> bool {
    let expected_version = template_version.trim();
    if expected_version.is_empty() {
        return false;
    }

    template_tag_version(xml_content)
        .map(|local_version| local_version.trim() == expected_version)
        .unwrap_or(false)
}

fn read_cached_template_assets(
    template_id: &str,
    template_version: &str,
) -> Result<Option<PreparedTemplate>, String> {
    let (template_dir, template_file_path, assets_dir) = cached_template_paths(template_id)?;

    if !template_file_path.is_file() || !assets_dir.is_dir() {
        return Ok(None);
    }

    let xml_content = fs::read_to_string(&template_file_path).map_err(|error| error.to_string())?;
    if !xml_matches_template_version(&xml_content, template_version) {
        return Ok(None);
    }
    let xml_content = normalize_template_file_resource_paths(
        &template_file_path,
        &template_dir,
        &assets_dir,
        xml_content,
    )?;

    Ok(Some(PreparedTemplate {
        template_dir: template_dir.to_string_lossy().to_string(),
        template_file_path: template_file_path.to_string_lossy().to_string(),
        material_package_path: String::new(),
        assets_dir: assets_dir.to_string_lossy().to_string(),
        xml_content,
    }))
}

fn download_bytes(
    app: &AppHandle,
    download_id: &str,
    url: &str,
    cancel_flag: &AtomicBool,
    start_progress: u8,
    end_progress: u8,
    status: &str,
) -> Result<Vec<u8>, String> {
    ensure_not_cancelled(cancel_flag)?;
    emit_transfer_progress(app, download_id, start_progress, status, "xml", 0, None, 0);

    let client = reqwest::blocking::Client::new();
    let mut response = client.get(url).send().map_err(bos_request_error)?;
    let response_status = response.status();

    if !response_status.is_success() {
        return Err(format!(
            "BOS download failed (HTTP {})",
            response_status.as_u16()
        ));
    }

    let total = response.content_length();
    let mut downloaded = 0_u64;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        ensure_not_cancelled(cancel_flag)?;

        let read_count = response
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;

        if read_count == 0 {
            break;
        }

        downloaded += read_count as u64;
        bytes.extend_from_slice(&buffer[..read_count]);
        emit_transfer_progress(
            app,
            download_id,
            progress_between(start_progress, end_progress, downloaded, total),
            status,
            "xml",
            downloaded,
            total,
            0,
        );
    }

    emit_transfer_progress(
        app,
        download_id,
        end_progress,
        status,
        "xml",
        downloaded,
        total.or(Some(downloaded)),
        0,
    );
    Ok(bytes)
}

fn bos_request_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "BOS request timed out".to_string()
    } else if error.is_connect() {
        "BOS connection failed".to_string()
    } else {
        "BOS network request failed".to_string()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedContentRange {
    start: Option<u64>,
    end: Option<u64>,
    total: Option<u64>,
}

fn parse_content_range(value: &str) -> Option<ParsedContentRange> {
    let value = value.trim();
    let range_and_total = value.strip_prefix("bytes ")?;
    let (range, total) = range_and_total.split_once('/')?;
    let total = if total == "*" {
        None
    } else {
        total.parse::<u64>().ok()
    };

    if range == "*" {
        return Some(ParsedContentRange {
            start: None,
            end: None,
            total,
        });
    }

    let (start, end) = range.split_once('-')?;
    Some(ParsedContentRange {
        start: Some(start.parse::<u64>().ok()?),
        end: Some(end.parse::<u64>().ok()?),
        total,
    })
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PartialDownloadMetadata {
    template_version: String,
    etag: Option<String>,
}

fn read_partial_download_metadata(path: &Path) -> Option<PartialDownloadMetadata> {
    let content = fs::read(path).ok()?;
    serde_json::from_slice(&content).ok()
}

fn write_partial_download_metadata(
    path: &Path,
    metadata: &PartialDownloadMetadata,
) -> Result<(), String> {
    let content = serde_json::to_vec(metadata).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

fn remove_file_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn clear_partial_download(
    output_path: &Path,
    partial_path: &Path,
    metadata_path: &Path,
) -> Result<(), String> {
    remove_file_if_exists(output_path)?;
    remove_file_if_exists(partial_path)?;
    remove_file_if_exists(metadata_path)
}

fn validate_partial_download_version(
    output_path: &Path,
    partial_path: &Path,
    metadata_path: &Path,
    template_version: &str,
) -> Result<Option<PartialDownloadMetadata>, String> {
    let metadata = read_partial_download_metadata(metadata_path);
    let has_download_file = output_path.is_file() || partial_path.is_file();
    let version_matches = metadata
        .as_ref()
        .map(|value| value.template_version == template_version)
        .unwrap_or(false);

    if has_download_file && !version_matches {
        clear_partial_download(output_path, partial_path, metadata_path)?;
        return Ok(None);
    }

    Ok(metadata)
}

#[allow(clippy::too_many_arguments)]
fn download_resumable_to_file(
    app: &AppHandle,
    download_id: &str,
    url: &str,
    output_path: &Path,
    partial_path: &Path,
    metadata_path: &Path,
    template_version: &str,
    cancel_flag: &AtomicBool,
    start_progress: u8,
    end_progress: u8,
    status: &str,
) -> Result<(), String> {
    ensure_not_cancelled(cancel_flag)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let client = reqwest::blocking::Client::new();
    let mut metadata = validate_partial_download_version(
        output_path,
        partial_path,
        metadata_path,
        template_version,
    )?;

    for restart_attempt in 0..2 {
        ensure_not_cancelled(cancel_flag)?;

        let resume_offset = if partial_path.is_file() {
            fs::metadata(partial_path)
                .map_err(|error| error.to_string())?
                .len()
        } else {
            0
        };
        let download_status = if resume_offset > 0 {
            "正在续传素材包..."
        } else {
            status
        };
        emit_transfer_progress(
            app,
            download_id,
            start_progress,
            download_status,
            "assets",
            resume_offset,
            None,
            resume_offset,
        );

        let mut request = client.get(url);
        if resume_offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={resume_offset}-"));
            if let Some(etag) = metadata
                .as_ref()
                .and_then(|value| value.etag.as_deref())
                .filter(|value| !value.trim().is_empty())
            {
                request = request.header(reqwest::header::IF_MATCH, etag);
            }
        }

        let mut response = request.send().map_err(bos_request_error)?;
        let response_status = response.status();

        if response_status == reqwest::StatusCode::PRECONDITION_FAILED {
            clear_partial_download(output_path, partial_path, metadata_path)?;
            metadata = None;
            if restart_attempt == 0 {
                continue;
            }
        }

        if response_status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            let total = response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_content_range)
                .and_then(|value| value.total);

            if resume_offset > 0 && total == Some(resume_offset) {
                remove_file_if_exists(output_path)?;
                fs::rename(partial_path, output_path).map_err(|error| error.to_string())?;
                emit_transfer_progress(
                    app,
                    download_id,
                    end_progress,
                    download_status,
                    "assets",
                    resume_offset,
                    total,
                    resume_offset,
                );
                return Ok(());
            }

            clear_partial_download(output_path, partial_path, metadata_path)?;
            metadata = None;
            if restart_attempt == 0 {
                continue;
            }
        }

        if !response_status.is_success() {
            return Err(format!(
                "BOS download failed (HTTP {})",
                response_status.as_u16()
            ));
        }

        let parsed_content_range = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_content_range);
        let is_partial_response = response_status == reqwest::StatusCode::PARTIAL_CONTENT;
        let append_to_partial = resume_offset > 0 && is_partial_response;

        if append_to_partial
            && parsed_content_range.as_ref().and_then(|value| value.start) != Some(resume_offset)
        {
            clear_partial_download(output_path, partial_path, metadata_path)?;
            metadata = None;
            if restart_attempt == 0 {
                continue;
            }
            return Err("BOS resume response range is invalid".to_string());
        }

        let response_etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if append_to_partial {
            let previous_etag = metadata.as_ref().and_then(|value| value.etag.as_deref());
            if previous_etag.is_some()
                && response_etag.as_deref().is_some()
                && previous_etag != response_etag.as_deref()
            {
                clear_partial_download(output_path, partial_path, metadata_path)?;
                metadata = None;
                if restart_attempt == 0 {
                    continue;
                }
                return Err("BOS object changed while resuming download".to_string());
            }
        }

        let total = parsed_content_range
            .as_ref()
            .and_then(|value| value.total)
            .or_else(|| {
                response.content_length().map(|length| {
                    if append_to_partial {
                        resume_offset.saturating_add(length)
                    } else {
                        length
                    }
                })
            });
        let active_resume_offset = if append_to_partial { resume_offset } else { 0 };
        metadata = Some(PartialDownloadMetadata {
            template_version: template_version.to_string(),
            etag: response_etag.or_else(|| metadata.as_ref().and_then(|value| value.etag.clone())),
        });
        let active_metadata = metadata
            .as_ref()
            .ok_or_else(|| "Partial download metadata is missing".to_string())?;
        write_partial_download_metadata(metadata_path, active_metadata)?;

        let mut options = OpenOptions::new();
        options.create(true).write(true);
        if append_to_partial {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let mut output_file = options
            .open(partial_path)
            .map_err(|error| error.to_string())?;
        let mut downloaded = active_resume_offset;
        let mut buffer = [0_u8; 64 * 1024];
        emit_transfer_progress(
            app,
            download_id,
            progress_between(start_progress, end_progress, downloaded, total),
            download_status,
            "assets",
            downloaded,
            total,
            active_resume_offset,
        );

        loop {
            ensure_not_cancelled(cancel_flag)?;

            let read_count = response
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;

            if read_count == 0 {
                break;
            }

            output_file
                .write_all(&buffer[..read_count])
                .map_err(|error| error.to_string())?;
            downloaded += read_count as u64;
            emit_transfer_progress(
                app,
                download_id,
                progress_between(start_progress, end_progress, downloaded, total),
                download_status,
                "assets",
                downloaded,
                total,
                active_resume_offset,
            );
        }

        output_file.flush().map_err(|error| error.to_string())?;
        let final_size = fs::metadata(partial_path)
            .map_err(|error| error.to_string())?
            .len();
        if let Some(total) = total {
            if final_size != total {
                return Err(format!(
                    "BOS download incomplete: received {final_size} of {total} bytes"
                ));
            }
        }

        remove_file_if_exists(output_path)?;
        fs::rename(partial_path, output_path).map_err(|error| error.to_string())?;
        emit_transfer_progress(
            app,
            download_id,
            end_progress,
            download_status,
            "assets",
            final_size,
            total.or(Some(final_size)),
            active_resume_offset,
        );
        return Ok(());
    }

    Err("BOS resumable download could not be restarted".to_string())
}

fn extract_zip(
    app: &AppHandle,
    download_id: &str,
    zip_path: &Path,
    assets_dir: &Path,
    cancel_flag: &AtomicBool,
) -> Result<(), String> {
    let temp_assets_dir = assets_dir.with_extension("tmp");
    if temp_assets_dir.exists() {
        fs::remove_dir_all(&temp_assets_dir).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&temp_assets_dir).map_err(|error| error.to_string())?;

    let file = fs::File::open(zip_path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    let total = archive.len().max(1) as u64;

    emit_progress(app, download_id, 90, "正在解压素材...");

    for index in 0..archive.len() {
        ensure_not_cancelled(cancel_flag)?;

        let mut zipped_file = archive.by_index(index).map_err(|error| error.to_string())?;
        let Some(enclosed_name) = zipped_file.enclosed_name().map(|path| path.to_owned()) else {
            continue;
        };
        let relative_path = enclosed_name
            .strip_prefix("assets")
            .unwrap_or(enclosed_name.as_path());

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let output_path = temp_assets_dir.join(relative_path);

        if zipped_file.is_dir() {
            fs::create_dir_all(&output_path).map_err(|error| error.to_string())?;
        } else {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }

            let mut output_file =
                fs::File::create(&output_path).map_err(|error| error.to_string())?;
            let mut buffer = [0_u8; 64 * 1024];

            loop {
                ensure_not_cancelled(cancel_flag)?;

                let read_count = zipped_file
                    .read(&mut buffer)
                    .map_err(|error| error.to_string())?;
                if read_count == 0 {
                    break;
                }

                io::Write::write_all(&mut output_file, &buffer[..read_count])
                    .map_err(|error| error.to_string())?;
            }
        }

        emit_progress(
            app,
            download_id,
            progress_between(90, 99, (index + 1) as u64, Some(total)),
            "正在解压素材...",
        );
    }

    if assets_dir.exists() {
        fs::remove_dir_all(assets_dir).map_err(|error| error.to_string())?;
    }
    fs::rename(&temp_assets_dir, assets_dir).map_err(|error| error.to_string())?;

    Ok(())
}

fn prepare_template_assets_blocking(
    app: AppHandle,
    template_id: String,
    template_version: String,
    template_file_url: String,
    material_package_url: String,
    download_id: String,
    cancel_flag: Arc<AtomicBool>,
) -> Result<PreparedTemplate, String> {
    let (template_dir, template_file_path, assets_dir) = cached_template_paths(&template_id)?;
    fs::create_dir_all(&template_dir).map_err(|error| error.to_string())?;
    let material_package_path = template_dir.join("materials.zip");
    let partial_package_path = template_dir.join("materials.zip.part");
    let partial_metadata_path = template_dir.join("materials.zip.part.json");

    let result = (|| {
        ensure_not_cancelled(&cancel_flag)?;
        emit_progress(&app, &download_id, 5, "正在检查本地模板资源...");

        let cached_xml_content = if template_file_path.is_file() {
            Some(fs::read_to_string(&template_file_path).map_err(|error| error.to_string())?)
        } else {
            None
        };
        let local_xml_version_matches = cached_xml_content
            .as_ref()
            .map(|xml_content| xml_matches_template_version(xml_content, &template_version))
            .unwrap_or(false);

        if cached_xml_content.is_some() && !local_xml_version_matches {
            emit_progress(&app, &download_id, 8, "本地模板版本已更新，正在重新下载...");
            if assets_dir.exists() {
                fs::remove_dir_all(&assets_dir).map_err(|error| error.to_string())?;
            }
            if material_package_path.exists() {
                fs::remove_file(&material_package_path).map_err(|error| error.to_string())?;
            }
            remove_file_if_exists(&partial_package_path)?;
            remove_file_if_exists(&partial_metadata_path)?;
            fs::remove_file(&template_file_path).map_err(|error| error.to_string())?;
        }

        let mut xml_content = if local_xml_version_matches {
            emit_progress(&app, &download_id, 15, "已找到本地模板文件...");
            cached_xml_content.unwrap_or_default()
        } else {
            let template_url = resolve_url("", &template_file_url)?;
            let xml_bytes = download_bytes(
                &app,
                &download_id,
                &template_url,
                &cancel_flag,
                5,
                10,
                "正在下载模板文件...",
            )?;
            let xml_content =
                String::from_utf8(xml_bytes.clone()).map_err(|error| error.to_string())?;
            fs::write(&template_file_path, xml_bytes).map_err(|error| error.to_string())?;
            xml_content
        };

        ensure_not_cancelled(&cancel_flag)?;

        if local_xml_version_matches && assets_dir.is_dir() {
            emit_progress(&app, &download_id, 100, "已加载本地模板资源");
        } else {
            let package_metadata = validate_partial_download_version(
                &material_package_path,
                &partial_package_path,
                &partial_metadata_path,
                &template_version,
            )?;

            if !material_package_path.is_file() {
                let package_url = resolve_url("", &material_package_url)?;
                download_resumable_to_file(
                    &app,
                    &download_id,
                    &package_url,
                    &material_package_path,
                    &partial_package_path,
                    &partial_metadata_path,
                    &template_version,
                    &cancel_flag,
                    10,
                    90,
                    "正在下载素材包...",
                )?;
            } else if package_metadata.is_some() {
                emit_progress(&app, &download_id, 90, "素材包已下载，正在继续解压...");
            }

            let extract_result = extract_zip(
                &app,
                &download_id,
                &material_package_path,
                &assets_dir,
                &cancel_flag,
            );
            if let Err(error) = extract_result {
                if !cancel_flag.load(Ordering::Relaxed) {
                    let _ = remove_file_if_exists(&material_package_path);
                    let _ = remove_file_if_exists(&partial_metadata_path);
                }
                return Err(error);
            }
            remove_file_if_exists(&material_package_path)?;
            remove_file_if_exists(&partial_metadata_path)?;
            emit_progress(&app, &download_id, 100, "模板资源已准备完成");
        }

        xml_content = normalize_template_file_resource_paths(
            &template_file_path,
            &template_dir,
            &assets_dir,
            xml_content,
        )?;

        Ok(PreparedTemplate {
            template_dir: template_dir.to_string_lossy().to_string(),
            template_file_path: template_file_path.to_string_lossy().to_string(),
            material_package_path: String::new(),
            assets_dir: assets_dir.to_string_lossy().to_string(),
            xml_content,
        })
    })();

    let _ = remove_download_task(&download_id);

    result
}

#[tauri::command]
fn get_cached_template_assets(
    template_id: String,
    template_version: String,
) -> Result<Option<PreparedTemplate>, String> {
    read_cached_template_assets(&template_id, &template_version)
}

#[tauri::command]
fn read_original_template_xml(template_id: String) -> Result<String, String> {
    if template_id.trim().is_empty() {
        return Err("templateId 不能为空".to_string());
    }
    let (_, template_file_path, _) = cached_template_paths(&template_id)?;
    if !template_file_path.is_file() {
        return Err("原始模板 template.xml 不存在".to_string());
    }
    fs::read_to_string(template_file_path).map_err(|error| error.to_string())
}

#[tauri::command]
async fn prepare_template_assets(
    app: AppHandle,
    template_id: String,
    template_version: String,
    template_file_url: String,
    material_package_url: String,
    download_id: String,
) -> Result<PreparedTemplate, String> {
    let cancel_flag = register_download_task(&download_id)?;
    let app_handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        prepare_template_assets_blocking(
            app_handle,
            template_id,
            template_version,
            template_file_url,
            material_package_url,
            download_id,
            cancel_flag,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn cancel_template_download(download_id: String) -> Result<(), String> {
    let tasks = download_tasks().lock().map_err(|error| error.to_string())?;

    if let Some(flag) = tasks.get(&download_id) {
        flag.store(true, Ordering::Relaxed);
    }

    Ok(())
}

#[tauri::command]
fn ensure_default_output_dir() -> Result<String, String> {
    ensure_aicut_output_dir().map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
async fn download_help_guide(
    api_base_url: String,
    authorization_token: String,
    output_dir: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        download_help_guide_blocking(api_base_url, authorization_token, output_dir)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn create_project_workspace(
    template_id: String,
    project_id: String,
) -> Result<ProjectWorkspace, String> {
    let (_, project_root) = ensure_aicut_dirs()?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let workspace_id = format!(
        "{}-{}",
        sanitize_name(&template_id),
        sanitize_name(&project_id)
    );
    let project_dir = project_root.join(&workspace_id);

    fs::create_dir_all(&project_dir).map_err(|error| error.to_string())?;
    let (template_dir, template_file_path, assets_dir) = cached_template_paths(&template_id)?;
    let template_xml =
        fs::read_to_string(&template_file_path).map_err(|error| error.to_string())?;
    let template_xml = normalize_template_file_resource_paths(
        &template_file_path,
        &template_dir,
        &assets_dir,
        template_xml,
    )?;
    let project_file_xml = generate_project_file_xml(&template_xml, &workspace_id, timestamp)?;
    fs::write(project_dir.join("template.xml"), &template_xml)
        .map_err(|error| error.to_string())?;
    fs::write(project_dir.join("projectFile.xml"), project_file_xml)
        .map_err(|error| error.to_string())?;

    Ok(ProjectWorkspace {
        project_dir: project_dir.to_string_lossy().to_string(),
        template_file_path: path_to_xml_filepath(project_dir.join("template.xml")),
        project_xml: template_xml,
    })
}

#[tauri::command]
fn read_project_workspace(
    template_id: String,
    project_id: String,
) -> Result<LocalProjectWorkspace, String> {
    let (_, project_root) = ensure_aicut_dirs()?;
    let workspace_id = format!(
        "{}-{}",
        sanitize_name(&template_id),
        sanitize_name(&project_id)
    );
    let project_dir = project_root.join(workspace_id);
    if !project_dir.is_dir() {
        return Err("本地工程目录不存在".to_string());
    }

    let project_dir = fs::canonicalize(project_dir).map_err(|error| error.to_string())?;
    let template_file_path = project_dir.join("template.xml");
    let project_file_path = project_dir.join("projectFile.xml");
    if !template_file_path.is_file() || !project_file_path.is_file() {
        return Err("本地工程缺少 template.xml 或 projectFile.xml".to_string());
    }

    let template_xml =
        fs::read_to_string(&template_file_path).map_err(|error| error.to_string())?;
    let project_file_xml =
        fs::read_to_string(&project_file_path).map_err(|error| error.to_string())?;
    let existing_asset_ids = find_xml_start_tags(&template_xml, "asset")
        .into_iter()
        .filter_map(|tag| {
            let asset_id = xml_attribute_value(&tag, "id")?;
            let filepath = xml_attribute_value(&tag, "filepath")?;
            PathBuf::from(filepath).is_file().then_some(asset_id)
        })
        .collect();

    Ok(LocalProjectWorkspace {
        project_dir: path_to_xml_filepath(project_dir.clone()),
        template_file_path: path_to_xml_filepath(template_file_path),
        assets_dir: path_to_xml_filepath(project_dir.join("assets")),
        template_xml,
        project_file_xml,
        existing_asset_ids,
    })
}

#[tauri::command]
fn save_project_asset(
    project_dir: String,
    asset_id: String,
    source_path: String,
) -> Result<ProjectAssetImport, String> {
    if asset_id.trim().is_empty() {
        return Err("assetId 不能为空".to_string());
    }

    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;

    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let source_path =
        fs::canonicalize(PathBuf::from(source_path)).map_err(|error| error.to_string())?;
    if !source_path.is_file() {
        return Err("选择的视频文件不存在".to_string());
    }

    let source_file_name = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(sanitize_file_name)
        .unwrap_or_else(|| "video.mp4".to_string());
    let source_hash = file_content_hash(&source_path)?;
    let target_file_name = format!("{source_hash}_{source_file_name}");
    let assets_dir = project_dir.join("assets");
    let target_path = assets_dir.join(&target_file_name);
    let project_filepath = path_to_xml_filepath(target_path.clone());
    let project_file_path = project_dir.join("projectFile.xml");
    let project_template_path = project_dir.join("template.xml");
    let project_file_xml =
        fs::read_to_string(&project_file_path).map_err(|error| error.to_string())?;
    let updated_project_file_xml =
        update_project_asset_filepath(&project_file_xml, &asset_id, &project_filepath)?;
    let project_template_xml =
        fs::read_to_string(&project_template_path).map_err(|error| error.to_string())?;
    let updated_project_template_xml =
        update_project_asset_filepath(&project_template_xml, &asset_id, &project_filepath)?;

    fs::create_dir_all(&assets_dir).map_err(|error| error.to_string())?;
    if !target_path.is_file() {
        fs::copy(&source_path, &target_path).map_err(|error| error.to_string())?;
    }
    fs::write(&project_file_path, updated_project_file_xml).map_err(|error| error.to_string())?;
    fs::write(&project_template_path, &updated_project_template_xml)
        .map_err(|error| error.to_string())?;

    Ok(ProjectAssetImport {
        copied_path: target_path.to_string_lossy().to_string(),
        project_filepath,
        project_xml: updated_project_template_xml,
    })
}

#[tauri::command]
fn update_project_asset_offset(
    project_dir: String,
    asset_id: String,
    offset_ms: u64,
    area_offsets: Option<Vec<ProjectAreaOffsetUpdate>>,
) -> Result<(), String> {
    if asset_id.trim().is_empty() {
        return Err("assetId 不能为空".to_string());
    }

    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;

    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let project_file_path = project_dir.join("projectFile.xml");
    let project_file_xml =
        fs::read_to_string(&project_file_path).map_err(|error| error.to_string())?;
    let updated_project_file_xml =
        if let Some(area_offsets) = area_offsets.as_ref().filter(|offsets| !offsets.is_empty()) {
            update_project_clip_area_offsets(&project_file_xml, &asset_id, area_offsets)?
        } else {
            update_project_clip_offsets(&project_file_xml, &asset_id, offset_ms)?
        };

    fs::write(&project_file_path, updated_project_file_xml).map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
fn update_project_asset_properties(
    project_dir: String,
    asset_id: String,
    properties: ProjectAssetProperties,
) -> Result<String, String> {
    let asset_id = asset_id.trim();
    if asset_id.is_empty() {
        return Err("assetId 不能为空".to_string());
    }

    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;
    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let template_file_path = project_dir.join("template.xml");
    let project_file_path = project_dir.join("projectFile.xml");
    let template_xml =
        fs::read_to_string(&template_file_path).map_err(|error| error.to_string())?;
    let project_file_xml =
        fs::read_to_string(&project_file_path).map_err(|error| error.to_string())?;
    let properties = normalize_project_asset_properties(properties)?;
    let updated_template_xml =
        update_template_asset_properties(&template_xml, asset_id, &properties)?;
    let updated_project_file_xml =
        update_template_asset_properties(&project_file_xml, asset_id, &properties)?;
    fs::write(&template_file_path, &updated_template_xml).map_err(|error| error.to_string())?;
    fs::write(&project_file_path, updated_project_file_xml).map_err(|error| error.to_string())?;

    Ok(updated_template_xml)
}

#[tauri::command]
fn apply_project_asset_generated_video(
    project_dir: String,
    asset_id: String,
    preview_video_path: String,
    mut properties: ProjectAssetProperties,
) -> Result<ProjectGeneratedAsset, String> {
    let asset_id = asset_id.trim();
    if asset_id.is_empty() {
        return Err("assetId 不能为空".to_string());
    }

    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;
    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let preview_video_path = fs::canonicalize(PathBuf::from(preview_video_path))
        .map_err(|error| format!("生成的视频不存在: {error}"))?;
    if !preview_video_path.is_file() {
        return Err("生成的视频路径不是文件".to_string());
    }

    let generated_dir = project_dir.join("generated");
    fs::create_dir_all(&generated_dir).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let extension = preview_video_path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("mp4");
    let generated_path = generated_dir.join(format!(
        "{}_{}.{}",
        sanitize_name(asset_id),
        timestamp,
        extension
    ));
    let generate_path = path_to_xml_filepath(generated_path);
    let template_file_path = project_dir.join("template.xml");
    let project_file_path = project_dir.join("projectFile.xml");
    let template_xml =
        fs::read_to_string(&template_file_path).map_err(|error| error.to_string())?;
    let project_file_xml =
        fs::read_to_string(&project_file_path).map_err(|error| error.to_string())?;
    let template_xml =
        update_project_asset_generatepath(&template_xml, asset_id, None).unwrap_or(template_xml);
    let project_file_xml = update_project_asset_generatepath(&project_file_xml, asset_id, None)
        .unwrap_or(project_file_xml);
    properties.generatepath = Some(generate_path.clone());
    let properties = normalize_project_asset_properties(properties)?;
    let updated_template_xml =
        update_template_asset_properties(&template_xml, asset_id, &properties)?;
    let updated_project_file_xml =
        update_template_asset_properties(&project_file_xml, asset_id, &properties)?;

    fs::copy(&preview_video_path, &generate_path)
        .map_err(|error| format!("保存生成视频失败: {error}"))?;
    fs::write(&template_file_path, &updated_template_xml).map_err(|error| error.to_string())?;
    fs::write(&project_file_path, updated_project_file_xml).map_err(|error| error.to_string())?;

    Ok(ProjectGeneratedAsset {
        generate_path,
        project_xml: updated_template_xml,
    })
}

#[tauri::command]
fn reset_project_asset_generated_video(
    project_dir: String,
    asset_id: String,
) -> Result<String, String> {
    let asset_id = asset_id.trim();
    if asset_id.is_empty() {
        return Err("assetId 不能为空".to_string());
    }

    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;
    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let template_file_path = project_dir.join("template.xml");
    let project_file_path = project_dir.join("projectFile.xml");
    let template_xml =
        fs::read_to_string(&template_file_path).map_err(|error| error.to_string())?;
    let project_file_xml =
        fs::read_to_string(&project_file_path).map_err(|error| error.to_string())?;
    let mut generated_paths = collect_asset_generated_paths(&template_xml, asset_id);
    generated_paths.extend(collect_asset_generated_paths(&project_file_xml, asset_id));
    let updated_template_xml =
        remove_asset_area_property_element(&template_xml, asset_id, "generatepath")?;
    let updated_project_file_xml =
        remove_asset_area_property_element(&project_file_xml, asset_id, "generatepath")?;
    let updated_template_xml =
        update_project_asset_generatepath(&updated_template_xml, asset_id, None)
            .unwrap_or(updated_template_xml);
    let updated_project_file_xml =
        update_project_asset_generatepath(&updated_project_file_xml, asset_id, None)
            .unwrap_or(updated_project_file_xml);

    let generated_dir = project_dir.join("generated");
    let canonical_generated_dir = generated_dir
        .is_dir()
        .then(|| fs::canonicalize(&generated_dir))
        .transpose()
        .map_err(|error| format!("读取 generated 目录失败: {error}"))?;
    if let Some(canonical_generated_dir) = canonical_generated_dir {
        for generated_path in generated_paths {
            let generated_path = PathBuf::from(generated_path);
            if !generated_path.exists() {
                continue;
            }
            let canonical_generated_path = fs::canonicalize(&generated_path)
                .map_err(|error| format!("读取生成视频失败: {error}"))?;
            if canonical_generated_path.starts_with(&canonical_generated_dir)
                && canonical_generated_path.is_file()
            {
                fs::remove_file(&canonical_generated_path)
                    .map_err(|error| format!("删除生成视频失败: {error}"))?;
            }
        }
    }

    fs::write(&template_file_path, &updated_template_xml).map_err(|error| error.to_string())?;
    fs::write(&project_file_path, updated_project_file_xml).map_err(|error| error.to_string())?;

    Ok(updated_template_xml)
}

#[tauri::command]
fn apply_project_subtitle(project_dir: String, text: String) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("请输入内容".to_string());
    }

    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;

    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let project_file_path = project_dir.join("projectFile.xml");
    let project_template_path = project_dir.join("template.xml");
    let project_template_xml =
        fs::read_to_string(&project_template_path).map_err(|error| error.to_string())?;
    let subtitle = find_first_template_subtitle(&project_template_xml)
        .ok_or_else(|| "工程 template.xml 中未找到 subtitle".to_string())?;
    let project_file_xml =
        fs::read_to_string(&project_file_path).map_err(|error| error.to_string())?;
    let updated_project_file_xml = update_project_subtitle(&project_file_xml, &subtitle, text)?;
    let updated_project_template_xml =
        update_template_subtitle_default(&project_template_xml, &subtitle, text)?;

    fs::write(&project_file_path, updated_project_file_xml).map_err(|error| error.to_string())?;
    fs::write(&project_template_path, &updated_project_template_xml)
        .map_err(|error| error.to_string())?;

    Ok(updated_project_template_xml)
}

fn normalize_composer_output_path(value: &str) -> Result<PathBuf, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("输出文件路径不能为空".to_string());
    }

    let mut output_path = PathBuf::from(value);
    if !output_path.is_absolute() {
        return Err("输出文件路径必须是绝对路径".to_string());
    }
    if output_path.file_name().is_none() {
        return Err("输出文件名不能为空".to_string());
    }

    match output_path
        .extension()
        .and_then(|extension| extension.to_str())
    {
        None | Some("") => {
            output_path.set_extension("mp4");
        }
        Some(extension) if extension.eq_ignore_ascii_case("mp4") => {}
        Some(_) => return Err("导出文件必须使用 .mp4 后缀".to_string()),
    }

    Ok(output_path)
}

#[tauri::command]
async fn compose_project_video(
    app: AppHandle,
    composer: tauri::State<'_, ComposerState>,
    template_path: String,
    project_dir: String,
    output_path: String,
    export_id: String,
) -> Result<ComposerExportResult, String> {
    app_log_info(format!(
        "[composer] compose_project_video requested export_id={export_id}"
    ));
    let template_path = PathBuf::from(template_path);
    app_log_info(format!(
        "[composer] validating template path: {}",
        template_path.display()
    ));
    if !template_path.is_file() {
        return Err("模板 XML 文件不存在".to_string());
    }

    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;
    app_log_info(format!(
        "[composer] validating project dir: {}",
        project_dir.display()
    ));
    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let project_path = project_dir.join("projectFile.xml");
    app_log_info(format!(
        "[composer] validating project xml: {}",
        project_path.display()
    ));
    if !project_path.is_file() {
        return Err("projectFile.xml 不存在".to_string());
    }

    let output_path = normalize_composer_output_path(&output_path)?;
    let output_dir = output_path
        .parent()
        .ok_or_else(|| "输出文件缺少父目录".to_string())?;
    app_log_info(format!(
        "[composer] ensuring selected output parent dir: {}",
        output_dir.display()
    ));
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    if !output_dir.is_dir() {
        return Err("输出目录无效".to_string());
    }

    app_log_info(format!("[composer] output file: {}", output_path.display()));
    let output_path_string = output_path.to_string_lossy().to_string();
    let template_path_string = template_path.to_string_lossy().to_string();
    let project_path_string = project_path.to_string_lossy().to_string();
    let composer = composer.inner().clone();
    let export_id_for_progress = export_id.clone();
    let app_for_progress = app.clone();

    emit_composer_progress(&app, &export_id, 0, "正在准备导出...");

    app_log_info("[composer] spawning blocking compose task");
    tauri::async_runtime::spawn_blocking(move || {
        let composer = composer.lock().map_err(|error| error.to_string())?;
        let _wake_guard = match ExportWakeGuard::acquire() {
            Ok(guard) => {
                if guard.is_active() {
                    app_log_info("[power] export wake lock acquired");
                } else {
                    app_log_info(
                        "[power] export wake lock is unsupported on this platform; continuing",
                    );
                }
                Some(guard)
            }
            Err(error) => {
                app_log_error(format!(
                    "[power] failed to acquire export wake lock; continuing export: {error}"
                ));
                None
            }
        };
        composer.compose_video(
            &template_path_string,
            &project_path_string,
            &output_path_string,
            app_for_progress,
            export_id_for_progress,
        )
    })
    .await
    .map_err(|error| error.to_string())??;

    app_log_info(format!(
        "[composer] compose_project_video finished export_id={export_id}"
    ));
    emit_composer_progress(&app, &export_id, 100, "导出完成");

    Ok(ComposerExportResult {
        output_path: output_path.to_string_lossy().to_string(),
    })
}

fn sanitize_preview_file_stem(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    let sanitized = sanitized.trim().trim_matches('.');

    if sanitized.is_empty() {
        "video".to_string()
    } else {
        sanitized.to_string()
    }
}

fn reset_image_temp_dir(image_temp_dir: &Path) -> Result<(), String> {
    if image_temp_dir.exists() {
        let metadata = fs::symlink_metadata(image_temp_dir)
            .map_err(|error| format!("读取 imageTemp 目录失败: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err("imageTemp 不能是软链接".to_string());
        }
        if !metadata.is_dir() {
            return Err("imageTemp 路径不是目录".to_string());
        }

        for entry in fs::read_dir(image_temp_dir)
            .map_err(|error| format!("读取 imageTemp 目录失败: {error}"))?
        {
            let entry = entry.map_err(|error| format!("读取 imageTemp 内容失败: {error}"))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("读取旧预览文件失败: {error}"))?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                fs::remove_dir_all(&path)
                    .map_err(|error| format!("清理旧预览目录失败: {error}"))?;
            } else {
                fs::remove_file(&path).map_err(|error| format!("清理旧预览文件失败: {error}"))?;
            }
        }
    } else {
        fs::create_dir_all(image_temp_dir)
            .map_err(|error| format!("创建 imageTemp 目录失败: {error}"))?;
    }

    Ok(())
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn prepare_beauty_frame_params(
    app: &AppHandle,
    mut params: ComposerBeautyFrameParams,
) -> Result<ComposerBeautyFrameParams, String> {
    params.whiteness = finite_or(params.whiteness, 0.0).clamp(0.0, 1.0);
    params.smoothing = finite_or(params.smoothing, 0.0).clamp(0.0, 1.0);
    params.saturation = finite_or(params.saturation, 100.0).clamp(0.0, 200.0);
    params.skin_tone = finite_or(params.skin_tone, 0.0).clamp(-1.0, 1.0);
    params.face_detect = 1;
    params.rotation = finite_or(params.rotation, 0.0);
    params.lut_intensity = finite_or(params.lut_intensity, 0.0).clamp(0.0, 1.0);
    params.position_x = finite_or(params.position_x, 0.0);
    params.position_y = finite_or(params.position_y, 0.0);
    params.scale = finite_or(params.scale, 1.0).clamp(0.01, 10.0);
    params.canvas_width = if params.canvas_width == 0 {
        1920
    } else {
        params.canvas_width.clamp(1, 16_384)
    };
    params.canvas_height = if params.canvas_height == 0 {
        1080
    } else {
        params.canvas_height.clamp(1, 16_384)
    };
    params.transform_origin = match params.transform_origin.trim().to_ascii_lowercase().as_str() {
        "" | "center" => "center".to_string(),
        _ => return Err("当前仅支持以 center 作为视频变换原点".to_string()),
    };
    let lut_file = params
        .lut_file
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let Some(lut_file) = lut_file else {
        params.lut_intensity = 0.0;
        return Ok(params);
    };

    let relative_path = Path::new(&lut_file);
    if relative_path.is_absolute()
        || relative_path.components().any(|component| {
            !matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err("LUT 文件路径无效".to_string());
    }

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| error.to_string())?;
    let lut_root = fs::canonicalize(resource_dir.join("luts"))
        .map_err(|error| format!("LUT 资源目录不可用: {error}"))?;
    let relative_lut_path = relative_path.strip_prefix("luts").unwrap_or(relative_path);
    let lut_path = fs::canonicalize(lut_root.join(relative_lut_path))
        .map_err(|error| format!("LUT 文件不存在: {error}"))?;
    if !lut_path.starts_with(&lut_root) || !lut_path.is_file() {
        return Err("LUT 文件路径无效".to_string());
    }
    let extension = lut_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "cube" | "3dl" | "dat" | "m3d" | "csp") {
        return Err("LUT 文件格式不受支持".to_string());
    }

    params.lut_file = Some(path_to_xml_filepath(lut_path));
    Ok(params)
}

#[tauri::command]
async fn preview_composer_beauty_frame(
    app: AppHandle,
    composer: tauri::State<'_, ComposerState>,
    input_video_path: String,
    timestamp_ms: i64,
    params: ComposerBeautyFrameParams,
) -> Result<ComposerBeautyFrameResult, String> {
    if timestamp_ms < 0 {
        return Err("预览时间不能小于 0".to_string());
    }

    let input_video_path = fs::canonicalize(PathBuf::from(input_video_path))
        .map_err(|error| format!("输入视频不存在: {error}"))?;
    if !input_video_path.is_file() {
        return Err("输入视频路径不是文件".to_string());
    }
    let video_dir = input_video_path
        .parent()
        .ok_or_else(|| "无法确定输入视频所在目录".to_string())?;
    let image_temp_dir = video_dir.join("imageTemp");
    reset_image_temp_dir(&image_temp_dir)?;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let video_name = input_video_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_preview_file_stem)
        .unwrap_or_else(|| "video".to_string());
    let output_stem = format!("{now_ms}_{video_name}");
    let output_image_path = image_temp_dir.join(format!("{output_stem}.png"));
    let params_json_path = image_temp_dir.join(format!("{output_stem}.json"));
    let params = prepare_beauty_frame_params(&app, params)?;
    let json_params = serde_json::to_string_pretty(&params).map_err(|error| error.to_string())?;
    fs::write(&params_json_path, &json_params)
        .map_err(|error| format!("保存美颜参数失败: {error}"))?;

    let input_video_path_text = input_video_path.to_string_lossy().to_string();
    let output_image_path_text = output_image_path.to_string_lossy().to_string();
    let params_json_path_text = params_json_path.to_string_lossy().to_string();
    let composer = composer.inner().clone();
    let output_image_path_for_call = output_image_path_text.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let composer = composer.lock().map_err(|error| error.to_string())?;
        composer.beauty_process_frame(
            &input_video_path_text,
            timestamp_ms,
            &output_image_path_for_call,
            &json_params,
        )
    })
    .await
    .map_err(|error| error.to_string())??;

    if !output_image_path.is_file() {
        return Err("美颜接口执行成功，但未生成预览图片".to_string());
    }

    Ok(ComposerBeautyFrameResult {
        output_image_path: output_image_path_text,
        params_json_path: params_json_path_text,
        timestamp_ms,
    })
}

#[tauri::command]
async fn preview_composer_beauty_file(
    app: AppHandle,
    composer: tauri::State<'_, ComposerState>,
    input_video_path: String,
    start_time_ms: i64,
    duration_ms: i64,
    params: ComposerBeautyFrameParams,
) -> Result<ComposerBeautyFileResult, String> {
    let start_time_ms = start_time_ms.max(0);
    let duration_ms = duration_ms.max(0);
    let input_video_path = fs::canonicalize(PathBuf::from(input_video_path))
        .map_err(|error| format!("输入视频不存在: {error}"))?;
    if !input_video_path.is_file() {
        return Err("输入视频路径不是文件".to_string());
    }
    let video_dir = input_video_path
        .parent()
        .ok_or_else(|| "无法确定输入视频所在目录".to_string())?;
    let image_temp_dir = video_dir.join("imageTemp");
    reset_image_temp_dir(&image_temp_dir)?;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let video_name = input_video_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_preview_file_stem)
        .unwrap_or_else(|| "video".to_string());
    let output_stem = format!("{now_ms}_{video_name}");
    let output_video_path = image_temp_dir.join(format!("{output_stem}.mp4"));
    let params_json_path = image_temp_dir.join(format!("{output_stem}.json"));
    let params = prepare_beauty_frame_params(&app, params)?;
    let json_params = serde_json::to_string_pretty(&params).map_err(|error| error.to_string())?;
    fs::write(&params_json_path, &json_params)
        .map_err(|error| format!("保存美颜参数失败: {error}"))?;

    let input_video_path_text = input_video_path.to_string_lossy().to_string();
    let output_video_path_text = output_video_path.to_string_lossy().to_string();
    let params_json_path_text = params_json_path.to_string_lossy().to_string();
    let composer = composer.inner().clone();
    let output_video_path_for_call = output_video_path_text.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let composer = composer.lock().map_err(|error| error.to_string())?;
        composer.beauty_process_file(
            &input_video_path_text,
            &output_video_path_for_call,
            start_time_ms,
            duration_ms,
            &json_params,
        )
    })
    .await
    .map_err(|error| error.to_string())??;

    if !output_video_path.is_file() {
        return Err("美颜接口执行成功，但未生成预览视频".to_string());
    }

    Ok(ComposerBeautyFileResult {
        output_video_path: output_video_path_text,
        params_json_path: params_json_path_text,
        start_time_ms,
        duration_ms,
    })
}

#[tauri::command]
fn read_project_cover(project_dir: String) -> Result<tauri::ipc::Response, String> {
    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;

    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let cover_path = project_dir.join("title.png");
    if !cover_path.is_file() {
        return Err("项目封面不存在".to_string());
    }

    let cover_path = fs::canonicalize(cover_path).map_err(|error| error.to_string())?;
    if !cover_path.starts_with(&project_dir) {
        return Err("项目封面路径无效".to_string());
    }

    let cover_bytes = fs::read(&cover_path).map_err(|error| error.to_string())?;
    app_log_info(format!(
        "[export] project cover read path={} bytes={}",
        cover_path.display(),
        cover_bytes.len()
    ));

    Ok(tauri::ipc::Response::new(cover_bytes))
}

#[tauri::command]
fn delete_project_asset_files(project_dir: String, asset_paths: Vec<String>) -> Result<(), String> {
    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;

    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let assets_dir = project_dir.join("assets");
    if !assets_dir.is_dir() {
        return Ok(());
    }

    let assets_dir = fs::canonicalize(assets_dir).map_err(|error| error.to_string())?;
    let project_file_path = project_dir.join("projectFile.xml");
    let referenced_filepaths = if project_file_path.is_file() {
        fs::read_to_string(&project_file_path)
            .map(|xml| collect_project_asset_filepaths(&xml))
            .map_err(|error| error.to_string())?
    } else {
        HashSet::new()
    };

    for asset_path in asset_paths {
        let path = PathBuf::from(asset_path);
        if !path.exists() {
            continue;
        }

        let path = fs::canonicalize(path).map_err(|error| error.to_string())?;
        if path.starts_with(&assets_dir) && path.is_file() {
            let still_referenced = project_filepath_candidates_from_asset_path(&project_dir, &path)
                .iter()
                .any(|project_filepath| referenced_filepaths.contains(project_filepath));

            if still_referenced {
                continue;
            }

            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
fn delete_project_workspaces(project_ids: Vec<String>) -> Result<(), String> {
    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_ids = project_ids
        .into_iter()
        .map(|project_id| project_id.trim().to_string())
        .filter(|project_id| {
            !project_id.is_empty()
                && project_id
                    .chars()
                    .all(|character| character.is_ascii_digit())
        })
        .collect::<HashSet<_>>();

    if project_ids.is_empty() {
        return Ok(());
    }

    for entry in fs::read_dir(&project_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let folder_name = entry.file_name().to_string_lossy().to_string();
        let Some((_, project_id)) = folder_name.rsplit_once('-') else {
            continue;
        };
        if !project_ids.contains(project_id) {
            continue;
        }

        let workspace = fs::canonicalize(&path).map_err(|error| error.to_string())?;
        if !workspace.starts_with(&project_root) || workspace == project_root {
            return Err("本地工程目录无效".to_string());
        }
        fs::remove_dir_all(workspace).map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn get_machine_code() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-WmiObject Win32_ComputerSystemProduct | Select-Object -ExpandProperty UUID",
            ])
            .output()
            .map_err(|error| error.to_string())?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }

        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("system_profiler")
            .arg("SPHardwareDataType")
            .output()
            .map_err(|error| error.to_string())?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let machine_code = stdout
            .lines()
            .find_map(|line| {
                let line = line.trim();
                line.strip_prefix("Hardware UUID:")
                    .map(|value| value.trim().to_string())
            })
            .unwrap_or_default();

        if machine_code.is_empty() {
            return Err("Hardware UUID not found".to_string());
        }

        return Ok(machine_code);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("Unsupported platform".to_string())
    }
}

#[tauri::command]
fn get_terminal_info() -> Result<TerminalInfo, String> {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("cmd")
            .args(["/C", "echo %COMPUTERNAME%"])
            .output()
            .map_err(|error| error.to_string())?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }

        return Ok(TerminalInfo {
            terminal_type: 2,
            terminal_name: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        });
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("scutil")
            .args(["--get", "ComputerName"])
            .output()
            .map_err(|error| error.to_string())?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }

        return Ok(TerminalInfo {
            terminal_type: 1,
            terminal_name: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        });
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("Unsupported platform".to_string())
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum MacMenuLanguage {
    SimplifiedChinese,
    TraditionalChinese,
    English,
}

#[cfg(target_os = "macos")]
struct MacMenuLabels {
    about: &'static str,
    services: &'static str,
    hide: &'static str,
    hide_others: &'static str,
    quit: &'static str,
    file: &'static str,
    close_window: &'static str,
    edit: &'static str,
    undo: &'static str,
    redo: &'static str,
    cut: &'static str,
    copy: &'static str,
    paste: &'static str,
    select_all: &'static str,
    view: &'static str,
    fullscreen: &'static str,
    window: &'static str,
    minimize: &'static str,
    zoom: &'static str,
    help: &'static str,
}

#[cfg(target_os = "macos")]
fn preferred_macos_menu_language() -> MacMenuLanguage {
    use objc2_foundation::NSLocale;

    let primary_language = NSLocale::preferredLanguages()
        .firstObject()
        .map(|language| language.to_string())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if primary_language.starts_with("zh-hant")
        || primary_language.starts_with("zh-tw")
        || primary_language.starts_with("zh-hk")
        || primary_language.starts_with("zh-mo")
    {
        MacMenuLanguage::TraditionalChinese
    } else if primary_language.starts_with("zh") {
        MacMenuLanguage::SimplifiedChinese
    } else {
        MacMenuLanguage::English
    }
}

#[cfg(target_os = "macos")]
fn mac_menu_labels(language: MacMenuLanguage) -> Option<MacMenuLabels> {
    match language {
        MacMenuLanguage::SimplifiedChinese => Some(MacMenuLabels {
            about: "关于",
            services: "服务",
            hide: "隐藏",
            hide_others: "隐藏其他",
            quit: "退出",
            file: "文件",
            close_window: "关闭窗口",
            edit: "编辑",
            undo: "撤销",
            redo: "重做",
            cut: "剪切",
            copy: "拷贝",
            paste: "粘贴",
            select_all: "全选",
            view: "视图",
            fullscreen: "进入全屏幕",
            window: "窗口",
            minimize: "最小化",
            zoom: "缩放",
            help: "帮助",
        }),
        MacMenuLanguage::TraditionalChinese => Some(MacMenuLabels {
            about: "關於",
            services: "服務",
            hide: "隱藏",
            hide_others: "隱藏其他",
            quit: "結束",
            file: "檔案",
            close_window: "關閉視窗",
            edit: "編輯",
            undo: "還原",
            redo: "重做",
            cut: "剪下",
            copy: "拷貝",
            paste: "貼上",
            select_all: "全選",
            view: "顯示方式",
            fullscreen: "進入全螢幕",
            window: "視窗",
            minimize: "縮到最小",
            zoom: "縮放",
            help: "輔助說明",
        }),
        MacMenuLanguage::English => None,
    }
}

#[cfg(target_os = "macos")]
fn build_macos_menu<R: tauri::Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{
        AboutMetadata, Menu, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID, WINDOW_SUBMENU_ID,
    };

    let Some(labels) = mac_menu_labels(preferred_macos_menu_language()) else {
        return Menu::default(app);
    };

    let package_info = app.package_info();
    let app_name = package_info.name.clone();
    let about_metadata = AboutMetadata {
        name: Some(app_name.clone()),
        version: Some(package_info.version.to_string()),
        copyright: app.config().bundle.copyright.clone(),
        authors: app
            .config()
            .bundle
            .publisher
            .clone()
            .map(|publisher| vec![publisher]),
        ..Default::default()
    };

    let app_menu = Submenu::with_items(
        app,
        app_name,
        true,
        &[
            &PredefinedMenuItem::about(app, Some(labels.about), Some(about_metadata))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, Some(labels.services))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some(labels.hide))?,
            &PredefinedMenuItem::hide_others(app, Some(labels.hide_others))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some(labels.quit))?,
        ],
    )?;
    let file_menu = Submenu::with_items(
        app,
        labels.file,
        true,
        &[&PredefinedMenuItem::close_window(
            app,
            Some(labels.close_window),
        )?],
    )?;
    let edit_menu = Submenu::with_items(
        app,
        labels.edit,
        true,
        &[
            &PredefinedMenuItem::undo(app, Some(labels.undo))?,
            &PredefinedMenuItem::redo(app, Some(labels.redo))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some(labels.cut))?,
            &PredefinedMenuItem::copy(app, Some(labels.copy))?,
            &PredefinedMenuItem::paste(app, Some(labels.paste))?,
            &PredefinedMenuItem::select_all(app, Some(labels.select_all))?,
        ],
    )?;
    let view_menu = Submenu::with_items(
        app,
        labels.view,
        true,
        &[&PredefinedMenuItem::fullscreen(
            app,
            Some(labels.fullscreen),
        )?],
    )?;
    let window_menu = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        labels.window,
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some(labels.minimize))?,
            &PredefinedMenuItem::maximize(app, Some(labels.zoom))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, Some(labels.close_window))?,
        ],
    )?;
    let help_menu = Submenu::with_id_and_items(app, HELP_SUBMENU_ID, labels.help, true, &[])?;

    Menu::with_items(
        app,
        &[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &window_menu,
            &help_menu,
        ],
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());

    #[cfg(target_os = "macos")]
    let builder = builder.menu(build_macos_menu);

    builder
        .setup(|app| {
            if let Err(error) = ensure_aicut_dirs() {
                eprintln!("[app] failed to ensure aicut dirs: {error}");
            }
            app_log_info("[app] setup start");
            let composer = ComposerRuntime::initialize();
            app.manage(Arc::new(Mutex::new(composer)));
            app_log_info("[app] composer state managed");
            app.manage(Arc::new(Mutex::new(None::<PrBridgeRuntime>)));
            app_log_info("[app] PR bridge state managed");
            app.manage(Arc::new(Mutex::new(
                VecDeque::<PrTemplateExportEvent>::new(),
            )));
            app_log_info("[app] PR bridge inbox managed");

            if let Some(window) = app.get_webview_window("main") {
                app_log_info("[app] configuring main window");
                let _ = window.set_background_color(Some(Color(7, 18, 42, 255)));
            }
            app_log_info("[app] setup complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                app_log_info("[app] close requested, cleaning composer");
                if let Some(composer) = window.try_state::<ComposerState>() {
                    if let Ok(mut composer) = composer.lock() {
                        composer.cleanup();
                    } else {
                        app_log_error("[app] failed to lock composer during close");
                    }
                } else {
                    app_log_error("[app] composer state not found during close");
                }
                if let Some(bridge) = window.try_state::<PrBridgeState>() {
                    if let Ok(mut bridge) = bridge.lock() {
                        if let Some(runtime) = bridge.as_mut() {
                            shutdown_pr_bridge(runtime);
                        }
                        *bridge = None;
                    } else {
                        app_log_error("[app] failed to lock PR bridge during close");
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            start_pr_bridge,
            stop_pr_bridge,
            take_pr_template_exports,
            get_cached_template_assets,
            read_original_template_xml,
            prepare_template_assets,
            cancel_template_download,
            ensure_default_output_dir,
            download_help_guide,
            create_project_workspace,
            read_project_workspace,
            save_project_asset,
            update_project_asset_offset,
            update_project_asset_properties,
            apply_project_asset_generated_video,
            reset_project_asset_generated_video,
            apply_project_subtitle,
            compose_project_video,
            preview_composer_beauty_frame,
            preview_composer_beauty_file,
            read_project_cover,
            delete_project_asset_files,
            delete_project_workspaces,
            get_machine_code,
            get_terminal_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_beauty_transform_coordinate_contract() {
        let params = ComposerBeautyFrameParams {
            rotation: -450.0,
            saturation: 100.0,
            position_x: 960.0,
            position_y: 540.0,
            scale: 1.0,
            canvas_width: 1920,
            canvas_height: 1080,
            transform_origin: "center".to_string(),
            ..Default::default()
        };
        let value = serde_json::to_value(params).expect("serialize beauty transform params");

        assert_eq!(value["positionX"], 960.0);
        assert_eq!(value["positionY"], 540.0);
        assert_eq!(value["saturation"], 100.0);
        assert_eq!(value["scale"], 1.0);
        assert_eq!(value["rotation"], -450.0);
        assert_eq!(value["canvas_width"], 1920);
        assert_eq!(value["canvas_height"], 1080);
        assert_eq!(value["transform_origin"], "center");
        assert!(value.get("rotation_direction").is_none());
    }

    #[test]
    fn appends_and_updates_properties_for_every_area_using_asset() {
        let template_xml = r#"<xmeml><template id="template-1"><clips>
            <clip id="clip-1">
                <area id="area-1" asset-id="asset-1">
                    <source><duration>1000</duration></source>
                </area>
            </clip>
            <clip id="clip-2">
                <area id="area-2" asset-id="asset-1">
                    <property><saturation>80.0</saturation></property>
                </area>
            </clip>
            <clip id="clip-3">
                <area id="area-3" asset-id="asset-2"><source><duration>1000</duration></source></area>
            </clip>
        </clips></template></xmeml>"#;
        let properties = ProjectAssetProperties {
            whiteness: 0.2,
            smoothing: 0.3,
            saturation: 122.0,
            skin_tone: -0.4,
            face_detect: 1,
            rotation: -450.0,
            lut_style: "lut-009".to_string(),
            lut_intensity: 0.8,
            position_x: 480.0,
            position_y: 270.0,
            scale: 0.5,
            canvas_width: 960,
            canvas_height: 540,
            transform_origin: "center".to_string(),
            stabilization: false,
            one_click_beauty: false,
            generatepath: None,
        };

        let properties =
            normalize_project_asset_properties(properties).expect("normalize asset properties");
        let updated = update_template_asset_properties(template_xml, "asset-1", &properties)
            .expect("update asset properties");
        let updated_again = update_template_asset_properties(&updated, "asset-1", &properties)
            .expect("update asset properties again");

        assert_eq!(updated.matches("<property>").count(), 2);
        assert_eq!(updated_again, updated);
        assert_eq!(updated.matches("<saturation>122.0</saturation>").count(), 2);
        assert_eq!(updated.matches("<lut_style>lut-009</lut_style>").count(), 2);
        assert_eq!(updated.matches("<rotation>-450.0</rotation>").count(), 2);
        assert!(!updated.contains("<rotation_direction>"));
        assert_eq!(updated.matches("<positionX>480.0</positionX>").count(), 2);
        assert_eq!(
            updated
                .matches("<one_click_beauty>false</one_click_beauty>")
                .count(),
            2
        );
        assert!(updated.contains(
            "<source><duration>1000</duration></source>\n                    <property>"
        ));
        assert!(updated.contains(
            "<area id=\"area-3\" asset-id=\"asset-2\"><source><duration>1000</duration></source></area>"
        ));
    }

    #[test]
    fn expands_project_file_area_and_persists_generated_path_in_property() {
        let project_file_xml = r#"<project><clips><clip>
            <area id="area-1" asset-id="asset-1" offset="904" />
        </clip></clips></project>"#;
        let properties = ProjectAssetProperties {
            whiteness: 0.2,
            smoothing: 0.3,
            saturation: 122.0,
            skin_tone: -0.4,
            face_detect: 1,
            rotation: 15.0,
            lut_style: "lut-009".to_string(),
            lut_intensity: 0.8,
            position_x: 480.0,
            position_y: 270.0,
            scale: 0.5,
            canvas_width: 960,
            canvas_height: 540,
            transform_origin: "center".to_string(),
            stabilization: false,
            one_click_beauty: false,
            generatepath: Some("/project/generated/asset-1.mp4".to_string()),
        };

        let updated = update_template_asset_properties(project_file_xml, "asset-1", &properties)
            .expect("expand projectFile area");
        assert!(!updated.contains(r#"asset-id="asset-1" offset="904" />"#));
        assert!(updated.contains(r#"asset-id="asset-1" offset="904">"#));
        assert!(updated.contains("<generatepath>/project/generated/asset-1.mp4</generatepath>"));
        assert!(updated.contains("</property>\n            </area>"));
        assert_eq!(
            collect_asset_generated_paths(&updated, "asset-1"),
            HashSet::from(["/project/generated/asset-1.mp4".to_string()])
        );

        let reset = remove_asset_area_property_element(&updated, "asset-1", "generatepath")
            .expect("remove generated path property");
        assert!(!reset.contains("<generatepath>"));
        assert!(reset.contains("<saturation>122.0</saturation>"));
    }

    #[test]
    fn normalizes_custom_composer_output_path() {
        let output_dir = if cfg!(windows) {
            PathBuf::from(r"C:\Users\aicut\Videos")
        } else {
            PathBuf::from("/Users/aicut/Videos")
        };
        let without_extension = output_dir.join("我的视频");
        let normalized = normalize_composer_output_path(&without_extension.to_string_lossy())
            .expect("normalize output path");

        assert_eq!(normalized, output_dir.join("我的视频.mp4"));
        assert!(normalize_composer_output_path("relative/video.mp4").is_err());
        assert!(
            normalize_composer_output_path(&output_dir.join("我的视频.mov").to_string_lossy())
                .is_err()
        );
    }

    #[test]
    fn parses_satisfied_content_range() {
        assert_eq!(
            parse_content_range("bytes 1024-2047/4096"),
            Some(ParsedContentRange {
                start: Some(1024),
                end: Some(2047),
                total: Some(4096),
            })
        );
    }

    #[test]
    fn parses_unsatisfied_content_range() {
        assert_eq!(
            parse_content_range("bytes */4096"),
            Some(ParsedContentRange {
                start: None,
                end: None,
                total: Some(4096),
            })
        );
    }

    #[test]
    fn rejects_invalid_content_range() {
        assert_eq!(parse_content_range("1024-2047/4096"), None);
        assert_eq!(parse_content_range("bytes invalid/4096"), None);
    }

    #[test]
    fn generates_project_file_xml_from_template() {
        let template_xml = r#"<xmeml version="5">
    <template id="seqvvgcrrjs0yizf4tn" name="测试模板" version="1.0" timeunit="millisecond">
        <media-asset id="serhw8q52e9zp4s273w" name="素材集">
            <default-asset>
                <asset id="i3o6p9a2s5d8f1g4j7q0w" filepath="template/assets/1.mp4"/>
            </default-asset>
        </media-asset>
        <clips id="z7x1c4v8b2n5m9q0w3e6r" target-track="clips">
            <clip id="u4p7a0d3f6g9j2k5p8s1t" name="片段">
                <area id="e7r0t3y6u1i4o7p9a2s5d" asset-id="i3o6p9a2s5d8f1g4j7q0w">
                    <source>
                        <duration>5000</duration>
                    </source>
                </area>
            </clip>
        </clips>
    </template>
</xmeml>"#;
        let project_xml =
            generate_project_file_xml(template_xml, "tpl-test-1000", 1000).expect("project xml");

        assert!(project_xml.contains("<project id=\"tpl-test-1000\""));
        assert!(project_xml.contains("<template id=\"seqvvgcrrjs0yizf4tn\" version=\"1.0\">"));
        let last_update_time = project_xml
            .split("<last-updtime>")
            .nth(1)
            .and_then(|value| value.split("</last-updtime>").next())
            .expect("last update time");

        assert_eq!(last_update_time.len(), 19);
        assert_eq!(&last_update_time[4..5], "-");
        assert_eq!(&last_update_time[7..8], "-");
        assert_eq!(&last_update_time[10..11], " ");
        assert_eq!(&last_update_time[13..14], ":");
        assert_eq!(&last_update_time[16..17], ":");
        assert!(!project_xml.contains("<last-updtime>1000</last-updtime>"));
        assert!(project_xml.contains("<media-asset id=\"serhw8q52e9zp4s273w\">"));
        assert!(project_xml.contains("filepath=\"template/assets/1.mp4\""));
        assert!(!project_xml.contains("<media-assets>"));
        assert!(project_xml.contains("<clips id=\"z7x1c4v8b2n5m9q0w3e6r\" target-track=\"clips\">"));
        assert!(project_xml.contains(
            "<area id=\"e7r0t3y6u1i4o7p9a2s5d\" asset-id=\"i3o6p9a2s5d8f1g4j7q0w\" offset=\"0\" />"
        ));
        assert!(!project_xml.contains("<source>"));
    }

    #[test]
    fn generates_project_clips_with_default_clips_attributes() {
        let template_xml = r#"<xmeml version="5">
    <template id="template-a" name="Template A" version="1.0" timeunit="millisecond">
        <clips>
            <clip id="clip-a">
                <area id="area-a" asset-id="asset-a">
                    <source>
                        <duration>5000</duration>
                    </source>
                </area>
            </clip>
        </clips>
    </template>
</xmeml>"#;
        let project_xml =
            generate_project_file_xml(template_xml, "project-a", 1000).expect("project xml");

        assert!(project_xml.contains(r#"<clips id="clips" target-track="clips">"#));
        assert!(project_xml.contains(r#"id="area-a" asset-id="asset-a" offset="0""#));
    }

    #[test]
    fn normalizes_template_resource_paths_to_absolute_paths() {
        let template_dir = PathBuf::from(if cfg!(windows) {
            r"C:\aicut\templates\tpl"
        } else {
            "/Users/aicut/templates/tpl"
        });
        let assets_dir = template_dir.join("assets");
        let xml = r#"<template>
            <video>
                <demo-path>template/assets/template.mp4</demo-path>
            </video>
            <tracks>
                <track id="bg">
                    <filepath>common/background.mp4</filepath>
                </track>
            </tracks>
            <media-asset id="group-a">
                <default-asset>
                    <asset id="asset-a" filepath="template/assets/1.mp4" />
                </default-asset>
            </media-asset>
        </template>"#;

        let normalized = normalize_template_resource_paths(xml, &template_dir, &assets_dir);

        assert!(normalized.contains(&format!(
            r#"filepath="{}""#,
            path_to_xml_filepath(assets_dir.join("1.mp4"))
        )));
        assert!(normalized.contains(&format!(
            "<demo-path>{}</demo-path>",
            escape_xml_text(&path_to_xml_filepath(assets_dir.join("template.mp4")))
        )));
        assert!(normalized.contains(&format!(
            "<filepath>{}</filepath>",
            escape_xml_text(&path_to_xml_filepath(
                assets_dir.join("common/background.mp4")
            ))
        )));

        if cfg!(windows) {
            assert!(!normalized.contains(r"\assets\common/background.mp4"));
            assert!(normalized.contains(r"\assets\common\background.mp4"));
        } else {
            assert!(!normalized.contains(r"/assets/common\background.mp4"));
            assert!(normalized.contains("/assets/common/background.mp4"));
        }
    }

    #[test]
    fn removes_windows_verbatim_prefix_from_xml_filepaths() {
        if !cfg!(windows) {
            return;
        }

        assert_eq!(
            path_to_xml_filepath(PathBuf::from(r"\\?\C:\aicut\project\1-80\assets\video.mp4")),
            r"C:\aicut\project\1-80\assets\video.mp4"
        );
        assert_eq!(
            path_to_xml_filepath(PathBuf::from(r"\\?\UNC\server\share\video.mp4")),
            r"\\server\share\video.mp4"
        );
    }

    #[test]
    fn updates_project_asset_filepath_by_asset_id() {
        let project_xml = r#"<project>
        <media-asset id="group-a">
            <asset id="asset-a" filepath="template/assets/1.mp4" />
            <asset id="asset-b" filepath="template/assets/2.mp4" />
        </media-asset>
    </project>"#;
        let updated_xml =
            update_project_asset_filepath(project_xml, "asset-b", "project/assets/demo.mp4")
                .expect("updated xml");

        assert!(updated_xml.contains(r#"id="asset-a" filepath="template/assets/1.mp4""#));
        assert!(updated_xml.contains(r#"id="asset-b" filepath="project/assets/demo.mp4""#));
    }

    #[test]
    fn adds_updates_and_removes_project_asset_generatepath() {
        let template_xml = r#"<template><assets>
            <asset id="asset-a" filepath="/project/assets/a.mp4" />
            <asset id="asset-b" filepath="/project/assets/b.mp4" generatepath="/old.mp4" />
        </assets></template>"#;

        let added = update_project_asset_generatepath(
            template_xml,
            "asset-a",
            Some("/project/generated/a.mp4"),
        )
        .expect("add generatepath");
        assert!(added.contains(
            r#"id="asset-a" filepath="/project/assets/a.mp4" generatepath="/project/generated/a.mp4""#
        ));

        let updated =
            update_project_asset_generatepath(&added, "asset-b", Some("/project/generated/b.mp4"))
                .expect("update generatepath");
        assert!(updated.contains(r#"generatepath="/project/generated/b.mp4""#));

        let removed = update_project_asset_generatepath(&updated, "asset-b", None)
            .expect("remove generatepath");
        assert!(removed.contains(r#"id="asset-b" filepath="/project/assets/b.mp4" />"#));
        assert!(!removed.contains("/project/generated/b.mp4"));
    }

    #[test]
    fn collects_project_asset_filepaths() {
        let project_xml = r#"<project>
        <media-asset id="group-a">
            <asset id="asset-a" filepath="project/assets/shared.mp4" />
            <asset id="asset-b" filepath="project/assets/shared.mp4" />
        </media-asset>
    </project>"#;
        let filepaths = collect_project_asset_filepaths(project_xml);

        assert_eq!(filepaths.len(), 1);
        assert!(filepaths.contains("project/assets/shared.mp4"));
    }

    #[test]
    fn updates_all_project_clip_offsets_by_asset_id() {
        let project_xml = r#"<project>
        <clips id="clips" target-track="clips">
            <clip id="clip-a">
                <area id="area-a" asset-id="asset-a" offset="0" />
                <area id="area-b" asset-id="asset-b" offset="0" />
            </clip>
            <clip id="clip-b">
                <area id="area-c" asset-id="asset-a" offset="1200" />
            </clip>
        </clips>
    </project>"#;
        let updated_xml =
            update_project_clip_offsets(project_xml, "asset-a", 2500).expect("updated xml");

        assert!(updated_xml.contains(r#"id="area-a" asset-id="asset-a" offset="2500""#));
        assert!(updated_xml.contains(r#"id="area-b" asset-id="asset-b" offset="0""#));
        assert!(updated_xml.contains(r#"id="area-c" asset-id="asset-a" offset="2500""#));
    }

    #[test]
    fn updates_project_clip_offsets_by_area_id() {
        let project_xml = r#"<project>
        <clips id="clips" target-track="clips">
            <clip id="clip-a">
                <area id="area-a" asset-id="asset-a" offset="0" />
                <area id="area-b" asset-id="asset-a" offset="0" />
                <area id="area-c" asset-id="asset-b" offset="0" />
            </clip>
        </clips>
    </project>"#;
        let area_offsets = vec![
            ProjectAreaOffsetUpdate {
                area_id: "area-a".to_string(),
                offset_ms: 10_000,
            },
            ProjectAreaOffsetUpdate {
                area_id: "area-b".to_string(),
                offset_ms: 12_000,
            },
        ];
        let updated_xml = update_project_clip_area_offsets(project_xml, "asset-a", &area_offsets)
            .expect("updated xml");

        assert!(updated_xml.contains(r#"id="area-a" asset-id="asset-a" offset="10000""#));
        assert!(updated_xml.contains(r#"id="area-b" asset-id="asset-a" offset="12000""#));
        assert!(updated_xml.contains(r#"id="area-c" asset-id="asset-b" offset="0""#));
    }

    #[test]
    fn applies_first_template_subtitle_to_project_clip() {
        let template_xml = r#"<template>
        <clips id="clips" target-track="clips">
            <clip id="clip-a">
                <subtitle id="subtitle-a" absoluteStartTime="1000" duration="3000">
                    <default>默认标题</default>
                </subtitle>
            </clip>
            <clip id="clip-b">
                <subtitle id="subtitle-b"></subtitle>
            </clip>
        </clips>
    </template>"#;
        let project_xml = r#"<project>
        <clips id="clips" target-track="clips">
            <clip id="clip-a">
                <area id="area-a" asset-id="asset-a" offset="0" />
                <subtitle id="old-a" text="旧标题" />
            </clip>
            <clip id="clip-b">
                <subtitle id="old-b" text="旧标题 2" />
            </clip>
        </clips>
    </project>"#;
        let subtitle = find_first_template_subtitle(template_xml).expect("subtitle");
        let updated_xml =
            update_project_subtitle(project_xml, &subtitle, "新标题").expect("updated xml");

        assert!(updated_xml.contains(
            r#"<subtitle id="subtitle-a" text="新标题" absoluteStartTime="1000" duration="3000" />"#
        ));
        assert!(!updated_xml.contains("old-a"));
        assert!(!updated_xml.contains("old-b"));
        assert!(!updated_xml.contains("subtitle-b"));
    }

    #[test]
    fn updates_project_template_subtitle_default_text() {
        let template_xml = r#"<template>
        <clips id="clips" target-track="clips">
            <clip id="clip-a">
                <subtitle id="subtitle-a">
                    <default>默认标题</default>
                </subtitle>
            </clip>
            <clip id="clip-b">
                <subtitle id="subtitle-b">
                    <default>其他标题</default>
                </subtitle>
            </clip>
        </clips>
    </template>"#;
        let subtitle = TemplateSubtitle {
            clip_id: "clip-a".to_string(),
            id: "subtitle-a".to_string(),
            absolute_start_time: None,
            duration: None,
        };
        let updated_xml =
            update_template_subtitle_default(template_xml, &subtitle, "新标题 & 内容")
                .expect("updated template xml");

        assert!(updated_xml.contains("<default>新标题 &amp; 内容</default>"));
        assert!(updated_xml.contains("<default>其他标题</default>"));
        assert!(!updated_xml.contains("<default>默认标题</default>"));
    }

    #[test]
    fn updates_self_closing_project_template_subtitle_text() {
        let template_xml = r#"<template>
        <clips id="clips">
            <clip id="clip-a">
                <subtitle id="subtitle-a" text="" />
            </clip>
        </clips>
    </template>"#;
        let subtitle = TemplateSubtitle {
            clip_id: "clip-a".to_string(),
            id: "subtitle-a".to_string(),
            absolute_start_time: None,
            duration: None,
        };
        let updated_xml = update_template_subtitle_default(template_xml, &subtitle, "新标题")
            .expect("updated template xml");

        assert!(updated_xml.contains(r#"<subtitle id="subtitle-a" text="新标题" />"#));
    }

    #[test]
    fn maps_composer_steps_to_display_statuses() {
        let expected = [
            "初始化",
            "预处理片段",
            "合成画中画",
            "合并转场",
            "构建最终视频",
            "添加字幕",
            "混流音频",
            "合成完成",
        ];

        for (step, status) in expected.into_iter().enumerate() {
            assert_eq!(composer_step_status(step as i32), status);
        }
        assert_eq!(composer_step_status(-1), "正在合成视频...");
        assert_eq!(composer_step_status(8), "正在合成视频...");
    }

    #[test]
    fn clears_previous_beauty_preview_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let image_temp_dir = std::env::temp_dir().join(format!(
            "aicut-image-temp-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(image_temp_dir.join("old-directory")).expect("create test directory");
        fs::write(image_temp_dir.join("old.png"), b"old image").expect("write old image");
        fs::write(image_temp_dir.join("old.json"), b"{}").expect("write old json");
        fs::write(image_temp_dir.join("old-directory/old.txt"), b"old").expect("write nested file");

        reset_image_temp_dir(&image_temp_dir).expect("reset imageTemp");

        assert!(image_temp_dir.is_dir());
        assert_eq!(
            fs::read_dir(&image_temp_dir)
                .expect("read imageTemp")
                .count(),
            0
        );
        fs::remove_dir_all(image_temp_dir).expect("remove test directory");
    }

    #[test]
    fn decodes_and_sanitizes_manual_download_filename() {
        assert_eq!(
            sanitize_manual_filename(
                "AICut%E5%AE%A2%E6%88%B7%E7%AB%AF%E4%BD%BF%E7%94%A8%E6%89%8B%E5%86%8C.docx"
            ),
            "AICut客户端使用手册.docx"
        );
        assert_eq!(
            sanitize_manual_filename("../unsafe%2Fmanual.docx"),
            "manual.docx"
        );
        assert_eq!(sanitize_manual_filename("..."), "AICut使用手册.docx");
    }
}
