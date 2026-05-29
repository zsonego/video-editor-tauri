use chrono::{DateTime, Local};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    hash::{DefaultHasher, Hasher},
    io,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{utils::config::Color, AppHandle, Emitter, Manager, WindowEvent};

#[cfg(target_os = "macos")]
use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
};

#[cfg(target_os = "macos")]
use libloading::Library;

static DOWNLOAD_CANCEL_FLAGS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectAssetImport {
    copied_path: String,
    project_filepath: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateDownloadProgress {
    download_id: String,
    progress: u8,
    status: String,
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

type ComposerState = Arc<Mutex<ComposerRuntime>>;

struct ComposerRuntime {
    init_error: Option<String>,
    #[cfg(target_os = "macos")]
    _library: Option<Library>,
    #[cfg(target_os = "macos")]
    compose: Option<ComposerComposeFn>,
    #[cfg(target_os = "macos")]
    cleanup: Option<ComposerCleanupFn>,
    #[cfg(target_os = "macos")]
    initialized: bool,
}

#[cfg(target_os = "macos")]
type ComposerInitFn = unsafe extern "C" fn() -> c_int;
#[cfg(target_os = "macos")]
type ComposerCleanupFn = unsafe extern "C" fn();
#[cfg(target_os = "macos")]
type ComposerProgressCallback = extern "C" fn(c_int, *const c_char, *mut c_void);
#[cfg(target_os = "macos")]
type ComposerComposeFn = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    *const c_char,
    Option<ComposerProgressCallback>,
    *mut c_void,
) -> c_int;

#[cfg(target_os = "macos")]
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
    let payload = TemplateDownloadProgress {
        download_id: download_id.to_string(),
        progress: progress.min(100),
        status: status.to_string(),
    };
    let _ = app.emit("template-download-progress", payload);
}

fn emit_composer_progress(app: &AppHandle, export_id: &str, progress: u8, status: &str) {
    println!("[composer] progress export_id={export_id} progress={progress} status={status}");
    let payload = ComposerExportProgress {
        export_id: export_id.to_string(),
        progress: progress.min(100),
        status: status.to_string(),
    };
    let _ = app.emit("composer-export-progress", payload);
}

#[cfg(target_os = "macos")]
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
                eprintln!("[composer] initialization failed but app will continue: {error}");
                Self::disabled(error)
            }
        }
    }

    fn disabled(error: String) -> Self {
        Self {
            init_error: Some(error),
            #[cfg(target_os = "macos")]
            _library: None,
            #[cfg(target_os = "macos")]
            compose: None,
            #[cfg(target_os = "macos")]
            cleanup: None,
            #[cfg(target_os = "macos")]
            initialized: false,
        }
    }

    fn try_initialize() -> Result<Self, String> {
        #[cfg(target_os = "macos")]
        {
            println!("[composer] initializing runtime");
            let library_path = composer_library_path()?;
            println!("[composer] loading dylib: {}", library_path.display());
            let library = unsafe { Library::new(&library_path) }
                .map_err(|error| format!("加载 Composer 动态库失败: {error}"))?;
            println!("[composer] resolving composer_init");
            let init: ComposerInitFn = unsafe {
                *library
                    .get(b"composer_init\0")
                    .map_err(|error| format!("读取 composer_init 失败: {error}"))?
            };
            println!("[composer] resolving composer_compose");
            let compose: ComposerComposeFn = unsafe {
                *library
                    .get(b"composer_compose\0")
                    .map_err(|error| format!("读取 composer_compose 失败: {error}"))?
            };
            println!("[composer] resolving composer_cleanup");
            let cleanup: ComposerCleanupFn = unsafe {
                *library
                    .get(b"composer_cleanup\0")
                    .map_err(|error| format!("读取 composer_cleanup 失败: {error}"))?
            };
            println!("[composer] calling composer_init");
            let init_result = unsafe { init() };

            if init_result != 0 {
                eprintln!(
                    "[composer] composer_init failed: {}",
                    composer_error_message(init_result)
                );
                return Err(composer_error_message(init_result));
            }
            println!("[composer] composer_init success");

            Ok(Self {
                init_error: None,
                _library: Some(library),
                compose: Some(compose),
                cleanup: Some(cleanup),
                initialized: true,
            })
        }

        #[cfg(not(target_os = "macos"))]
        {
            println!("[composer] macOS composer runtime is disabled on this platform");
            Ok(Self {
                init_error: Some("Composer 动态库当前只支持 macOS".to_string()),
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
            eprintln!("[composer] compose skipped because runtime is unavailable: {error}");
            return Err(error.clone());
        }

        #[cfg(target_os = "macos")]
        {
            println!("[composer] compose start export_id={export_id}");
            println!("[composer] template_path={template_path}");
            println!("[composer] project_path={project_path}");
            println!("[composer] output_path={output_path}");
            let Some(compose) = self.compose else {
                let error = "composer_compose 函数未加载".to_string();
                eprintln!("[composer] {error}");
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
                println!("[composer] compose success");
                Ok(())
            } else {
                eprintln!(
                    "[composer] compose failed: {}",
                    composer_error_message(result)
                );
                Err(composer_error_message(result))
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (template_path, project_path, output_path, app, export_id);
            eprintln!("[composer] compose requested on unsupported platform");
            Err("Composer 动态库当前只支持 macOS".to_string())
        }
    }

    fn cleanup(&mut self) {
        #[cfg(target_os = "macos")]
        {
            if self.initialized {
                let Some(cleanup) = self.cleanup else {
                    eprintln!("[composer] composer_cleanup 函数未加载，跳过清理");
                    self.initialized = false;
                    return;
                };
                println!("[composer] calling composer_cleanup");
                unsafe {
                    cleanup();
                }
                self.initialized = false;
                println!("[composer] composer_cleanup complete");
            } else if let Some(error) = &self.init_error {
                eprintln!("[composer] cleanup skipped because runtime is unavailable: {error}");
            }
        }
    }
}

impl Drop for ComposerRuntime {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(target_os = "macos")]
extern "C" fn composer_progress_callback(
    percent: c_int,
    message: *const c_char,
    userdata: *mut c_void,
) {
    if userdata.is_null() {
        eprintln!("[composer] progress callback skipped: userdata is null");
        return;
    }

    let context = unsafe { &*(userdata.cast::<ComposerCallbackContext>()) };
    let status = if message.is_null() {
        "正在合成视频...".to_string()
    } else {
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .trim()
            .to_string()
    };
    let status = if status.is_empty() {
        "正在合成视频...".to_string()
    } else {
        status
    };
    let progress = percent.clamp(0, 100) as u8;

    emit_composer_progress(&context.app, &context.export_id, progress, &status);
}

#[cfg(target_os = "macos")]
fn composer_library_path() -> Result<PathBuf, String> {
    println!("[composer] resolving libcomposer.dylib path");
    let bundled_path = std::env::current_exe().ok().and_then(|exe| {
        exe.parent()
            .and_then(|macos_dir| macos_dir.parent())
            .map(|contents_dir| contents_dir.join("Frameworks").join("libcomposer.dylib"))
    });

    if let Some(path) = bundled_path {
        println!("[composer] checking bundled dylib: {}", path.display());
        if path.is_file() {
            return Ok(path);
        }
    }

    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("libs")
        .join("macos")
        .join("libcomposer.dylib");
    println!("[composer] checking dev dylib: {}", dev_path.display());
    if dev_path.is_file() {
        return Ok(dev_path);
    }

    eprintln!("[composer] libcomposer.dylib not found");
    Err("未找到 libcomposer.dylib".to_string())
}

fn aicut_root_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let base_dir = dirs::data_local_dir();

    #[cfg(target_os = "macos")]
    let base_dir = dirs::data_dir();

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

    fs::create_dir_all(&template_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&project_dir).map_err(|error| error.to_string())?;

    Ok((template_dir, project_dir))
}

fn ensure_aicut_output_dir() -> Result<PathBuf, String> {
    let output_dir = aicut_root_dir()?.join("output");
    println!(
        "[composer] ensuring default output dir: {}",
        output_dir.display()
    );
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
        Some(value[..value_end].to_string())
    } else {
        let value_end = value_start
            .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
            .unwrap_or(value_start.len());
        Some(value_start[..value_end].to_string())
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

fn project_filepath_from_asset_path(project_dir: &Path, asset_path: &Path) -> Option<String> {
    let relative_path = asset_path.strip_prefix(project_dir).ok()?;
    let normalized_path = relative_path.to_string_lossy().replace('\\', "/");

    Some(format!("project/{normalized_path}"))
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

                    Some(TemplateSubtitle { clip_id, id })
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
            output.push_str(&format!(
                "                <subtitle id=\"{}\" text=\"{}\" />\n",
                escape_xml_attribute(&subtitle.id),
                escape_xml_attribute(text)
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
            let id = xml_attribute_value(&clips_tag, "id")?;
            let target_track = xml_attribute_value(&clips_tag, "target-track").unwrap_or_default();
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
    emit_progress(app, download_id, start_progress, status);

    let mut response = reqwest::blocking::get(url).map_err(|error| error.to_string())?;
    let response_status = response.status();

    if !response_status.is_success() {
        return Err(format!("Download failed: {url} ({response_status})"));
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
        emit_progress(
            app,
            download_id,
            progress_between(start_progress, end_progress, downloaded, total),
            status,
        );
    }

    emit_progress(app, download_id, end_progress, status);
    Ok(bytes)
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

    emit_progress(app, download_id, 82, "正在解压素材...");

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
            progress_between(82, 98, (index + 1) as u64, Some(total)),
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
    api_base_url: String,
    download_id: String,
    cancel_flag: Arc<AtomicBool>,
) -> Result<PreparedTemplate, String> {
    let (template_dir, template_file_path, assets_dir) = cached_template_paths(&template_id)?;
    fs::create_dir_all(&template_dir).map_err(|error| error.to_string())?;
    let material_package_path = template_dir.join("materials.zip");

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
            fs::remove_file(&template_file_path).map_err(|error| error.to_string())?;
        }

        let xml_content = if local_xml_version_matches {
            emit_progress(&app, &download_id, 15, "已找到本地模板文件...");
            cached_xml_content.unwrap_or_default()
        } else {
            let template_url = resolve_url(&api_base_url, &template_file_url)?;
            let xml_bytes = download_bytes(
                &app,
                &download_id,
                &template_url,
                &cancel_flag,
                5,
                20,
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
            if material_package_path.exists() {
                fs::remove_file(&material_package_path).map_err(|error| error.to_string())?;
            }

            let package_url = resolve_url(&api_base_url, &material_package_url)?;
            let package_bytes = download_bytes(
                &app,
                &download_id,
                &package_url,
                &cancel_flag,
                25,
                80,
                "正在下载素材包...",
            )?;
            fs::write(&material_package_path, package_bytes).map_err(|error| error.to_string())?;

            extract_zip(
                &app,
                &download_id,
                &material_package_path,
                &assets_dir,
                &cancel_flag,
            )?;
            fs::remove_file(&material_package_path).map_err(|error| error.to_string())?;
            emit_progress(&app, &download_id, 100, "模板资源已准备完成");
        }

        Ok(PreparedTemplate {
            template_dir: template_dir.to_string_lossy().to_string(),
            template_file_path: template_file_path.to_string_lossy().to_string(),
            material_package_path: String::new(),
            assets_dir: assets_dir.to_string_lossy().to_string(),
            xml_content,
        })
    })();

    let cancelled = cancel_flag.load(Ordering::Relaxed);
    let _ = remove_download_task(&download_id);

    if cancelled && result.is_err() {
        let _ = fs::remove_dir_all(&template_dir);
    }

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
async fn prepare_template_assets(
    app: AppHandle,
    template_id: String,
    template_version: String,
    template_file_url: String,
    material_package_url: String,
    api_base_url: String,
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
            api_base_url,
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
fn create_project_workspace(template_id: String) -> Result<ProjectWorkspace, String> {
    let (_, project_root) = ensure_aicut_dirs()?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let project_id = format!("{}-{timestamp}", sanitize_name(&template_id));
    let project_dir = project_root.join(&project_id);

    fs::create_dir_all(&project_dir).map_err(|error| error.to_string())?;
    let (_, template_file_path, _) = cached_template_paths(&template_id)?;
    let template_xml =
        fs::read_to_string(&template_file_path).map_err(|error| error.to_string())?;
    let project_file_xml = generate_project_file_xml(&template_xml, &project_id, timestamp)?;
    fs::write(project_dir.join("projectFile.xml"), project_file_xml)
        .map_err(|error| error.to_string())?;

    Ok(ProjectWorkspace {
        project_dir: project_dir.to_string_lossy().to_string(),
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
    let project_filepath = format!("project/assets/{target_file_name}");
    let project_file_path = project_dir.join("projectFile.xml");
    let project_file_xml =
        fs::read_to_string(&project_file_path).map_err(|error| error.to_string())?;
    let updated_project_file_xml =
        update_project_asset_filepath(&project_file_xml, &asset_id, &project_filepath)?;

    fs::create_dir_all(&assets_dir).map_err(|error| error.to_string())?;
    if !target_path.is_file() {
        fs::copy(&source_path, &target_path).map_err(|error| error.to_string())?;
    }
    fs::write(&project_file_path, updated_project_file_xml).map_err(|error| error.to_string())?;

    Ok(ProjectAssetImport {
        copied_path: target_path.to_string_lossy().to_string(),
        project_filepath,
    })
}

#[tauri::command]
fn update_project_asset_offset(
    project_dir: String,
    asset_id: String,
    offset_ms: u64,
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
        update_project_clip_offsets(&project_file_xml, &asset_id, offset_ms)?;

    fs::write(&project_file_path, updated_project_file_xml).map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
fn apply_project_subtitle(
    project_dir: String,
    template_xml: String,
    text: String,
) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("请输入内容".to_string());
    }

    let subtitle = find_first_template_subtitle(&template_xml)
        .ok_or_else(|| "模板 XML 中未找到 subtitle".to_string())?;
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
    let updated_project_file_xml = update_project_subtitle(&project_file_xml, &subtitle, text)?;

    fs::write(&project_file_path, updated_project_file_xml).map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
async fn compose_project_video(
    app: AppHandle,
    composer: tauri::State<'_, ComposerState>,
    template_path: String,
    project_dir: String,
    output_dir: String,
    export_id: String,
) -> Result<ComposerExportResult, String> {
    println!("[composer] compose_project_video requested export_id={export_id}");
    let template_path = PathBuf::from(template_path);
    println!(
        "[composer] validating template path: {}",
        template_path.display()
    );
    if !template_path.is_file() {
        return Err("模板 XML 文件不存在".to_string());
    }

    let (_, project_root) = ensure_aicut_dirs()?;
    let project_root = fs::canonicalize(project_root).map_err(|error| error.to_string())?;
    let project_dir =
        fs::canonicalize(PathBuf::from(project_dir)).map_err(|error| error.to_string())?;
    println!(
        "[composer] validating project dir: {}",
        project_dir.display()
    );
    if !project_dir.starts_with(&project_root) {
        return Err("项目目录无效".to_string());
    }

    let project_path = project_dir.join("projectFile.xml");
    println!(
        "[composer] validating project xml: {}",
        project_path.display()
    );
    if !project_path.is_file() {
        return Err("projectFile.xml 不存在".to_string());
    }

    let output_dir = PathBuf::from(output_dir);
    println!(
        "[composer] ensuring selected output dir: {}",
        output_dir.display()
    );
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    if !output_dir.is_dir() {
        return Err("输出目录无效".to_string());
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let output_path = output_dir.join(format!("aicut-output-{timestamp}.mp4"));
    println!("[composer] output file: {}", output_path.display());
    let output_path_string = output_path.to_string_lossy().to_string();
    let template_path_string = template_path.to_string_lossy().to_string();
    let project_path_string = project_path.to_string_lossy().to_string();
    let composer = composer.inner().clone();
    let export_id_for_progress = export_id.clone();
    let app_for_progress = app.clone();

    emit_composer_progress(&app, &export_id, 0, "正在准备导出...");

    println!("[composer] spawning blocking compose task");
    tauri::async_runtime::spawn_blocking(move || {
        let composer = composer.lock().map_err(|error| error.to_string())?;
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

    println!("[composer] compose_project_video finished export_id={export_id}");
    emit_composer_progress(&app, &export_id, 100, "导出完成");

    Ok(ComposerExportResult {
        output_path: output_path.to_string_lossy().to_string(),
    })
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
            if project_filepath_from_asset_path(&project_dir, &path)
                .map(|project_filepath| referenced_filepaths.contains(&project_filepath))
                .unwrap_or(false)
            {
                continue;
            }

            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            println!("[app] setup start");
            let composer = ComposerRuntime::initialize();
            app.manage(Arc::new(Mutex::new(composer)));
            println!("[app] composer state managed");

            if let Some(window) = app.get_webview_window("main") {
                println!("[app] configuring main window");
                let _ = window.set_background_color(Some(Color(7, 18, 42, 255)));
            }
            println!("[app] setup complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                println!("[app] close requested, cleaning composer");
                if let Some(composer) = window.try_state::<ComposerState>() {
                    if let Ok(mut composer) = composer.lock() {
                        composer.cleanup();
                    } else {
                        eprintln!("[app] failed to lock composer during close");
                    }
                } else {
                    eprintln!("[app] composer state not found during close");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_cached_template_assets,
            prepare_template_assets,
            cancel_template_download,
            ensure_default_output_dir,
            create_project_workspace,
            save_project_asset,
            update_project_asset_offset,
            apply_project_subtitle,
            compose_project_video,
            delete_project_asset_files,
            get_machine_code
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn applies_first_template_subtitle_to_project_clip() {
        let template_xml = r#"<template>
        <clips id="clips" target-track="clips">
            <clip id="clip-a">
                <subtitle id="subtitle-a" startTime="0" duration="3000">
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

        assert!(updated_xml.contains(r#"<subtitle id="subtitle-a" text="新标题" />"#));
        assert!(!updated_xml.contains("old-a"));
        assert!(!updated_xml.contains("old-b"));
        assert!(!updated_xml.contains("subtitle-b"));
    }
}
