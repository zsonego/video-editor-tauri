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
use tauri::{utils::config::Color, AppHandle, Emitter, Manager};

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
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_background_color(Some(Color(7, 18, 42, 255)));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_cached_template_assets,
            prepare_template_assets,
            cancel_template_download,
            create_project_workspace,
            save_project_asset,
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
        let template_xml = include_str!("../../template.xml");
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
        assert!(!project_xml.contains("<clips"));
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
}
