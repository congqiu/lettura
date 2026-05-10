use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::api_types::{
    PageListResponse, PageResponse, PageShareResponse, UploadResponse,
};
use crate::cli::{
    PagesCmd, PagesDeleteArgs, PagesListArgs, PagesPublishArgs, PagesRestoreArgs,
    PagesShareArgs, PagesUpdateArgs, OutputFormat,
};
use crate::client::ApiClient;
use crate::error::CliError;
use crate::output::{emit_json, emit_ids, info};

const MAX_HTML_SIZE: u64 = 10 * 1024 * 1024; // 10 MB
const MAX_ZIP_SIZE: u64 = 50 * 1024 * 1024; // 50 MB

pub async fn run(
    cmd: &PagesCmd,
    client: &ApiClient,
    output: OutputFormat,
    pretty: bool,
) -> Result<i32, CliError> {
    match cmd {
        PagesCmd::Publish(args) => publish(client, args, output, pretty).await,
        PagesCmd::List(args) => list(client, args, output, pretty).await,
        PagesCmd::Update(args) => update(client, args, output, pretty).await,
        PagesCmd::Delete(args) => delete(client, args, output).await,
        PagesCmd::Restore(args) => restore(client, args, output).await,
        PagesCmd::Share(args) => share(client, args, output).await,
    }
}

// ---------------------------------------------------------------------------
// publish
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct PublishBody {
    upload_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
}

async fn publish(
    client: &ApiClient,
    args: &PagesPublishArgs,
    output: OutputFormat,
    pretty: bool,
) -> Result<i32, CliError> {
    let file_path = prepare_upload_file(&args.source).await?;

    info(&format!("Uploading {}...", args.source));
    let upload: UploadResponse = client.upload_files(&file_path).await?;

    // Clean up temp file if we created one (not a plain existing file)
    if !Path::new(&args.source).is_file() {
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    let title = args.title.clone().or(upload.suggested_title.clone());

    let body = PublishBody {
        upload_id: upload.upload_id,
        entry_file: args.entry_file.clone(),
        title,
        description: args.description.clone(),
        password: args.password.clone(),
        expires_at: args.expires_at.clone(),
    };

    let page: PageResponse = client.post("/api/v1/pages", &body).await?;

    match output {
        OutputFormat::Json => {
            emit_json(&page, pretty).map_err(CliError::from)?;
        }
        OutputFormat::Ids => {
            emit_ids([&page.id]).map_err(CliError::from)?;
        }
        OutputFormat::Human => {
            info(&format!("Published: {} ({})", page.title, page.url));
        }
    }

    Ok(0)
}

// ---------------------------------------------------------------------------
// prepare_upload_file
// ---------------------------------------------------------------------------

/// Resolves a user-provided source string to a file path ready for upload.
/// - HTTP/HTTPS URL  → fetched into a temp file
/// - Directory path → zipped into a temp file
/// - File path      → returned as-is
async fn prepare_upload_file(source: &str) -> Result<PathBuf, CliError> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let path = fetch_url_to_temp_file(source).await?;
        Ok(path)
    } else if Path::new(source).is_dir() {
        let source_owned = source.to_owned();
        let path = tokio::task::spawn_blocking(move || zip_directory(&source_owned))
            .await
            .map_err(|e| CliError::ServerError(format!("zip task failed: {e}")))??;
        Ok(path)
    } else if Path::new(source).is_file() {
        Ok(PathBuf::from(source))
    } else {
        Err(CliError::BadArgs(format!(
            "Path does not exist: {}",
            source
        )))
    }
}

/// Downloads a public HTML URL into a temporary file.
async fn fetch_url_to_temp_file(url: &str) -> Result<PathBuf, CliError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| CliError::Network(e.to_string()))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e: reqwest::Error| CliError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(CliError::BadArgs(format!(
            "URL returned HTTP {}, expected 2xx",
            status.as_u16()
        )));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v: &reqwest::header::HeaderValue| v.to_str().ok())
        .unwrap_or("");

    if !content_type.contains("text/html") {
        return Err(CliError::BadArgs(format!(
            "URL content-type is '{}', expected text/html",
            content_type
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e: reqwest::Error| CliError::Network(e.to_string()))?;

    if bytes.len() as u64 > MAX_HTML_SIZE {
        return Err(CliError::UploadFailed(format!(
            "HTML file exceeds {} MB limit",
            MAX_HTML_SIZE / 1024 / 1024
        )));
    }

    let mut tmp = std::env::temp_dir();
    tmp.push(format!("{}.html", Uuid::new_v4()));

    tokio::fs::write(&tmp, &bytes).await?;
    Ok(tmp)
}

/// Creates a ZIP archive of a directory and returns the path to the temp ZIP file.
fn zip_directory(dir: &str) -> Result<PathBuf, CliError> {
    let mut tmp = std::env::temp_dir();
    tmp.push(format!("{}.zip", Uuid::new_v4()));

    let file = std::fs::File::create(&tmp)
        .map_err(|e| CliError::UploadFailed(format!("Failed to create temp ZIP: {}", e)))?;

    let mut zip = ZipWriter::new(file);
    let options =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let dir_path = Path::new(dir);
    let mut found_file = false;

    walk_dir_recursive(dir_path, dir_path, &mut zip, &options, &mut found_file)?;

    zip.finish()
        .map_err(|e| CliError::UploadFailed(format!("ZIP finish error: {}", e)))?;

    if !found_file {
        return Err(CliError::BadArgs(
            "Cannot publish an empty directory".into(),
        ));
    }

    let metadata = std::fs::metadata(&tmp)
        .map_err(|e| CliError::UploadFailed(format!("Cannot stat ZIP: {}", e)))?;
    if metadata.len() > MAX_ZIP_SIZE {
        return Err(CliError::UploadFailed(format!(
            "ZIP file exceeds {} MB limit",
            MAX_ZIP_SIZE / 1024 / 1024
        )));
    }

    Ok(tmp)
}

fn walk_dir_recursive(
    root: &Path,
    current: &Path,
    zip: &mut ZipWriter<std::fs::File>,
    options: &SimpleFileOptions,
    found_file: &mut bool,
) -> Result<(), CliError> {
    let entries = std::fs::read_dir(current).map_err(|e| {
        CliError::BadArgs(format!("Cannot read directory '{}': {}", current.display(), e))
    })?;

    for entry in entries.flatten() {
        let path = entry.path();

        // Exclude hidden files and __MACOSX
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.') || n == "__MACOSX")
            .unwrap_or(false)
        {
            continue;
        }

        if path.is_dir() {
            walk_dir_recursive(root, &path, zip, options, found_file)?;
        } else {
            *found_file = true;
            let relative = path.strip_prefix(root).unwrap_or(&path);
            let name = relative.to_string_lossy().replace('\\', "/");

            zip.start_file(&name, *options)
                .map_err(|e| CliError::UploadFailed(format!("ZIP start_file error: {}", e)))?;

            let data = std::fs::read(&path)
                .map_err(|e| CliError::UploadFailed(format!("Cannot read '{}': {}", path.display(), e)))?;

            zip.write_all(&data)
                .map_err(|e| CliError::UploadFailed(format!("ZIP write error: {}", e)))?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

async fn list(
    client: &ApiClient,
    args: &PagesListArgs,
    output: OutputFormat,
    pretty: bool,
) -> Result<i32, CliError> {
    let query: Vec<(&str, String)> = vec![
        ("status", args.status.clone()),
        ("page", args.page.to_string()),
        ("limit", args.limit.to_string()),
    ];

    let resp: PageListResponse = client.get("/api/v1/pages", &query).await?;

    match output {
        OutputFormat::Json => {
            emit_json(&resp, pretty).map_err(CliError::from)?;
        }
        OutputFormat::Ids => {
            emit_ids(resp.items.iter().map(|p| p.id.clone())).map_err(CliError::from)?;
        }
        OutputFormat::Human => {
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            for p in &resp.items {
                let status = p.status.as_deref().unwrap_or("-");
                writeln!(
                    lock,
                    "{}\t{}\t{}\t{}\t{}",
                    p.slug,
                    p.title,
                    status,
                    p.url,
                    p.created_at
                )?;
            }
        }
    }

    Ok(0)
}

// ---------------------------------------------------------------------------
// update
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, Default)]
#[serde(default)]
struct UpdateBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    /// null means "clear the password"
    #[serde(skip_serializing_if = "Option::is_none")]
    clear_password: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    upload_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_file: Option<String>,
}

async fn update(
    client: &ApiClient,
    args: &PagesUpdateArgs,
    output: OutputFormat,
    pretty: bool,
) -> Result<i32, CliError> {
    if args.password.is_some() && args.clear_password {
        return Err(CliError::BadArgs(
            "Cannot use both --password and --clear-password at the same time".into(),
        ));
    }

    let mut body = UpdateBody::default();
    body.title = args.title.clone();
    body.description = args.description.clone();
    body.status = args.status.clone();
    body.expires_at = args.expires_at.clone();
    body.entry_file = args.entry_file.clone();

    if args.clear_password {
        body.clear_password = Some(true);
        body.password = None;
    } else {
        body.password = args.password.clone();
    }

    // Upload files if --files is provided
    if let Some(ref files_src) = args.files {
        let file_path = prepare_upload_file(files_src).await?;
        info(&format!("Uploading {}...", files_src));
        let upload: UploadResponse = client.upload_files(&file_path).await?;

        // Clean up temp file if we created one
        if !Path::new(files_src).is_file() {
            let _ = tokio::fs::remove_file(&file_path).await;
        }

        body.upload_id = Some(upload.upload_id);
    }

    let path = format!("/api/v1/pages/{}", args.id);
    let page: PageResponse = client.http_patch(&path, &body).await?;

    match output {
        OutputFormat::Json => {
            emit_json(&page, pretty).map_err(CliError::from)?;
        }
        OutputFormat::Ids => {
            emit_ids([&page.id]).map_err(CliError::from)?;
        }
        OutputFormat::Human => {
            info(&format!("Updated: {} ({})", page.title, page.url));
        }
    }

    Ok(0)
}

// ---------------------------------------------------------------------------
// delete
// ---------------------------------------------------------------------------

async fn delete(
    client: &ApiClient,
    args: &PagesDeleteArgs,
    output: OutputFormat,
) -> Result<i32, CliError> {
    let path = format!("/api/v1/pages/{}", args.id);
    #[derive(serde::Deserialize, Default)]
    struct Empty {}
    let _: Empty = client.delete(&path).await?;

    if matches!(output, OutputFormat::Human) {
        info(&format!("Deleted page: {}", args.id));
    }

    Ok(0)
}

// ---------------------------------------------------------------------------
// restore
// ---------------------------------------------------------------------------

async fn restore(
    client: &ApiClient,
    args: &PagesRestoreArgs,
    output: OutputFormat,
) -> Result<i32, CliError> {
    let path = format!("/api/v1/pages/{}/restore", args.id);
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct R {
        id: String,
    }
    let _: R = client.post(&path, &()).await?;

    if matches!(output, OutputFormat::Human) {
        info(&format!("Restored page: {}", args.id));
    }

    Ok(0)
}

// ---------------------------------------------------------------------------
// share
// ---------------------------------------------------------------------------

async fn share(
    client: &ApiClient,
    args: &PagesShareArgs,
    output: OutputFormat,
) -> Result<i32, CliError> {
    let path = format!("/api/v1/pages/{}/share-url", args.id);
    let resp: PageShareResponse = client.get(&path, &[]).await?;

    match output {
        OutputFormat::Json => {
            emit_json(&resp, false).map_err(CliError::from)?;
        }
        OutputFormat::Ids | OutputFormat::Human => {
            info(&resp.url);
        }
    }

    Ok(0)
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prepare_upload_file_nonexistent() {
        let result = prepare_upload_file("/this/path/does/not/exist").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CliError::BadArgs(_)));
        assert!(
            format!("{}", err).contains("does not exist"),
            "error message should mention 'does not exist'"
        );
    }

    #[tokio::test]
    async fn test_prepare_upload_file_existing_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_string_lossy().to_string();
        let result = prepare_upload_file(&path).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from(&path));
    }

    #[test]
    fn test_zip_directory_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = zip_directory(tmp.path().to_str().unwrap());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CliError::BadArgs(ref s) if s.contains("empty")));
    }

    #[test]
    fn test_zip_directory_with_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("index.html"), "<h1>Hello</h1>").unwrap();
        std::fs::write(tmp.path().join("style.css"), "body {}").unwrap();

        let result = zip_directory(tmp.path().to_str().unwrap());
        assert!(result.is_ok());
        let zip_path = result.unwrap();

        // Verify ZIP contains two files
        let file = std::fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"index.html".to_string()));
        assert!(names.contains(&"style.css".to_string()));

        let _ = std::fs::remove_file(zip_path);
    }

    #[test]
    fn test_zip_directory_excludes_hidden_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".hidden"), "secret").unwrap();
        std::fs::write(tmp.path().join("visible.html"), "visible").unwrap();

        let result = zip_directory(tmp.path().to_str().unwrap());
        assert!(result.is_ok());
        let zip_path = result.unwrap();

        let file = std::fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(!names.iter().any(|n| n.contains(".hidden")));
        assert!(names.contains(&"visible.html".to_string()));

        let _ = std::fs::remove_file(zip_path);
    }

    #[test]
    fn test_zip_directory_excludes_macosx() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("__MACOSX")).unwrap();
        std::fs::write(tmp.path().join("__MACOSX").join("metadata"), "mac").unwrap();
        std::fs::write(tmp.path().join("index.html"), "real").unwrap();

        let result = zip_directory(tmp.path().to_str().unwrap());
        assert!(result.is_ok());
        let zip_path = result.unwrap();

        let file = std::fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(!names.iter().any(|n| n.contains("__MACOSX")));
        assert!(names.contains(&"index.html".to_string()));

        let _ = std::fs::remove_file(zip_path);
    }
}