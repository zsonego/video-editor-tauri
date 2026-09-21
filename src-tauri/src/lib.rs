use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    fs::OpenOptions,
    hash::{DefaultHasher, Hasher},
    io,
    io::Write,
    io::{Read, Seek, SeekFrom},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
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
static ASSET_THUMBNAIL_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static CUSTOM_STORAGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const TEMPLATE_DOWNLOAD_EVENT_NAME: &str = "template-download-progress";
#[cfg(target_os = "windows")]
const WINDOWS_RUNTIME_DOWNLOAD_EVENT_NAME: &str = "windows-runtime-download-progress";

#[cfg(target_os = "windows")]
const WINDOWS_RUNTIME_REQUIRED_DLLS: &[&str] = &[
    "cublas64_12.dll",
    "cublasLt64_12.dll",
    "cudart64_12.dll",
    "cudnn64_9.dll",
    "cudnn_adv64_9.dll",
    "cudnn_cnn64_9.dll",
    "cudnn_engines_precompiled64_9.dll",
    "cudnn_engines_runtime_compiled64_9.dll",
    "cudnn_graph64_9.dll",
    "cudnn_heuristic64_9.dll",
    "cudnn_ops64_9.dll",
    "cufft64_11.dll",
    "cufftw64_11.dll",
    "nvblas64_12.dll",
    "onnxruntime.dll",
    "onnxruntime_providers_cuda.dll",
    "onnxruntime_providers_shared.dll",
];

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
    output_directory: String,
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

fn pr_bridge_info(session_id: &str, output_directory: String) -> PrBridgeInfo {
    PrBridgeInfo {
        service: "aicut-template-bridge",
        protocol_version: PR_BRIDGE_PROTOCOL_VERSION,
        page: "create-template",
        session_id: session_id.to_string(),
        port: PR_BRIDGE_PORT,
        output_directory,
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
    let output_directory = ensure_custom_storage_dirs()?.pr_temp;
    let info = pr_bridge_info(&session_id, path_to_xml_filepath(output_directory));
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
struct SavedCustomTemplate {
    template_file_path: String,
    assets_dir: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomTemplateSummary {
    template_id: String,
    backend_template_id: Option<String>,
    status: u8,
    submission_ready: bool,
    name: String,
    duration_ms: u64,
    resolution: String,
    preview_path: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomTemplateDetail {
    template_id: String,
    status: u8,
    template_file_path: String,
    project_root: String,
    xml_content: String,
    fixed_material_path: String,
    is_pr_imported: bool,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomTemplateEditorState {
    fixed_material_file: String,
    is_pr_imported: bool,
    status: Option<u8>,
    submission_ready: Option<bool>,
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
struct ProjectAssetFingerprint {
    material_key: String,
    fingerprint: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AdaptedProjectAssetSpeed {
    project_xml: String,
    adapted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectGeneratedAsset {
    generate_path: String,
    project_xml: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreservedProjectAssetPreview {
    preview_video_path: String,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowsRuntimePreparationResult {
    ready: bool,
    downloaded: bool,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomTemplateVideoResult {
    output_path: String,
    cover_path: String,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomTemplateUploadState {
    backend_template_id: Option<String>,
    finalized: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomTemplateUploadFiles {
    xml_size: u64,
    cover_size: u64,
    assets_size: u64,
    backend_template_id: Option<String>,
    finalized: bool,
}

#[derive(Deserialize, Serialize, Default)]
struct ComposerBeautyFrameParams {
    whiteness: f64,
    smoothing: f64,
    saturation: f64,
    skin_tone: f64,
    face_detect: i32,
    rotation: f64,
    lut_file: String,
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
    #[serde(
        rename = "clipStartTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    clip_start_time: Option<serde_json::Value>,
    #[serde(
        rename = "clipDuration",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    clip_duration: Option<serde_json::Value>,
    #[serde(
        rename = "startFilterEffect",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    start_filter_effect: Option<String>,
    #[serde(
        rename = "startFilterDuration",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    start_filter_duration: Option<serde_json::Value>,
    #[serde(
        rename = "endFilterEffect",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    end_filter_effect: Option<String>,
    #[serde(
        rename = "endFilterDuration",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    end_filter_duration: Option<serde_json::Value>,
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
    compose_with_options: Option<ComposerComposeWithOptionsFn>,
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
type ComposerComposeWithOptionsFn = unsafe extern "C" fn(
    *const c_char,
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
    emit_transfer_progress_for_event(
        app,
        TEMPLATE_DOWNLOAD_EVENT_NAME,
        download_id,
        progress,
        status,
        phase,
        downloaded_bytes,
        total_bytes,
        resumed_bytes,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_transfer_progress_for_event(
    app: &AppHandle,
    event_name: &str,
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
    let _ = app.emit(event_name, payload);
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
    #[cfg(target_os = "windows")]
    fn is_available(&self) -> bool {
        self.init_error.is_none() && self.initialized
    }

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
            compose_with_options: None,
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
            let compose: Option<ComposerComposeFn> = unsafe {
                match library.get(b"composer_compose\0") {
                    Ok(symbol) => Some(*symbol),
                    Err(error) => {
                        app_log_error(format!("[composer] composer_compose unavailable: {error}"));
                        None
                    }
                }
            };
            app_log_info("[composer] resolving composer_compose_with_options");
            let compose_with_options: Option<ComposerComposeWithOptionsFn> = unsafe {
                match library.get(b"composer_compose_with_options\0") {
                    Ok(symbol) => {
                        app_log_info("[composer] composer_compose_with_options resolved");
                        Some(*symbol)
                    }
                    Err(error) => {
                        app_log_error(format!(
                            "[composer] composer_compose_with_options unavailable: {error}"
                        ));
                        None
                    }
                }
            };
            if compose.is_none() && compose_with_options.is_none() {
                return Err("Composer 动态库未提供视频合成接口".to_string());
            }
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
                compose,
                compose_with_options,
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
        json_params: &str,
        allow_legacy_fallback: bool,
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
            app_log_info(format!("[composer] json_params={json_params}"));
            let template_path_text = template_path.to_string();
            let project_path_text = project_path.to_string();
            let output_path_text = output_path.to_string();
            let json_params_text = json_params.to_string();
            let export_id_text = export_id.clone();
            if self.compose_with_options.is_none() && !allow_legacy_fallback {
                let error =
                    "当前 Composer 动态库版本不支持整片水印预览，请更新本机动态库".to_string();
                app_log_error(format!("[composer] {error}"));
                return Err(error);
            }
            let template_path =
                CString::new(template_path).map_err(|_| "模板路径包含非法字符".to_string())?;
            let project_path =
                CString::new(project_path).map_err(|_| "工程路径包含非法字符".to_string())?;
            let output_path =
                CString::new(output_path).map_err(|_| "输出路径包含非法字符".to_string())?;
            let json_params =
                CString::new(json_params).map_err(|_| "合成参数包含非法字符".to_string())?;
            let mut context = ComposerCallbackContext { app, export_id };
            let result = unsafe {
                if let Some(compose_with_options) = self.compose_with_options {
                    compose_with_options(
                        template_path.as_ptr(),
                        project_path.as_ptr(),
                        output_path.as_ptr(),
                        json_params.as_ptr(),
                        Some(composer_progress_callback),
                        (&mut context as *mut ComposerCallbackContext).cast::<c_void>(),
                    )
                } else {
                    let Some(compose) = self.compose else {
                        return Err("Composer 视频合成函数未加载".to_string());
                    };
                    app_log_info(
                        "[composer] composer_compose_with_options unavailable; using legacy composer_compose",
                    );
                    compose(
                        template_path.as_ptr(),
                        project_path.as_ptr(),
                        output_path.as_ptr(),
                        Some(composer_progress_callback),
                        (&mut context as *mut ComposerCallbackContext).cast::<c_void>(),
                    )
                }
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
                     json_params: {json_params_text}\n\
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
            let _ = (
                template_path,
                project_path,
                output_path,
                json_params,
                allow_legacy_fallback,
                app,
                export_id,
            );
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
                Err(format!("美颜图片处理失败（错误码 {result}）"))
            } else {
                Err(format!("美颜图片处理失败（错误码 {result}）: {last_error}"))
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
                Err(format!("美颜视频处理失败（错误码 {result}）"))
            } else {
                Err(format!("美颜视频处理失败（错误码 {result}）: {last_error}"))
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

struct CustomStorageDirs {
    project: PathBuf,
    preview: PathBuf,
    pr_temp: PathBuf,
}

fn migrate_directory_contents(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let destination_entry = destination.join(entry.file_name());
        if destination_entry.exists() {
            continue;
        }
        fs::rename(entry.path(), destination_entry).map_err(|error| {
            format!(
                "迁移旧模板工厂目录失败（{}）：{error}",
                entry.path().display()
            )
        })?;
    }
    if fs::read_dir(source)
        .map_err(|error| error.to_string())?
        .next()
        .is_none()
    {
        fs::remove_dir(source).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn ensure_custom_storage_dirs_at(aicut_root: &Path) -> Result<CustomStorageDirs, String> {
    let custom_root = aicut_root.join("custom");
    fs::create_dir_all(&custom_root).map_err(|error| error.to_string())?;

    let project_dir = custom_root.join("project");
    let preview_dir = custom_root.join("preview");
    let pr_temp_dir = custom_root.join("prTemp");
    fs::create_dir_all(&project_dir).map_err(|error| error.to_string())?;

    migrate_directory_contents(&custom_root.join("temp"), &pr_temp_dir)?;
    fs::create_dir_all(&pr_temp_dir).map_err(|error| error.to_string())?;

    let legacy_preview_dir = aicut_root.join("preview").join("template-factory");
    migrate_directory_contents(&legacy_preview_dir, &preview_dir)?;
    fs::create_dir_all(&preview_dir).map_err(|error| error.to_string())?;

    for entry in fs::read_dir(&custom_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        let directory_name = entry.file_name().to_string_lossy().to_string();
        if matches!(
            directory_name.to_ascii_lowercase().as_str(),
            "project" | "preview" | "prtemp" | "temp"
        ) {
            continue;
        }
        let legacy_template_dir = entry.path();
        if !legacy_template_dir.join("template.xml").is_file() {
            continue;
        }
        let destination = project_dir.join(entry.file_name());
        if destination.exists() {
            continue;
        }
        fs::rename(&legacy_template_dir, &destination).map_err(|error| {
            format!(
                "迁移旧的我的模板目录失败（{}）：{error}",
                legacy_template_dir.display()
            )
        })?;
    }

    Ok(CustomStorageDirs {
        project: project_dir,
        preview: preview_dir,
        pr_temp: pr_temp_dir,
    })
}

fn ensure_custom_storage_dirs() -> Result<CustomStorageDirs, String> {
    let _guard = CUSTOM_STORAGE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "模板工厂存储目录锁定失败".to_string())?;
    let aicut_root = aicut_root_dir()?;
    ensure_custom_storage_dirs_at(&aicut_root)
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

fn sanitize_custom_template_key(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
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
    property_inner: Option<String>,
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

fn sha256_fingerprint(reader: &mut impl Read) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read_count = reader.read(&mut buffer)?;
        if read_count == 0 {
            break;
        }
        hasher.update(&buffer[..read_count]);
    }
    let digest = hasher.finalize();
    let mut fingerprint = String::with_capacity(32);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest.iter().take(16) {
        fingerprint.push(HEX[(byte >> 4) as usize] as char);
        fingerprint.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(fingerprint)
}

fn sha256_fingerprints_for_paths(paths: &[PathBuf]) -> Vec<String> {
    if paths.is_empty() {
        return Vec::new();
    }

    const MAX_FINGERPRINT_WORKERS: usize = 4;
    let worker_count = thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1)
        .min(MAX_FINGERPRINT_WORKERS)
        .min(paths.len());
    let mut fingerprints = vec![String::new(); paths.len()];

    thread::scope(|scope| {
        let mut workers = Vec::with_capacity(worker_count);
        for worker_index in 0..worker_count {
            workers.push(scope.spawn(move || {
                let mut results = Vec::new();
                for path_index in (worker_index..paths.len()).step_by(worker_count) {
                    let fingerprint = fs::File::open(&paths[path_index])
                        .and_then(|mut file| sha256_fingerprint(&mut file))
                        .unwrap_or_default();
                    results.push((path_index, fingerprint));
                }
                results
            }));
        }

        for worker in workers {
            if let Ok(results) = worker.join() {
                for (path_index, fingerprint) in results {
                    fingerprints[path_index] = fingerprint;
                }
            }
        }
    });

    fingerprints
}

fn asset_fingerprints_from_xml(
    template_xml: &str,
    template_dir: &Path,
    assets_dir: &Path,
) -> Vec<ProjectAssetFingerprint> {
    let mut path_indices = HashMap::<PathBuf, usize>::new();
    let mut unique_paths = Vec::<PathBuf>::new();
    let mut assets = Vec::<(String, Option<usize>)>::new();
    for (_, default_assets) in find_xml_element_blocks(template_xml, "default-asset") {
        for asset_tag in find_xml_start_tags(&default_assets, "asset") {
            let Some(material_key) = xml_attribute_value(&asset_tag, "id") else {
                continue;
            };
            let filepath = xml_attribute_value(&asset_tag, "filepath").unwrap_or_default();
            let resolved = resolve_template_resource_filepath(template_dir, assets_dir, &filepath);
            let path_index = fs::canonicalize(resolved)
                .ok()
                .filter(|path| path.is_file())
                .map(|path| {
                    if let Some(path_index) = path_indices.get(&path) {
                        *path_index
                    } else {
                        let path_index = unique_paths.len();
                        unique_paths.push(path.clone());
                        path_indices.insert(path, path_index);
                        path_index
                    }
                });
            assets.push((material_key, path_index));
        }
    }

    let unique_fingerprints = sha256_fingerprints_for_paths(&unique_paths);
    assets
        .into_iter()
        .map(|(material_key, path_index)| ProjectAssetFingerprint {
            material_key,
            fingerprint: path_index
                .and_then(|index| unique_fingerprints.get(index))
                .cloned()
                .unwrap_or_default(),
        })
        .collect()
}

fn project_asset_fingerprints(project_dir: &Path) -> Result<Vec<ProjectAssetFingerprint>, String> {
    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir = fs::canonicalize(project_dir).map_err(|error| error.to_string())?;
    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }
    let template_xml = fs::read_to_string(project_dir.join("template.xml"))
        .map_err(|error| format!("读取工程模板失败: {error}"))?;
    let template_id = find_xml_element_blocks(&template_xml, "template")
        .into_iter()
        .next()
        .and_then(|(tag, _)| xml_attribute_value(&tag, "id"))
        .unwrap_or_default();
    let (template_dir, _, assets_dir) = if template_id.is_empty() {
        (
            project_dir.clone(),
            project_dir.join("template.xml"),
            project_dir.join("assets"),
        )
    } else {
        cached_template_paths(&template_id)?
    };
    Ok(asset_fingerprints_from_xml(
        &template_xml,
        &template_dir,
        &assets_dir,
    ))
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

fn remove_attribute_from_xml_start_tags(xml: &str, tag_name: &str, attribute: &str) -> String {
    let open_pattern = format!("<{tag_name}");
    let mut output = String::with_capacity(xml.len());
    let mut search_start = 0;

    while let Some(relative_start) = xml[search_start..].find(&open_pattern) {
        let tag_start = search_start + relative_start;
        let after_name = xml[tag_start + open_pattern.len()..].chars().next();
        if !is_xml_name_boundary(after_name) {
            output.push_str(&xml[search_start..tag_start + open_pattern.len()]);
            search_start = tag_start + open_pattern.len();
            continue;
        }

        let Some(relative_tag_end) = xml[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        output.push_str(&xml[search_start..tag_start]);
        output.push_str(&remove_xml_attribute(&xml[tag_start..tag_end], attribute));
        search_start = tag_end;
    }

    output.push_str(&xml[search_start..]);
    output
}

fn remove_all_xml_elements(xml: &str, element_name: &str) -> Result<String, String> {
    let opening_prefix = format!("<{element_name}");
    let closing_tag = format!("</{element_name}>");
    let mut output = xml.to_string();
    let mut search_start = 0;

    while let Some(relative_start) = output[search_start..].find(&opening_prefix) {
        let element_start = search_start + relative_start;
        let after_name = output[element_start + opening_prefix.len()..]
            .chars()
            .next();
        if !is_xml_name_boundary(after_name) {
            search_start = element_start + opening_prefix.len();
            continue;
        }

        let relative_open_end = output[element_start..]
            .find('>')
            .ok_or_else(|| format!("XML 中的 {element_name} 节点未正确闭合"))?;
        let open_end = element_start + relative_open_end + 1;
        let element_end = if output[element_start..open_end].trim_end().ends_with("/>") {
            open_end
        } else {
            let relative_close_start = output[open_end..]
                .find(&closing_tag)
                .ok_or_else(|| format!("XML 中的 {element_name} 节点未正确闭合"))?;
            open_end + relative_close_start + closing_tag.len()
        };

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
        search_start = removal_start;
    }

    Ok(output)
}

fn clear_project_generatepaths(xml: &str) -> Result<String, String> {
    let xml = remove_all_xml_elements(xml, "generatepath")?;
    Ok(remove_attribute_from_xml_start_tags(
        &xml,
        "asset",
        "generatepath",
    ))
}

fn update_project_root_identity(
    project_file_xml: &str,
    project_id: &str,
    project_name: &str,
) -> Result<String, String> {
    let open_pattern = "<project";
    let mut search_start = 0;

    while let Some(relative_start) = project_file_xml[search_start..].find(open_pattern) {
        let tag_start = search_start + relative_start;
        let after_name = project_file_xml[tag_start + open_pattern.len()..]
            .chars()
            .next();
        if !is_xml_name_boundary(after_name) {
            search_start = tag_start + open_pattern.len();
            continue;
        }
        let relative_tag_end = project_file_xml[tag_start..]
            .find('>')
            .ok_or_else(|| "projectFile.xml 中的 project 节点未正确闭合".to_string())?;
        let tag_end = tag_start + relative_tag_end + 1;
        let tag = replace_or_insert_xml_attribute(
            &project_file_xml[tag_start..tag_end],
            "id",
            project_id,
        );
        let tag = replace_or_insert_xml_attribute(&tag, "name", project_name);
        return Ok(format!(
            "{}{}{}",
            &project_file_xml[..tag_start],
            tag,
            &project_file_xml[tag_end..]
        ));
    }

    Err("projectFile.xml 缺少 project 节点".to_string())
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

fn prepare_project_asset_properties(
    app: &AppHandle,
    properties: ProjectAssetProperties,
) -> Result<ProjectAssetProperties, String> {
    let mut properties = normalize_project_asset_properties(properties)?;
    if properties.lut_style != "none" {
        properties.lut_style = resolve_lut_resource_file_path(app, &properties.lut_style)?;
    }
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

fn first_xml_element_range(content: &str, name: &str) -> Option<(usize, usize, usize, usize)> {
    let opening = format!("<{name}");
    let closing = format!("</{name}>");
    let mut search_start = 0;
    while let Some(relative_start) = content[search_start..].find(&opening) {
        let start = search_start + relative_start;
        if !is_xml_name_boundary(content[start + opening.len()..].chars().next()) {
            search_start = start + opening.len();
            continue;
        }
        let open_end = start + content[start..].find('>')? + 1;
        if content[start..open_end].trim_end().ends_with("/>") {
            search_start = open_end;
            continue;
        }
        let close_start = open_end + content[open_end..].find(&closing)?;
        return Some((start, open_end, close_start, close_start + closing.len()));
    }
    None
}

fn adapt_area_transform_speed(area_inner: &str, video_duration_ms: u64) -> (String, bool) {
    let required_ms = first_xml_element_range(area_inner, "source")
        .and_then(|(_, source_open_end, source_close_start, _)| {
            collect_xml_child_element_values(
                &area_inner[source_open_end..source_close_start],
                "duration",
            )
            .into_iter()
            .next()
        })
        .and_then(|duration| duration.parse::<u64>().ok())
        .unwrap_or(0);
    if required_ms == 0 || video_duration_ms == 0 {
        return (area_inner.to_string(), false);
    }

    let needs_slowdown = video_duration_ms < required_ms;
    let speed_thousandths = ((video_duration_ms as u128 * 1000) / required_ms as u128).max(1);
    let speed_text = format!(
        "{}.{:03}",
        speed_thousandths / 1000,
        speed_thousandths % 1000
    );

    if let Some((transform_start, transform_open_end, transform_close_start, _)) =
        first_xml_element_range(area_inner, "transform")
    {
        let transform_inner = &area_inner[transform_open_end..transform_close_start];
        if let Some((speed_start, speed_open_end, speed_close_start, speed_end)) =
            first_xml_element_range(transform_inner, "speed")
        {
            let speed_tag = &transform_inner[speed_start..speed_open_end];
            let current_speed = transform_inner[speed_open_end..speed_close_start].trim();
            let was_adapted =
                xml_attribute_value(speed_tag, "data-auto-slowdown").as_deref() == Some("true");
            if !needs_slowdown && !was_adapted {
                return (area_inner.to_string(), false);
            }

            let original_speed = if was_adapted {
                xml_attribute_value(speed_tag, "data-original-speed")
                    .unwrap_or_else(|| current_speed.to_string())
            } else {
                current_speed.to_string()
            };
            let original_speed = if original_speed.trim().is_empty() {
                "1".to_string()
            } else {
                original_speed
            };
            let (updated_tag, updated_value) = if needs_slowdown {
                let tag = replace_or_insert_xml_attribute(speed_tag, "data-auto-slowdown", "true");
                (
                    replace_or_insert_xml_attribute(&tag, "data-original-speed", &original_speed),
                    speed_text.as_str(),
                )
            } else {
                let tag = remove_xml_attribute(speed_tag, "data-auto-slowdown");
                (
                    remove_xml_attribute(&tag, "data-original-speed"),
                    original_speed.as_str(),
                )
            };
            let mut updated = area_inner.to_string();
            updated.replace_range(
                transform_open_end + speed_start..transform_open_end + speed_end,
                &format!("{updated_tag}{updated_value}</speed>"),
            );
            return (updated, needs_slowdown);
        }

        if needs_slowdown {
            let mut updated = area_inner.to_string();
            let transform_indent = area_inner[..transform_start]
                .rsplit_once('\n')
                .map(|(_, indent)| indent)
                .filter(|indent| indent.chars().all(char::is_whitespace))
                .unwrap_or("");
            let speed_indent = format!("{transform_indent}    ");
            let insertion = format!(
                "\n{speed_indent}<speed data-auto-slowdown=\"true\" data-original-speed=\"1\">{speed_text}</speed>"
            );
            let insertion_at = transform_open_end + transform_inner.trim_end().len();
            updated.insert_str(insertion_at, &insertion);
            return (updated, true);
        }
        return (area_inner.to_string(), false);
    }

    if !needs_slowdown {
        return (area_inner.to_string(), false);
    }
    let insertion_at = first_xml_element_range(area_inner, "destination")
        .or_else(|| first_xml_element_range(area_inner, "property"))
        .map(|(start, _, _, _)| start)
        .unwrap_or(area_inner.len());
    let mut updated = area_inner.to_string();
    updated.insert_str(
        insertion_at,
        &format!(
            "<transform><speed data-auto-slowdown=\"true\" data-original-speed=\"1\">{speed_text}</speed></transform>\n"
        ),
    );
    (updated, true)
}

fn adapt_template_asset_speeds(
    template_xml: &str,
    asset_id: &str,
    video_duration_ms: u64,
) -> Result<(String, bool), String> {
    let mut output = String::new();
    let mut search_start = 0;
    let mut matched = false;
    let mut adapted = false;

    while let Some(relative_start) = template_xml[search_start..].find("<area") {
        let area_start = search_start + relative_start;
        if !is_xml_name_boundary(template_xml[area_start + "<area".len()..].chars().next()) {
            output.push_str(&template_xml[search_start..area_start + "<area".len()]);
            search_start = area_start + "<area".len();
            continue;
        }
        let Some(relative_tag_end) = template_xml[area_start..].find('>') else {
            break;
        };
        let tag_end = area_start + relative_tag_end + 1;
        let area_tag = &template_xml[area_start..tag_end];
        if xml_attribute_value(area_tag, "asset-id").as_deref() != Some(asset_id)
            || area_tag.trim_end().ends_with("/>")
        {
            output.push_str(&template_xml[search_start..tag_end]);
            search_start = tag_end;
            continue;
        }
        let Some(relative_close_start) = template_xml[tag_end..].find("</area>") else {
            return Err("template.xml 中的 area 节点未正确闭合".to_string());
        };
        let close_start = tag_end + relative_close_start;
        let close_end = close_start + "</area>".len();
        let (updated_inner, area_adapted) =
            adapt_area_transform_speed(&template_xml[tag_end..close_start], video_duration_ms);
        output.push_str(&template_xml[search_start..tag_end]);
        output.push_str(&updated_inner);
        output.push_str("</area>");
        search_start = close_end;
        matched = true;
        adapted |= area_adapted;
    }

    output.push_str(&template_xml[search_start..]);
    if matched {
        Ok((output, adapted))
    } else {
        Err("template.xml 中未找到使用当前 assetId 的 area".to_string())
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
                        .filter_map(|(area_tag, area_inner)| {
                            Some(TemplateClipArea {
                                id: xml_attribute_value(&area_tag, "id")?,
                                asset_id: xml_attribute_value(&area_tag, "asset-id")?,
                                property_inner: find_xml_element_blocks(&area_inner, "property")
                                    .into_iter()
                                    .next()
                                    .map(|(_, inner)| inner),
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
                if let Some(property_inner) = &area.property_inner {
                    output.push_str(&format!(
                        "                <area id=\"{}\" asset-id=\"{}\" offset=\"0\">\n",
                        escape_xml_attribute(&area.id),
                        escape_xml_attribute(&area.asset_id)
                    ));
                    output.push_str("                    <property>\n");
                    for property_line in property_inner.lines() {
                        let property_line = property_line.trim();
                        if property_line.is_empty() {
                            continue;
                        }
                        output.push_str("                        ");
                        output.push_str(property_line);
                        output.push('\n');
                    }
                    output.push_str("                    </property>\n");
                    output.push_str("                </area>\n");
                } else {
                    output.push_str(&format!(
                        "                <area id=\"{}\" asset-id=\"{}\" offset=\"0\" />\n",
                        escape_xml_attribute(&area.id),
                        escape_xml_attribute(&area.asset_id)
                    ));
                }
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
    progress_event_name: &str,
    progress_phase: &str,
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
        emit_transfer_progress_for_event(
            app,
            progress_event_name,
            download_id,
            start_progress,
            download_status,
            progress_phase,
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
                emit_transfer_progress_for_event(
                    app,
                    progress_event_name,
                    download_id,
                    end_progress,
                    download_status,
                    progress_phase,
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
        let mut last_emitted_progress =
            progress_between(start_progress, end_progress, downloaded, total);
        emit_transfer_progress_for_event(
            app,
            progress_event_name,
            download_id,
            last_emitted_progress,
            download_status,
            progress_phase,
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
            let current_progress =
                progress_between(start_progress, end_progress, downloaded, total);
            if current_progress != last_emitted_progress {
                last_emitted_progress = current_progress;
                emit_transfer_progress_for_event(
                    app,
                    progress_event_name,
                    download_id,
                    current_progress,
                    download_status,
                    progress_phase,
                    downloaded,
                    total,
                    active_resume_offset,
                );
            }
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
        emit_transfer_progress_for_event(
            app,
            progress_event_name,
            download_id,
            end_progress,
            download_status,
            progress_phase,
            final_size,
            total.or(Some(final_size)),
            active_resume_offset,
        );
        return Ok(());
    }

    Err("BOS resumable download could not be restarted".to_string())
}

#[cfg(target_os = "windows")]
fn emit_windows_runtime_progress(
    app: &AppHandle,
    download_id: &str,
    progress: u8,
    status: &str,
    phase: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    resumed_bytes: u64,
) {
    emit_transfer_progress_for_event(
        app,
        WINDOWS_RUNTIME_DOWNLOAD_EVENT_NAME,
        download_id,
        progress,
        status,
        phase,
        downloaded_bytes,
        total_bytes,
        resumed_bytes,
    );
}

#[cfg(target_os = "windows")]
fn windows_runtime_install_dir() -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|error| format!("无法定位应用程序安装目录：{error}"))?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法定位应用程序安装目录".to_string())
}

#[cfg(target_os = "windows")]
fn missing_windows_runtime_dlls(directory: &Path) -> Vec<&'static str> {
    WINDOWS_RUNTIME_REQUIRED_DLLS
        .iter()
        .copied()
        .filter(|file_name| {
            fs::metadata(directory.join(file_name))
                .map(|metadata| !metadata.is_file() || metadata.len() == 0)
                .unwrap_or(true)
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn composer_state_is_available(composer: &ComposerState) -> Result<bool, String> {
    composer
        .lock()
        .map(|runtime| runtime.is_available())
        .map_err(|error| format!("无法读取 Composer 状态：{error}"))
}

#[cfg(target_os = "windows")]
fn reinitialize_composer(composer: &ComposerState) -> Result<(), String> {
    let new_runtime = ComposerRuntime::initialize();
    if !new_runtime.is_available() {
        return Err(new_runtime
            .init_error
            .clone()
            .unwrap_or_else(|| "Composer 初始化失败".to_string()));
    }

    let mut runtime = composer
        .lock()
        .map_err(|error| format!("无法更新 Composer 状态：{error}"))?;
    runtime.cleanup();
    *runtime = new_runtime;
    Ok(())
}

#[cfg(target_os = "windows")]
fn extract_windows_runtime_zip(
    app: &AppHandle,
    download_id: &str,
    zip_path: &Path,
    install_dir: &Path,
    cancel_flag: &AtomicBool,
) -> Result<(), String> {
    let temp_dir = install_dir.join(".aicut-runtime-extract");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|error| format!("清理临时目录失败：{error}"))?;
    }
    fs::create_dir_all(&temp_dir).map_err(|error| format!("创建临时目录失败：{error}"))?;

    let result = (|| {
        let file =
            fs::File::open(zip_path).map_err(|error| format!("打开运行库压缩包失败：{error}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|error| format!("运行库压缩包无效：{error}"))?;
        let total_entries = archive.len().max(1) as u64;
        let mut extracted_dlls = HashSet::new();

        emit_windows_runtime_progress(
            app,
            download_id,
            86,
            "正在解压运行环境...",
            "extract",
            0,
            None,
            0,
        );

        for index in 0..archive.len() {
            ensure_not_cancelled(cancel_flag)?;
            let mut zipped_file = archive
                .by_index(index)
                .map_err(|error| format!("读取压缩包失败：{error}"))?;
            if zipped_file.is_dir() {
                continue;
            }

            let Some(enclosed_name) = zipped_file.enclosed_name() else {
                continue;
            };
            let Some(file_name) = enclosed_name.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if !file_name.to_ascii_lowercase().ends_with(".dll")
                || file_name.eq_ignore_ascii_case("libcomposer.dll")
            {
                continue;
            }

            let normalized_name = file_name.to_ascii_lowercase();
            if !extracted_dlls.insert(normalized_name) {
                return Err(format!("压缩包内存在重名 DLL：{file_name}"));
            }

            let output_path = temp_dir.join(file_name);
            let mut output_file = fs::File::create(&output_path)
                .map_err(|error| format!("创建 DLL 文件失败：{error}"))?;
            io::copy(&mut zipped_file, &mut output_file)
                .map_err(|error| format!("解压 DLL 文件失败：{error}"))?;
            output_file
                .flush()
                .map_err(|error| format!("写入 DLL 文件失败：{error}"))?;

            emit_windows_runtime_progress(
                app,
                download_id,
                progress_between(86, 96, (index + 1) as u64, Some(total_entries)),
                "正在解压运行环境...",
                "extract",
                (index + 1) as u64,
                Some(total_entries),
                0,
            );
        }

        let missing = missing_windows_runtime_dlls(&temp_dir);
        if !missing.is_empty() {
            return Err(format!("运行库压缩包缺少文件：{}", missing.join("、")));
        }

        for entry in fs::read_dir(&temp_dir).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let source_path = entry.path();
            if !source_path.is_file() {
                continue;
            }
            let target_path = install_dir.join(entry.file_name());
            remove_file_if_exists(&target_path)?;
            fs::rename(&source_path, &target_path)
                .map_err(|error| format!("安装运行库失败：{error}"))?;
        }

        Ok(())
    })();

    let _ = fs::remove_dir_all(&temp_dir);
    result
}

#[cfg(target_os = "windows")]
fn prepare_windows_runtime_blocking(
    app: AppHandle,
    composer: ComposerState,
    download_url: String,
    runtime_version: String,
    download_id: String,
    cancel_flag: Arc<AtomicBool>,
) -> Result<WindowsRuntimePreparationResult, String> {
    let install_dir = windows_runtime_install_dir()?;
    let package_path = install_dir.join("win.zip");
    let partial_path = install_dir.join("win.zip.part");
    let metadata_path = install_dir.join("win.zip.part.json");

    let result = (|| {
        emit_windows_runtime_progress(
            &app,
            &download_id,
            2,
            "正在检查运行环境...",
            "check",
            0,
            None,
            0,
        );

        if composer_state_is_available(&composer)? {
            emit_windows_runtime_progress(
                &app,
                &download_id,
                100,
                "运行环境已就绪",
                "complete",
                0,
                None,
                0,
            );
            return Ok(WindowsRuntimePreparationResult {
                ready: true,
                downloaded: false,
            });
        }

        if missing_windows_runtime_dlls(&install_dir).is_empty()
            && reinitialize_composer(&composer).is_ok()
        {
            emit_windows_runtime_progress(
                &app,
                &download_id,
                100,
                "运行环境已就绪",
                "complete",
                0,
                None,
                0,
            );
            return Ok(WindowsRuntimePreparationResult {
                ready: true,
                downloaded: false,
            });
        }

        if download_url.trim().is_empty() {
            return Err("运行库下载地址为空".to_string());
        }

        download_resumable_to_file(
            &app,
            WINDOWS_RUNTIME_DOWNLOAD_EVENT_NAME,
            "download",
            &download_id,
            download_url.trim(),
            &package_path,
            &partial_path,
            &metadata_path,
            runtime_version.trim(),
            &cancel_flag,
            3,
            85,
            "初次运行需等待，请稍候",
        )?;

        extract_windows_runtime_zip(
            &app,
            &download_id,
            &package_path,
            &install_dir,
            &cancel_flag,
        )?;

        remove_file_if_exists(&package_path)?;
        remove_file_if_exists(&metadata_path)?;

        emit_windows_runtime_progress(
            &app,
            &download_id,
            97,
            "正在校验运行环境...",
            "verify",
            0,
            None,
            0,
        );
        let missing = missing_windows_runtime_dlls(&install_dir);
        if !missing.is_empty() {
            return Err(format!("运行环境校验失败，缺少：{}", missing.join("、")));
        }

        reinitialize_composer(&composer).map_err(|error| format!("运行环境加载失败：{error}"))?;

        emit_windows_runtime_progress(
            &app,
            &download_id,
            100,
            "运行环境准备完成",
            "complete",
            0,
            None,
            0,
        );
        Ok(WindowsRuntimePreparationResult {
            ready: true,
            downloaded: true,
        })
    })();

    let _ = remove_download_task(&download_id);
    result
}

#[tauri::command]
fn is_windows_runtime_ready(composer: State<'_, ComposerState>) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        if composer_state_is_available(composer.inner())? {
            return Ok(true);
        }

        let install_dir = windows_runtime_install_dir()?;
        if !missing_windows_runtime_dlls(&install_dir).is_empty() {
            return Ok(false);
        }

        return Ok(reinitialize_composer(composer.inner()).is_ok());
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = composer;
        Ok(true)
    }
}

#[tauri::command]
async fn prepare_windows_runtime(
    app: AppHandle,
    composer: State<'_, ComposerState>,
    download_url: String,
    runtime_version: String,
    download_id: String,
) -> Result<WindowsRuntimePreparationResult, String> {
    #[cfg(target_os = "windows")]
    {
        let cancel_flag = register_download_task(&download_id)?;
        let composer_state = composer.inner().clone();
        return tauri::async_runtime::spawn_blocking(move || {
            prepare_windows_runtime_blocking(
                app,
                composer_state,
                download_url,
                runtime_version,
                download_id,
                cancel_flag,
            )
        })
        .await
        .map_err(|error| error.to_string())?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, composer, download_url, runtime_version, download_id);
        Ok(WindowsRuntimePreparationResult {
            ready: true,
            downloaded: false,
        })
    }
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
                    TEMPLATE_DOWNLOAD_EVENT_NAME,
                    "assets",
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
fn save_custom_template_xml(
    template_id: String,
    template_xml: String,
    resource_paths: Vec<String>,
    fixed_material_path: String,
    is_pr_imported: bool,
) -> Result<SavedCustomTemplate, String> {
    let template_id = template_id.trim();
    if template_id.is_empty() {
        return Err("templateId 不能为空".to_string());
    }
    if template_xml.trim().is_empty() {
        return Err("模板 XML 不能为空".to_string());
    }

    let sanitized_template_id = sanitize_custom_template_key(template_id);
    if sanitized_template_id.eq_ignore_ascii_case("temp") {
        return Err("templateId 不能为 temp".to_string());
    }
    let custom_storage = ensure_custom_storage_dirs()?;
    let custom_template_dir = custom_storage.project.join(sanitized_template_id);
    fs::create_dir_all(&custom_template_dir).map_err(|error| error.to_string())?;

    let assets_dir = custom_template_dir.join("assets");
    let staging_assets_dir = custom_template_dir.join(".assets.tmp");
    if staging_assets_dir.exists() {
        fs::remove_dir_all(&staging_assets_dir).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&staging_assets_dir).map_err(|error| error.to_string())?;

    let fixed_material_file = PathBuf::from(fixed_material_path.trim())
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut copied_assets = HashMap::<String, PathBuf>::new();
    for resource_path in resource_paths {
        let source_path = PathBuf::from(resource_path.trim());
        if !source_path.is_file() {
            let _ = fs::remove_dir_all(&staging_assets_dir);
            return Err(format!("模板素材不存在：{}", source_path.display()));
        }
        let file_name = source_path
            .file_name()
            .ok_or_else(|| format!("无法读取模板素材文件名：{}", source_path.display()))?;
        let destination_key = file_name.to_string_lossy().to_lowercase();
        let canonical_source =
            fs::canonicalize(&source_path).unwrap_or_else(|_| source_path.clone());
        if let Some(existing_source) = copied_assets.get(&destination_key) {
            if existing_source != &canonical_source {
                let _ = fs::remove_dir_all(&staging_assets_dir);
                return Err(format!(
                    "存在两个同名模板素材，无法保存：{}",
                    file_name.to_string_lossy()
                ));
            }
            continue;
        }

        fs::copy(&source_path, staging_assets_dir.join(file_name)).map_err(|error| {
            let _ = fs::remove_dir_all(&staging_assets_dir);
            format!("模板素材复制失败（{}）：{error}", source_path.display())
        })?;
        copied_assets.insert(destination_key, canonical_source);
    }

    if assets_dir.exists() {
        fs::remove_dir_all(&assets_dir).map_err(|error| format!("旧模板素材清理失败：{error}"))?;
    }
    fs::rename(&staging_assets_dir, &assets_dir)
        .map_err(|error| format!("模板素材目录更新失败：{error}"))?;

    let template_file_path = custom_template_dir.join("template.xml");
    fs::write(&template_file_path, template_xml.as_bytes())
        .map_err(|error| format!("模板 XML 保存失败：{error}"))?;

    let previous_editor_state = custom_template_editor_state(&custom_template_dir);
    let editor_state = CustomTemplateEditorState {
        fixed_material_file,
        is_pr_imported,
        status: Some(previous_editor_state.status.unwrap_or_else(|| {
            if read_custom_template_upload_state(&custom_template_dir)
                .map(|state| state.finalized)
                .unwrap_or(false)
            {
                1
            } else {
                2
            }
        })),
        submission_ready: Some(previous_editor_state.submission_ready.unwrap_or_else(|| {
            !read_custom_template_upload_state(&custom_template_dir)
                .map(|state| state.finalized)
                .unwrap_or(false)
        })),
    };
    write_custom_template_editor_state(&custom_template_dir, &editor_state)?;

    let temp_dir = custom_storage.pr_temp;
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|error| format!("PR 临时素材清理失败：{error}"))?;
    }
    fs::create_dir_all(&temp_dir).map_err(|error| format!("PR 临时目录重建失败：{error}"))?;

    Ok(SavedCustomTemplate {
        template_file_path: path_to_xml_filepath(template_file_path),
        assets_dir: path_to_xml_filepath(assets_dir),
    })
}

fn custom_template_editor_state(template_dir: &Path) -> CustomTemplateEditorState {
    let state_path = template_dir.join(".editor-state.json");
    fs::read(state_path)
        .ok()
        .and_then(|content| serde_json::from_slice(&content).ok())
        .unwrap_or_default()
}

fn write_custom_template_editor_state(
    template_dir: &Path,
    state: &CustomTemplateEditorState,
) -> Result<(), String> {
    let content = serde_json::to_vec_pretty(state)
        .map_err(|error| format!("模板编辑状态序列化失败：{error}"))?;
    fs::write(template_dir.join(".editor-state.json"), content)
        .map_err(|error| format!("模板编辑状态保存失败：{error}"))
}

fn custom_template_status(template_dir: &Path, editor_state: &CustomTemplateEditorState) -> u8 {
    if let Some(status @ 0..=2) = editor_state.status {
        return status;
    }
    if read_custom_template_upload_state(template_dir)
        .map(|state| state.finalized)
        .unwrap_or(false)
    {
        1
    } else {
        2
    }
}

fn custom_template_submission_ready(
    template_dir: &Path,
    editor_state: &CustomTemplateEditorState,
) -> bool {
    editor_state.submission_ready.unwrap_or_else(|| {
        !read_custom_template_upload_state(template_dir)
            .map(|state| state.finalized)
            .unwrap_or(false)
    })
}

#[tauri::command]
fn update_custom_template_status(
    template_id: String,
    status: u8,
    submission_ready: Option<bool>,
) -> Result<(), String> {
    if status > 2 {
        return Err("模板状态无效".to_string());
    }
    let template_dir = custom_template_upload_dir(&template_id)?;
    let mut state = custom_template_editor_state(&template_dir);
    state.status = Some(status);
    if let Some(submission_ready) = submission_ready {
        state.submission_ready = Some(submission_ready);
    }
    write_custom_template_editor_state(&template_dir, &state)
}

fn custom_template_video_details(xml_content: &str) -> (u64, String, String) {
    let video_inner = find_xml_element_blocks(xml_content, "video")
        .into_iter()
        .next()
        .map(|(_, inner)| inner)
        .unwrap_or_default();
    let element_text = |tag_name: &str| {
        find_xml_element_blocks(&video_inner, tag_name)
            .into_iter()
            .next()
            .map(|(_, inner)| unescape_xml_value(inner.trim()))
            .unwrap_or_default()
    };
    (
        element_text("duration").parse::<u64>().unwrap_or(0),
        element_text("resolution"),
        element_text("demo-path"),
    )
}

fn custom_template_name(xml_content: &str, fallback: &str) -> String {
    find_xml_element_blocks(xml_content, "template")
        .into_iter()
        .next()
        .and_then(|(tag, _)| xml_attribute_value(&tag, "name"))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn custom_template_fixed_material_path(
    template_dir: &Path,
    assets_dir: &Path,
    xml_content: &str,
    editor_state: &CustomTemplateEditorState,
) -> String {
    if !editor_state.fixed_material_file.trim().is_empty() {
        let path = assets_dir.join(&editor_state.fixed_material_file);
        if path.is_file() {
            return path_to_xml_filepath(path);
        }
    }

    let tmp_top = assets_dir.join("tmptop.mov");
    if tmp_top.is_file() {
        return path_to_xml_filepath(tmp_top);
    }

    let background_path = find_xml_element_blocks(xml_content, "track")
        .into_iter()
        .find(|(tag, _)| xml_attribute_value(tag, "id").as_deref() == Some("bg"))
        .and_then(|(_, inner)| {
            find_xml_element_blocks(&inner, "filepath")
                .into_iter()
                .next()
                .map(|(_, value)| unescape_xml_value(value.trim()))
        })
        .unwrap_or_default();
    let background_name = Path::new(&background_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if background_name.eq_ignore_ascii_case("top.mov")
        || background_name.eq_ignore_ascii_case("tmptop.mov")
    {
        let resolved = PathBuf::from(resolve_template_resource_filepath(
            template_dir,
            assets_dir,
            &background_path,
        ));
        if resolved.is_file() {
            return path_to_xml_filepath(resolved);
        }
    }

    String::new()
}

#[tauri::command]
fn list_custom_templates() -> Result<Vec<CustomTemplateSummary>, String> {
    let project_root = ensure_custom_storage_dirs()?.project;
    let mut templates = Vec::new();

    for entry in fs::read_dir(&project_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if !file_type.is_dir() {
            continue;
        }
        let template_id = entry.file_name().to_string_lossy().to_string();
        let template_dir = entry.path();
        let template_file_path = template_dir.join("template.xml");
        if !template_file_path.is_file() {
            continue;
        }
        let xml_content = match fs::read_to_string(&template_file_path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let (duration_ms, resolution, demo_path) = custom_template_video_details(&xml_content);
        let assets_dir = template_dir.join("assets");
        let resolved_preview =
            resolve_template_resource_filepath(&template_dir, &assets_dir, &demo_path);
        let preview_path = if Path::new(&resolved_preview).is_file() {
            resolved_preview
        } else {
            String::new()
        };
        let updated_ms = fs::metadata(&template_file_path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let editor_state = custom_template_editor_state(&template_dir);
        let upload_state = read_custom_template_upload_state(&template_dir).unwrap_or_default();

        templates.push(CustomTemplateSummary {
            name: custom_template_name(&xml_content, &template_id),
            template_id,
            backend_template_id: upload_state.backend_template_id,
            status: custom_template_status(&template_dir, &editor_state),
            submission_ready: custom_template_submission_ready(&template_dir, &editor_state),
            duration_ms,
            resolution,
            preview_path,
            updated_at: format_timestamp(updated_ms),
        });
    }

    templates.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(templates)
}

#[tauri::command]
fn read_custom_template(template_id: String) -> Result<CustomTemplateDetail, String> {
    let template_id = template_id.trim();
    if template_id.is_empty() {
        return Err("templateId 不能为空".to_string());
    }
    let sanitized_template_id = sanitize_custom_template_key(template_id);
    if sanitized_template_id.eq_ignore_ascii_case("temp") {
        return Err("不能读取临时模板目录".to_string());
    }
    let template_dir = ensure_custom_storage_dirs()?
        .project
        .join(&sanitized_template_id);
    let template_file_path = template_dir.join("template.xml");
    if !template_file_path.is_file() {
        return Err("我的模板中不存在该模板".to_string());
    }
    let xml_content = fs::read_to_string(&template_file_path)
        .map_err(|error| format!("模板 XML 读取失败：{error}"))?;
    let assets_dir = template_dir.join("assets");
    let editor_state = custom_template_editor_state(&template_dir);
    let fixed_material_path = custom_template_fixed_material_path(
        &template_dir,
        &assets_dir,
        &xml_content,
        &editor_state,
    );

    Ok(CustomTemplateDetail {
        template_id: template_id.to_string(),
        status: custom_template_status(&template_dir, &editor_state),
        template_file_path: path_to_xml_filepath(template_file_path),
        project_root: path_to_xml_filepath(template_dir),
        xml_content,
        fixed_material_path,
        is_pr_imported: editor_state.is_pr_imported || assets_dir.join("tmptop.mov").is_file(),
    })
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
fn clone_project_workspace(
    source_project_dir: String,
    template_id: String,
    new_project_id: String,
    new_project_name: String,
) -> Result<LocalProjectWorkspace, String> {
    let template_id = template_id.trim();
    let new_project_id = new_project_id.trim();
    let new_project_name = new_project_name.trim();
    if template_id.is_empty() || new_project_id.is_empty() || new_project_name.is_empty() {
        return Err("新工程 ID、名称或模板 ID 不能为空".to_string());
    }

    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let source_project_dir =
        fs::canonicalize(source_project_dir).map_err(|error| format!("读取原工程失败: {error}"))?;
    if !source_project_dir.starts_with(&project_root) || !source_project_dir.is_dir() {
        return Err("原工程目录无效".to_string());
    }

    let workspace_id = format!(
        "{}-{}",
        sanitize_name(template_id),
        sanitize_name(new_project_id)
    );
    let target_dir = project_root.join(&workspace_id);
    if target_dir.is_dir() {
        return read_project_workspace(template_id.to_string(), new_project_id.to_string());
    }
    if target_dir.exists() {
        return Err("新工程目录已存在且不是文件夹".to_string());
    }

    let source_template_path = source_project_dir.join("template.xml");
    let source_project_file_path = source_project_dir.join("projectFile.xml");
    if !source_template_path.is_file() || !source_project_file_path.is_file() {
        return Err("原工程缺少 template.xml 或 projectFile.xml".to_string());
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary_dir = project_root.join(format!(
        ".{workspace_id}.copy-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&temporary_dir).map_err(|error| format!("创建临时工程目录失败: {error}"))?;

    let copy_result = (|| -> Result<(), String> {
        let template_xml = fs::read_to_string(&source_template_path)
            .map_err(|error| format!("读取原工程 template.xml 失败: {error}"))?;
        let project_file_xml = fs::read_to_string(&source_project_file_path)
            .map_err(|error| format!("读取原工程 projectFile.xml 失败: {error}"))?;
        let template_xml = clear_project_generatepaths(&template_xml)?;
        let project_file_xml = clear_project_generatepaths(&project_file_xml)?;
        let project_file_xml =
            update_project_root_identity(&project_file_xml, &workspace_id, new_project_name)?;

        fs::write(temporary_dir.join("template.xml"), template_xml)
            .map_err(|error| format!("写入副本 template.xml 失败: {error}"))?;
        fs::write(temporary_dir.join("projectFile.xml"), project_file_xml)
            .map_err(|error| format!("写入副本 projectFile.xml 失败: {error}"))?;

        let source_title_path = source_project_dir.join("title.png");
        if source_title_path.is_file() {
            fs::copy(source_title_path, temporary_dir.join("title.png"))
                .map_err(|error| format!("复制工程封面失败: {error}"))?;
        }

        fs::rename(&temporary_dir, &target_dir)
            .map_err(|error| format!("保存副本工程目录失败: {error}"))?;
        Ok(())
    })();

    if let Err(error) = copy_result {
        let _ = fs::remove_dir_all(&temporary_dir);
        if target_dir.is_dir() {
            return read_project_workspace(template_id.to_string(), new_project_id.to_string());
        }
        return Err(error);
    }

    read_project_workspace(template_id.to_string(), new_project_id.to_string())
}

#[tauri::command]
async fn get_project_asset_fingerprints(
    project_dir: String,
) -> Result<Vec<ProjectAssetFingerprint>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        project_asset_fingerprints(Path::new(&project_dir))
    })
    .await
    .map_err(|error| error.to_string())?
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
fn adapt_project_asset_speed(
    project_dir: String,
    asset_id: String,
    video_duration_ms: u64,
) -> Result<AdaptedProjectAssetSpeed, String> {
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
    let template_path = project_dir.join("template.xml");
    let template_xml = fs::read_to_string(&template_path).map_err(|error| error.to_string())?;
    let (updated_xml, adapted) =
        adapt_template_asset_speeds(&template_xml, asset_id.trim(), video_duration_ms)?;
    if updated_xml != template_xml {
        fs::write(&template_path, updated_xml.as_bytes()).map_err(|error| error.to_string())?;
    }
    Ok(AdaptedProjectAssetSpeed {
        project_xml: updated_xml,
        adapted,
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
    app: AppHandle,
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
    let properties = prepare_project_asset_properties(&app, properties)?;
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
    app: AppHandle,
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
    let properties = prepare_project_asset_properties(&app, properties)?;
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
fn preserve_project_asset_preview_video(
    project_dir: String,
    asset_id: String,
    preview_video_path: String,
) -> Result<PreservedProjectAssetPreview, String> {
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

    let source_path = fs::canonicalize(PathBuf::from(preview_video_path))
        .map_err(|error| format!("预览视频不存在: {error}"))?;
    if !source_path.is_file() {
        return Err("预览视频路径不是文件".to_string());
    }

    let preview_dir = project_dir.join("preview-assets");
    fs::create_dir_all(&preview_dir).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("mp4");
    let target_path = preview_dir.join(format!(
        "{}_{}.{}",
        sanitize_name(asset_id),
        timestamp,
        extension
    ));
    fs::copy(&source_path, &target_path)
        .map_err(|error| format!("保存素材预览视频失败: {error}"))?;

    if let Ok(entries) = fs::read_dir(&preview_dir) {
        let prefix = format!("{}_", sanitize_name(asset_id));
        for entry in entries.flatten() {
            let path = entry.path();
            if path == target_path || !path.is_file() {
                continue;
            }
            let matches_asset = path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|name| name.starts_with(&prefix))
                .unwrap_or(false);
            if matches_asset {
                let _ = fs::remove_file(path);
            }
        }
    }

    Ok(PreservedProjectAssetPreview {
        preview_video_path: target_path.to_string_lossy().to_string(),
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
            r#"{"watermark":false}"#,
            true,
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

#[tauri::command]
async fn preview_project_video(
    app: AppHandle,
    composer: tauri::State<'_, ComposerState>,
    template_path: String,
    project_dir: String,
    preview_id: String,
) -> Result<ComposerExportResult, String> {
    app_log_info(format!(
        "[composer] preview_project_video requested preview_id={preview_id}"
    ));

    let template_path = PathBuf::from(template_path);
    if !template_path.is_file() {
        return Err("模板 XML 文件不存在".to_string());
    }

    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;
    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let project_path = project_dir.join("projectFile.xml");
    if !project_path.is_file() {
        return Err("projectFile.xml 不存在".to_string());
    }

    let preview_dir = project_dir.join("preview");
    fs::create_dir_all(&preview_dir).map_err(|error| format!("创建预览目录失败: {error}"))?;
    let output_path = preview_dir.join("template-preview.mp4");
    if output_path.is_file() {
        fs::remove_file(&output_path).map_err(|error| format!("覆盖旧预览视频失败: {error}"))?;
    }

    let output_path_string = output_path.to_string_lossy().to_string();
    let template_path_string = template_path.to_string_lossy().to_string();
    let project_path_string = project_path.to_string_lossy().to_string();
    let composer = composer.inner().clone();
    let preview_id_for_progress = preview_id.clone();
    let app_for_progress = app.clone();

    emit_composer_progress(&app, &preview_id, 0, "正在准备预览...");

    tauri::async_runtime::spawn_blocking(move || {
        let composer = composer.lock().map_err(|error| error.to_string())?;
        let _wake_guard = ExportWakeGuard::acquire().ok();
        composer.compose_video(
            &template_path_string,
            &project_path_string,
            &output_path_string,
            r#"{"watermark":true}"#,
            false,
            app_for_progress,
            preview_id_for_progress,
        )
    })
    .await
    .map_err(|error| error.to_string())??;

    emit_composer_progress(&app, &preview_id, 100, "预览生成完成");

    Ok(ComposerExportResult {
        output_path: output_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
async fn preview_template_factory_video(
    app: AppHandle,
    composer: tauri::State<'_, ComposerState>,
    template_xml: String,
    preview_id: String,
) -> Result<ComposerExportResult, String> {
    if template_xml.trim().is_empty() {
        return Err("模板 XML 不能为空".to_string());
    }

    let preview_dir = ensure_custom_storage_dirs()?.preview;

    let template_path = preview_dir.join("template.xml");
    let project_path = preview_dir.join("projectFile.xml");
    let output_path = preview_dir.join("template-preview.mp4");
    let title_path = preview_dir.join("title.png");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let project_xml =
        generate_project_file_xml(&template_xml, "template-factory-preview", timestamp)?;

    if output_path.is_file() {
        fs::remove_file(&output_path)
            .map_err(|error| format!("覆盖旧模板预览视频失败: {error}"))?;
    }
    if title_path.is_file() {
        fs::remove_file(&title_path).map_err(|error| format!("清理旧模板封面失败: {error}"))?;
    }
    fs::write(&template_path, template_xml.as_bytes())
        .map_err(|error| format!("保存临时模板 XML 失败: {error}"))?;
    fs::write(&project_path, project_xml.as_bytes())
        .map_err(|error| format!("保存临时工程 XML 失败: {error}"))?;

    let template_path_text = path_to_xml_filepath(template_path);
    let project_path_text = path_to_xml_filepath(project_path);
    let output_path_text = path_to_xml_filepath(output_path.clone());
    let composer = composer.inner().clone();
    let preview_id_for_progress = preview_id.clone();
    let app_for_progress = app.clone();

    emit_composer_progress(&app, &preview_id, 0, "正在准备模板预览...");
    let compose_result = tauri::async_runtime::spawn_blocking(move || {
        let composer = composer.lock().map_err(|error| error.to_string())?;
        let _wake_guard = ExportWakeGuard::acquire().ok();
        composer.compose_video(
            &template_path_text,
            &project_path_text,
            &output_path_text,
            r#"{"watermark":false}"#,
            true,
            app_for_progress,
            preview_id_for_progress,
        )
    })
    .await
    .map_err(|error| error.to_string())?;

    if let Err(error) = compose_result {
        if output_path.is_file() {
            let _ = fs::remove_file(&output_path);
        }
        return Err(error);
    }
    if !output_path.is_file() {
        return Err("模板预览合成成功，但未生成视频文件".to_string());
    }

    emit_composer_progress(&app, &preview_id, 100, "模板预览生成完成");
    Ok(ComposerExportResult {
        output_path: path_to_xml_filepath(output_path),
    })
}

#[tauri::command]
async fn save_custom_template_video(
    template_id: String,
    preview_video_path: String,
    template_xml: String,
) -> Result<CustomTemplateVideoResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let template_id = template_id.trim();
        if template_id.is_empty() {
            return Err("templateId 不能为空".to_string());
        }
        if template_xml.trim().is_empty() {
            return Err("模板 XML 不能为空".to_string());
        }

        let sanitized_template_id = sanitize_custom_template_key(template_id);
        if sanitized_template_id.eq_ignore_ascii_case("temp") {
            return Err("templateId 不能为 temp".to_string());
        }

        let custom_storage = ensure_custom_storage_dirs()?;
        let preview_root = custom_storage.preview;
        let preview_root = fs::canonicalize(&preview_root)
            .map_err(|error| format!("模板预览目录不可用: {error}"))?;
        let preview_video_path = fs::canonicalize(PathBuf::from(preview_video_path))
            .map_err(|error| format!("模板预览视频不存在: {error}"))?;
        if !preview_video_path.starts_with(&preview_root) || !preview_video_path.is_file() {
            return Err("模板预览视频路径无效".to_string());
        }

        let preview_title_path = preview_video_path
            .parent()
            .ok_or_else(|| "模板预览视频缺少父目录".to_string())?
            .join("title.png");
        if !preview_title_path.is_file() {
            return Err("视频生成完成，但未生成 title.png 封面".to_string());
        }

        let custom_template_dir = custom_storage.project.join(sanitized_template_id);
        let template_file_path = custom_template_dir.join("template.xml");
        if !template_file_path.is_file() {
            return Err("我的模板中不存在对应的 template.xml".to_string());
        }
        let assets_dir = custom_template_dir.join("assets");
        fs::create_dir_all(&assets_dir)
            .map_err(|error| format!("创建模板素材目录失败: {error}"))?;

        let output_path = assets_dir.join("template.mp4");
        let cover_path = custom_template_dir.join("cover.png");
        fs::copy(&preview_video_path, &output_path)
            .map_err(|error| format!("替换模板预览视频失败: {error}"))?;
        fs::copy(&preview_title_path, &cover_path)
            .map_err(|error| format!("保存模板封面失败: {error}"))?;
        fs::remove_file(&preview_title_path)
            .map_err(|error| format!("移动模板封面失败: {error}"))?;
        fs::write(&template_file_path, template_xml.as_bytes())
            .map_err(|error| format!("更新模板 XML 失败: {error}"))?;

        Ok(CustomTemplateVideoResult {
            output_path: path_to_xml_filepath(output_path),
            cover_path: path_to_xml_filepath(cover_path),
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn custom_template_upload_dir(local_template_key: &str) -> Result<PathBuf, String> {
    let key = local_template_key.trim();
    if key.is_empty() || sanitize_custom_template_key(key) != key {
        return Err("本地模板目录名无效".to_string());
    }
    let template_dir = ensure_custom_storage_dirs()?.project.join(key);
    if !template_dir.join("template.xml").is_file() {
        return Err("本地模板 XML 不存在，请先生成模板".to_string());
    }
    Ok(template_dir)
}

fn read_custom_template_upload_state(
    template_dir: &Path,
) -> Result<CustomTemplateUploadState, String> {
    let state_path = template_dir.join(".upload-state.json");
    if !state_path.is_file() {
        return Ok(CustomTemplateUploadState::default());
    }
    let content = fs::read(&state_path).map_err(|error| format!("读取上传状态失败：{error}"))?;
    serde_json::from_slice(&content).map_err(|error| format!("上传状态文件无效：{error}"))
}

fn write_custom_template_upload_state(
    template_dir: &Path,
    state: &CustomTemplateUploadState,
) -> Result<(), String> {
    let state_path = template_dir.join(".upload-state.json");
    let staging_path = template_dir.join(".upload-state.json.tmp");
    let bytes = serde_json::to_vec_pretty(state).map_err(|error| error.to_string())?;
    fs::write(&staging_path, bytes).map_err(|error| format!("保存上传状态失败：{error}"))?;
    #[cfg(target_os = "windows")]
    if state_path.is_file() {
        fs::remove_file(&state_path).map_err(|error| format!("替换旧上传状态失败：{error}"))?;
    }
    fs::rename(&staging_path, &state_path).map_err(|error| format!("更新上传状态失败：{error}"))
}

fn add_custom_template_assets_to_zip(
    writer: &mut zip::ZipWriter<fs::File>,
    assets_dir: &Path,
    current_dir: &Path,
) -> Result<(), String> {
    for entry in fs::read_dir(current_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            add_custom_template_assets_to_zip(writer, assets_dir, &path)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(assets_dir)
                .map_err(|error| error.to_string())?;
            if relative == Path::new("cover.png") {
                continue;
            }
            let file_name = relative
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if file_name == "tmptop.mov"
                || (file_name.starts_with("top-segment-") && file_name.ends_with(".mov"))
            {
                continue;
            }
            let archive_name = relative.to_string_lossy().replace('\\', "/");
            writer
                .start_file(
                    archive_name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .map_err(|error| format!("写入素材压缩包失败：{error}"))?;
            let mut source = fs::File::open(&path)
                .map_err(|error| format!("读取模板素材失败（{}）：{error}", path.display()))?;
            io::copy(&mut source, writer)
                .map_err(|error| format!("压缩模板素材失败（{}）：{error}", path.display()))?;
        }
    }
    Ok(())
}

fn build_custom_template_upload_xml(template_xml: &str) -> Result<String, String> {
    if find_xml_element_blocks(template_xml, "clips").is_empty() {
        return Err("模板 XML 中缺少 clips，无法准备上传文件".to_string());
    }

    let mut output = String::with_capacity(template_xml.len());
    let mut cursor = 0;
    let mut search_start = 0;
    while let Some(relative_start) = template_xml[search_start..].find("<clip") {
        let tag_start = search_start + relative_start;
        if !is_xml_name_boundary(template_xml[tag_start + "<clip".len()..].chars().next()) {
            search_start = tag_start + "<clip".len();
            continue;
        }
        let tag_end = template_xml[tag_start..]
            .find('>')
            .map(|offset| tag_start + offset + 1)
            .ok_or_else(|| "模板 XML 中的 clip 起始标签不完整".to_string())?;
        let tag = &template_xml[tag_start..tag_end];
        let self_closing = tag.trim_end().ends_with("/>");
        let close_end = if self_closing {
            tag_end
        } else {
            template_xml[tag_end..]
                .find("</clip>")
                .map(|offset| tag_end + offset + "</clip>".len())
                .ok_or_else(|| "模板 XML 中的 clip 结束标签不完整".to_string())?
        };

        if xml_attribute_value(tag, "material-type")
            .as_deref()
            .map(|value| value.eq_ignore_ascii_case("fixed"))
            .unwrap_or(false)
        {
            let line_start = template_xml[..tag_start]
                .rfind('\n')
                .map(|position| position + 1)
                .unwrap_or(0);
            let remove_from_line_start = line_start >= cursor
                && template_xml[line_start..tag_start]
                    .chars()
                    .all(|character| matches!(character, ' ' | '\t' | '\r'));
            let removal_start = if remove_from_line_start {
                line_start
            } else {
                tag_start
            };
            output.push_str(&template_xml[cursor..removal_start]);
            cursor = close_end;
            while matches!(
                template_xml.as_bytes().get(cursor),
                Some(b' ' | b'\t' | b'\r')
            ) {
                cursor += 1;
            }
            if template_xml.as_bytes().get(cursor) == Some(&b'\n') {
                cursor += 1;
            }
        } else {
            output.push_str(&template_xml[cursor..tag_end]);
            if !self_closing {
                let inner = &template_xml[tag_end..close_end - "</clip>".len()];
                output.push_str(&remove_xml_child_element(inner, "top-video"));
                output.push_str("</clip>");
            }
            cursor = close_end;
        }
        search_start = close_end;
    }
    output.push_str(&template_xml[cursor..]);
    Ok(output)
}

#[tauri::command]
async fn prepare_custom_template_upload(
    local_template_key: String,
) -> Result<CustomTemplateUploadFiles, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let template_dir = custom_template_upload_dir(&local_template_key)?;
        let assets_dir = template_dir.join("assets");
        if !assets_dir.is_dir() {
            return Err("模板素材目录不存在，请先生成模板".to_string());
        }
        let cover_path = template_dir.join("cover.png");
        let legacy_cover_path = assets_dir.join("cover.png");
        if !cover_path.is_file() && legacy_cover_path.is_file() {
            fs::rename(&legacy_cover_path, &cover_path)
                .map_err(|error| format!("迁移旧模板封面失败：{error}"))?;
        }
        if !cover_path.is_file() {
            return Err("模板封面 cover.png 不存在，请先生成模板".to_string());
        }

        let template_xml = fs::read_to_string(template_dir.join("template.xml"))
            .map_err(|error| format!("读取本地模板 XML 失败：{error}"))?;
        let upload_xml = build_custom_template_upload_xml(&template_xml)?;
        let upload_xml_path = template_dir.join(".upload-template.xml");
        fs::write(&upload_xml_path, upload_xml.as_bytes())
            .map_err(|error| format!("保存上传专用模板 XML 失败：{error}"))?;

        let staging_zip = template_dir.join(".assets.zip.tmp");
        let assets_zip = template_dir.join("assets.zip");
        let result = (|| {
            let file = fs::File::create(&staging_zip)
                .map_err(|error| format!("创建素材压缩包失败：{error}"))?;
            let mut writer = zip::ZipWriter::new(file);
            add_custom_template_assets_to_zip(&mut writer, &assets_dir, &assets_dir)?;
            writer
                .finish()
                .map_err(|error| format!("完成素材压缩包失败：{error}"))?;
            if assets_zip.is_file() {
                fs::remove_file(&assets_zip)
                    .map_err(|error| format!("替换旧素材压缩包失败：{error}"))?;
            }
            fs::rename(&staging_zip, &assets_zip)
                .map_err(|error| format!("保存素材压缩包失败：{error}"))?;
            Ok::<(), String>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&staging_zip);
        }
        result?;

        let upload_state = read_custom_template_upload_state(&template_dir)?;
        let file_size = |path: &Path| {
            fs::metadata(path)
                .map(|metadata| metadata.len())
                .map_err(|error| error.to_string())
        };
        Ok(CustomTemplateUploadFiles {
            xml_size: file_size(&upload_xml_path)?,
            cover_size: file_size(&cover_path)?,
            assets_size: file_size(&assets_zip)?,
            backend_template_id: upload_state.backend_template_id,
            finalized: upload_state.finalized,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn save_custom_template_upload_state(
    local_template_key: String,
    backend_template_id: String,
    finalized: bool,
) -> Result<(), String> {
    if backend_template_id.trim().is_empty() {
        return Err("后台模板 ID 不能为空".to_string());
    }
    let template_dir = custom_template_upload_dir(&local_template_key)?;
    write_custom_template_upload_state(
        &template_dir,
        &CustomTemplateUploadState {
            backend_template_id: Some(backend_template_id),
            finalized,
        },
    )?;
    if finalized {
        let mut editor_state = custom_template_editor_state(&template_dir);
        editor_state.status = Some(1);
        editor_state.submission_ready = Some(false);
        write_custom_template_editor_state(&template_dir, &editor_state)?;
    }
    Ok(())
}

#[tauri::command]
fn read_custom_template_upload_chunk(
    local_template_key: String,
    file_type: String,
    chunk_index: u64,
    chunk_size: u64,
) -> Result<tauri::ipc::Response, String> {
    const MAX_CHUNK_SIZE: u64 = 20 * 1024 * 1024;
    if chunk_size == 0 || chunk_size > MAX_CHUNK_SIZE {
        return Err("上传分片大小无效".to_string());
    }
    let template_dir = custom_template_upload_dir(&local_template_key)?;
    let file_name = match file_type.as_str() {
        "xml" => ".upload-template.xml",
        "cover" => "cover.png",
        "assets" => "assets.zip",
        _ => return Err("上传文件类型无效".to_string()),
    };
    let path = template_dir.join(file_name);
    let mut file = fs::File::open(&path)
        .map_err(|error| format!("读取上传文件失败（{file_name}）：{error}"))?;
    let file_size = file.metadata().map_err(|error| error.to_string())?.len();
    let start = chunk_index
        .checked_mul(chunk_size)
        .ok_or_else(|| "上传分片序号无效".to_string())?;
    if start >= file_size {
        return Err("上传分片超出文件范围".to_string());
    }
    let length = (file_size - start).min(chunk_size) as usize;
    file.seek(SeekFrom::Start(start))
        .map_err(|error| format!("定位上传分片失败：{error}"))?;
    let mut bytes = vec![0u8; length];
    file.read_exact(&mut bytes)
        .map_err(|error| format!("读取上传分片失败：{error}"))?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
async fn generate_asset_thumbnail(
    composer: tauri::State<'_, ComposerState>,
    input_video_path: String,
) -> Result<tauri::ipc::Response, String> {
    let input_video_path = fs::canonicalize(PathBuf::from(input_video_path))
        .map_err(|error| format!("输入视频不存在: {error}"))?;
    if !input_video_path.is_file() {
        return Err("输入视频路径不是文件".to_string());
    }

    let temp_dir = std::env::temp_dir().join("aicut").join("asset-thumbnails");
    fs::create_dir_all(&temp_dir).map_err(|error| format!("创建封面临时目录失败: {error}"))?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let counter = ASSET_THUMBNAIL_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let video_name = input_video_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_preview_file_stem)
        .unwrap_or_else(|| "video".to_string());
    let output_image_path = temp_dir.join(format!(
        "{}_{}_{}_{}.png",
        std::process::id(),
        timestamp,
        counter,
        video_name
    ));

    let input_video_path_text = path_to_xml_filepath(input_video_path);
    let output_image_path_text = path_to_xml_filepath(output_image_path.clone());
    let output_image_path_for_task = output_image_path.clone();
    let composer = composer.inner().clone();

    let thumbnail_bytes = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let composer = composer.lock().map_err(|error| error.to_string())?;
            composer.beauty_process_frame(
                &input_video_path_text,
                0,
                &output_image_path_text,
                "{}",
            )?;
            drop(composer);

            if !output_image_path_for_task.is_file() {
                return Err("封面接口执行成功，但未生成临时图片".to_string());
            }
            fs::read(&output_image_path_for_task)
                .map_err(|error| format!("读取临时封面失败: {error}"))
        })();

        let cleanup_result = if output_image_path_for_task.exists() {
            fs::remove_file(&output_image_path_for_task)
                .map_err(|error| format!("删除临时封面失败: {error}"))
        } else {
            Ok(())
        };

        match (result, cleanup_result) {
            (Ok(bytes), Ok(())) => Ok(bytes),
            (Ok(_), Err(error)) => Err(error),
            (Err(error), _) => Err(error),
        }
    })
    .await
    .map_err(|error| error.to_string())??;

    Ok(tauri::ipc::Response::new(thumbnail_bytes))
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

fn resolve_lut_resource_file_path(app: &AppHandle, lut_file: &str) -> Result<String, String> {
    let relative_path = Path::new(lut_file.trim());
    if relative_path.as_os_str().is_empty()
        || relative_path.is_absolute()
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

    Ok(path_to_xml_filepath(lut_path))
}

#[tauri::command]
fn resolve_lut_resource_path(app: AppHandle, lut_file: String) -> Result<String, String> {
    resolve_lut_resource_file_path(&app, &lut_file)
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
    let lut_file = params.lut_file.trim().to_string();
    if lut_file.is_empty() {
        params.lut_file.clear();
        params.lut_intensity = 0.0;
        return Ok(params);
    }
    params.lut_file = resolve_lut_resource_file_path(app, &lut_file)?;
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
            is_windows_runtime_ready,
            prepare_windows_runtime,
            get_cached_template_assets,
            read_original_template_xml,
            save_custom_template_xml,
            list_custom_templates,
            read_custom_template,
            update_custom_template_status,
            prepare_template_assets,
            cancel_template_download,
            ensure_default_output_dir,
            download_help_guide,
            create_project_workspace,
            clone_project_workspace,
            get_project_asset_fingerprints,
            read_project_workspace,
            save_project_asset,
            adapt_project_asset_speed,
            update_project_asset_offset,
            update_project_asset_properties,
            apply_project_asset_generated_video,
            preserve_project_asset_preview_video,
            reset_project_asset_generated_video,
            apply_project_subtitle,
            compose_project_video,
            preview_project_video,
            preview_template_factory_video,
            save_custom_template_video,
            prepare_custom_template_upload,
            save_custom_template_upload_state,
            read_custom_template_upload_chunk,
            generate_asset_thumbnail,
            resolve_lut_resource_path,
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
    fn adapts_each_area_speed_and_restores_original_after_longer_replacement() {
        let xml = r#"<template><clips><clip>
            <area id="a1" asset-id="asset-1"><source><duration>9000</duration></source><transform><speed>1</speed></transform></area>
            <area id="a2" asset-id="asset-1"><source><duration>7000</duration></source><transform><speed>0.8</speed></transform></area>
            <area id="a3" asset-id="asset-2"><source><duration>9000</duration></source><transform><speed>1</speed></transform></area>
        </clip></clips></template>"#;
        let (adapted_xml, adapted) =
            adapt_template_asset_speeds(xml, "asset-1", 8000).expect("adapt speeds");
        assert!(adapted);
        assert!(adapted_xml
            .contains(r#"<speed data-auto-slowdown="true" data-original-speed="1">0.888</speed>"#));
        assert!(adapted_xml.contains(
            r#"id="a2" asset-id="asset-1"><source><duration>7000</duration></source><transform><speed>0.8</speed>"#
        ));
        assert!(adapted_xml.contains(
            r#"id="a3" asset-id="asset-2"><source><duration>9000</duration></source><transform><speed>1</speed>"#
        ));

        let (restored_xml, still_adapted) =
            adapt_template_asset_speeds(&adapted_xml, "asset-1", 10000).expect("restore speeds");
        assert!(!still_adapted);
        assert_eq!(restored_xml, xml);
    }

    #[test]
    fn adapts_multiple_short_areas_with_different_required_durations() {
        let xml = r#"<template><area id="a1" asset-id="asset-1"><source><duration>9000</duration></source><transform><speed>1</speed></transform></area><area id="a2" asset-id="asset-1"><source><duration>10000</duration></source><transform><speed>1</speed></transform></area></template>"#;
        let (updated, adapted) =
            adapt_template_asset_speeds(xml, "asset-1", 8000).expect("adapt speeds");
        assert!(adapted);
        assert!(updated.contains(">0.888</speed>"));
        assert!(updated.contains(">0.800</speed>"));
    }

    #[test]
    fn creates_speed_when_transform_is_missing() {
        let xml = r#"<template><area id="a1" asset-id="asset-1"><source><duration>9000</duration></source><destination><width>1920</width></destination></area></template>"#;
        let (updated, adapted) =
            adapt_template_asset_speeds(xml, "asset-1", 8000).expect("adapt speed");
        assert!(adapted);
        assert!(updated.contains(
            r#"<transform><speed data-auto-slowdown="true" data-original-speed="1">0.888</speed></transform>"#
        ));
        assert!(updated.find("<transform>") < updated.find("<destination>"));
    }

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
        assert_eq!(value["lut_file"], "");
        assert!(value.get("rotation_direction").is_none());
    }

    #[test]
    fn serializes_beauty_video_transition_params() {
        let params = ComposerBeautyFrameParams {
            clip_start_time: Some(serde_json::json!(3200)),
            clip_duration: Some(serde_json::json!(1320)),
            start_filter_effect: Some("Fade".to_string()),
            start_filter_duration: Some(serde_json::json!(1000)),
            end_filter_effect: Some("Slide-Left".to_string()),
            end_filter_duration: Some(serde_json::json!(500)),
            ..Default::default()
        };
        let value = serde_json::to_value(params).expect("serialize transition params");
        assert_eq!(value["clipStartTime"], 3200);
        assert_eq!(value["clipDuration"], 1320);
        assert_eq!(value["startFilterEffect"], "Fade");
        assert_eq!(value["startFilterDuration"], 1000);
        assert_eq!(value["endFilterEffect"], "Slide-Left");
        assert_eq!(value["endFilterDuration"], 500);
        assert!(value.get("filterEffect").is_none());
        assert!(value.get("filterDuration").is_none());

        let no_transition = ComposerBeautyFrameParams {
            clip_start_time: Some(serde_json::json!(0)),
            clip_duration: Some(serde_json::json!("")),
            start_filter_effect: Some(String::new()),
            start_filter_duration: Some(serde_json::json!(0)),
            end_filter_effect: Some(String::new()),
            end_filter_duration: Some(serde_json::json!(0)),
            ..Default::default()
        };
        let value = serde_json::to_value(no_transition).expect("serialize empty transition params");
        assert_eq!(value["clipStartTime"], 0);
        assert_eq!(value["clipDuration"], "");
        assert_eq!(value["startFilterEffect"], "");
        assert_eq!(value["startFilterDuration"], 0);
        assert_eq!(value["endFilterEffect"], "");
        assert_eq!(value["endFilterDuration"], 0);
    }

    #[test]
    fn hashes_file_content_as_first_32_sha256_hex_characters() {
        let mut input = io::Cursor::new(b"abc");
        assert_eq!(
            sha256_fingerprint(&mut input).expect("hash content"),
            "ba7816bf8f01cfea414140de5dae2223"
        );
    }

    #[test]
    fn lists_each_asset_even_when_file_fingerprints_are_shared_or_missing() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("timestamp")
            .as_nanos();
        let template_dir = std::env::temp_dir().join(format!(
            "aicut-fingerprint-test-{}-{unique}",
            std::process::id()
        ));
        let assets_dir = template_dir.join("assets");
        fs::create_dir_all(&assets_dir).expect("create test assets");
        fs::write(assets_dir.join("sample.mp4"), b"abc").expect("write test asset");
        fs::write(assets_dir.join("second.mp4"), b"xyz").expect("write second test asset");
        let xml = r#"<template><default-asset>
            <asset id="first" filepath="template/assets/sample.mp4" />
            <asset id="unique" filepath="template/assets/second.mp4" />
            <asset id="second" filepath="template/assets/sample.mp4" />
            <asset id="missing" filepath="template/assets/missing.mp4" />
        </default-asset></template>"#;

        let fingerprints = asset_fingerprints_from_xml(xml, &template_dir, &assets_dir);
        assert_eq!(fingerprints.len(), 4);
        assert_eq!(fingerprints[0].material_key, "first");
        assert_eq!(fingerprints[1].material_key, "unique");
        assert_eq!(fingerprints[2].material_key, "second");
        assert_eq!(fingerprints[0].fingerprint, fingerprints[2].fingerprint);
        assert_eq!(
            fingerprints[0].fingerprint,
            "ba7816bf8f01cfea414140de5dae2223"
        );
        let expected_unique =
            sha256_fingerprint(&mut io::Cursor::new(b"xyz")).expect("hash unique content");
        assert_eq!(fingerprints[1].fingerprint, expected_unique);
        assert_eq!(fingerprints[3].material_key, "missing");
        assert_eq!(fingerprints[3].fingerprint, "");
        fs::remove_dir_all(&template_dir).expect("remove test assets");
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
            lut_style: "/resources/luts/自然清新质感.cube".to_string(),
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
        assert_eq!(
            updated
                .matches("<lut_style>/resources/luts/自然清新质感.cube</lut_style>")
                .count(),
            2
        );
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
            lut_style: "/resources/luts/自然清新质感.cube".to_string(),
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
    fn clears_all_project_generatepaths_without_removing_other_properties() {
        let xml = r#"<xmeml>
    <asset id="asset-a" filepath="/assets/a.mp4" generatepath="/generated/a.mp4" />
    <area asset-id="asset-a">
        <property>
            <whiteness>0.5</whiteness>
            <generatepath>/generated/a.mp4</generatepath>
        </property>
    </area>
</xmeml>"#;

        let cleared = clear_project_generatepaths(xml).expect("clear generate paths");

        assert!(!cleared.contains("generatepath"));
        assert!(cleared.contains("filepath=\"/assets/a.mp4\""));
        assert!(cleared.contains("<whiteness>0.5</whiteness>"));
    }

    #[test]
    fn updates_copied_project_root_identity() {
        let xml = r#"<xmeml><project id="26-638" name="原工程"><meta /></project></xmeml>"#;
        let updated = update_project_root_identity(xml, "26-639", "旅拍丽江假期_副本")
            .expect("update project identity");

        assert!(updated.contains(r#"<project id="26-639" name="旅拍丽江假期_副本">"#));
        assert!(!updated.contains("26-638"));
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
    fn copies_template_area_properties_into_project_file() {
        let template_xml = r#"<xmeml version="5">
    <template id="template-a" name="Template A" version="1.0" timeunit="millisecond">
        <media-asset id="group-a">
            <default-asset>
                <asset id="asset-a" filepath="/videos/a.mov" />
            </default-asset>
        </media-asset>
        <clips id="clips-a" target-track="clips">
            <clip id="clip-a">
                <area id="area-a" asset-id="asset-a">
                    <source><duration>5000</duration></source>
                    <property>
                        <whiteness>0.5</whiteness>
                        <smoothing>0.25</smoothing>
                        <saturation>122</saturation>
                        <lut_style>/luts/style.cube</lut_style>
                        <lut_intensity>0.5</lut_intensity>
                    </property>
                </area>
            </clip>
        </clips>
    </template>
</xmeml>"#;

        let project_xml =
            generate_project_file_xml(template_xml, "project-a", 1000).expect("project xml");

        assert!(project_xml.contains(r#"<area id="area-a" asset-id="asset-a" offset="0">"#));
        assert!(project_xml.contains("<whiteness>0.5</whiteness>"));
        assert!(project_xml.contains("<smoothing>0.25</smoothing>"));
        assert!(project_xml.contains("<saturation>122</saturation>"));
        assert!(project_xml.contains("<lut_style>/luts/style.cube</lut_style>"));
        assert!(project_xml.contains("<lut_intensity>0.5</lut_intensity>"));
        assert!(project_xml.contains("</property>\n                </area>"));
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
    fn organizes_and_migrates_template_factory_storage() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let test_root = std::env::temp_dir().join(format!(
            "aicut-custom-storage-test-{}-{unique}",
            std::process::id()
        ));
        let legacy_template_dir = test_root.join("custom").join("template-123");
        let legacy_pr_temp_dir = test_root.join("custom").join("temp");
        let legacy_preview_dir = test_root.join("preview").join("template-factory");
        fs::create_dir_all(&legacy_template_dir).expect("create legacy template directory");
        fs::create_dir_all(&legacy_pr_temp_dir).expect("create legacy PR temp directory");
        fs::create_dir_all(&legacy_preview_dir).expect("create legacy preview directory");
        fs::write(legacy_template_dir.join("template.xml"), b"<template />")
            .expect("write legacy template");
        fs::write(legacy_pr_temp_dir.join("top.mov"), b"video").expect("write legacy PR asset");
        fs::write(legacy_preview_dir.join("template-preview.mp4"), b"preview")
            .expect("write legacy preview");

        let dirs = ensure_custom_storage_dirs_at(&test_root).expect("organize custom storage");

        assert_eq!(dirs.project, test_root.join("custom").join("project"));
        assert_eq!(dirs.preview, test_root.join("custom").join("preview"));
        assert_eq!(dirs.pr_temp, test_root.join("custom").join("prTemp"));
        assert!(dirs
            .project
            .join("template-123")
            .join("template.xml")
            .is_file());
        assert!(dirs.pr_temp.join("top.mov").is_file());
        assert!(dirs.preview.join("template-preview.mp4").is_file());
        assert!(!legacy_template_dir.exists());
        assert!(!legacy_pr_temp_dir.exists());
        assert!(!legacy_preview_dir.exists());

        fs::remove_dir_all(test_root).expect("remove custom storage test directory");
    }

    #[test]
    fn sanitizes_local_template_directory_key() {
        assert_eq!(
            sanitize_custom_template_key("旅行模板_1789459200000"),
            "旅行模板_1789459200000"
        );
        assert_eq!(
            sanitize_custom_template_key("旅行/模板:第一版_1789459200000"),
            "旅行_模板_第一版_1789459200000"
        );
    }

    #[test]
    fn keeps_local_custom_template_status_and_submit_eligibility() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let template_dir = std::env::temp_dir().join(format!(
            "aicut-custom-status-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&template_dir).expect("create template directory");

        let state = CustomTemplateEditorState {
            status: Some(2),
            submission_ready: Some(false),
            ..Default::default()
        };
        write_custom_template_editor_state(&template_dir, &state).expect("save editor state");
        let loaded = custom_template_editor_state(&template_dir);
        assert_eq!(custom_template_status(&template_dir, &loaded), 2);
        assert!(!custom_template_submission_ready(&template_dir, &loaded));

        fs::remove_dir_all(template_dir).expect("remove test directory");
    }

    #[test]
    fn writes_upload_zip_without_cover_or_pr_top_preview_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let test_dir = std::env::temp_dir().join(format!(
            "aicut-template-upload-test-{}-{unique}",
            std::process::id()
        ));
        let assets_dir = test_dir.join("assets");
        fs::create_dir_all(&assets_dir).expect("create assets directory");
        fs::write(assets_dir.join("template.mp4"), b"video").expect("write video");
        fs::write(assets_dir.join("top.mov"), b"overlay").expect("write overlay");
        fs::write(assets_dir.join("cover.png"), b"old cover").expect("write old cover");
        fs::write(assets_dir.join("tmptop.mov"), b"PR fixed layer").expect("write temporary top");
        fs::write(assets_dir.join("top-segment-001.mov"), b"top slice").expect("write top segment");
        fs::write(assets_dir.join("top-segment-002.MOV"), b"top slice")
            .expect("write uppercase top segment");
        let zip_path = test_dir.join("assets.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&zip_path).expect("create zip"));
        add_custom_template_assets_to_zip(&mut writer, &assets_dir, &assets_dir)
            .expect("add assets to zip");
        writer.finish().expect("finish zip");

        let mut archive =
            zip::ZipArchive::new(fs::File::open(&zip_path).expect("open zip")).expect("read zip");
        assert_eq!(archive.len(), 2);
        assert!(archive.by_name("template.mp4").is_ok());
        assert!(archive.by_name("top.mov").is_ok());
        assert!(archive.by_name("assets/template.mp4").is_err());
        assert!(archive.by_name("cover.png").is_err());
        assert!(archive.by_name("tmptop.mov").is_err());
        assert!(archive.by_name("top-segment-001.mov").is_err());
        assert!(archive.by_name("top-segment-002.MOV").is_err());

        fs::remove_dir_all(test_dir).expect("remove upload test directory");
    }

    #[test]
    fn filters_fixed_clips_and_variable_top_videos_from_upload_xml() {
        let local_xml = r#"<xmeml><template>
        <tracks>
            <track id="overlay"><filepath>template/assets/top.mov</filepath></track>
            <track id="recording">
                <clip starttime="0" endtime="1000"><narration></narration><prompt></prompt><filepath>template/assets/recording-001.wav</filepath></clip>
                <clip starttime="2000" endtime="3000"><narration></narration><prompt></prompt><filepath>template/assets/recording-002.wav</filepath></clip>
            </track>
        </tracks>
        <clips id="clips" target-track="clips">
            <clip id="fixed-1" material-type="fixed">
                <top-video>template/assets/top-segment-001.mov</top-video>
                <subtitle id="fixed-subtitle"><default>本地字幕</default></subtitle>
            </clip>
            <clip id="variable-1" material-type="variable">
                <top-video>template/assets/top-segment-003.mov</top-video>
                <area id="area-1" asset-id="asset-1"><property><saturation>100</saturation></property></area>
            </clip>
            <clip id="fixed-2" material-type="fixed"><top-video>template/assets/top-segment-002.mov</top-video></clip>
            <clip id="variable-2" material-type="variable"><area id="area-2" asset-id="asset-2" /></clip>
        </clips>
    </template></xmeml>"#;

        let upload_xml = build_custom_template_upload_xml(local_xml).expect("build upload XML");

        assert!(!upload_xml.contains("fixed-1"));
        assert!(!upload_xml.contains("fixed-2"));
        assert!(!upload_xml.contains("fixed-subtitle"));
        assert!(!upload_xml.contains("<top-video>"));
        assert!(upload_xml.contains("variable-1"));
        assert!(upload_xml.contains("variable-2"));
        assert!(upload_xml.contains("<saturation>100</saturation>"));
        assert!(upload_xml.contains("template/assets/top.mov"));
        assert!(upload_xml.contains("template/assets/recording-001.wav"));
        assert!(upload_xml.contains("template/assets/recording-002.wav"));
        assert!(upload_xml.contains("<clip starttime=\"2000\" endtime=\"3000\">"));
        assert!(local_xml.contains("fixed-1"));
        assert!(local_xml.contains("top-segment-003.mov"));
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
