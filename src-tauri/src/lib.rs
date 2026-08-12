use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::Cursor,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;

const DEFAULT_CONFIG: &str = r#"server:
  host: "127.0.0.1"
  port: 8090
  name: "AxonHub"
  request_timeout: "30s"
  llm_request_timeout: "600s"
  debug: false
  cors:
    enabled: true
    allowed_origins:
      - "http://localhost:3000"
      - "http://localhost:5173"
    allowed_methods: ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]
    allowed_headers: ["*"]
    allow_credentials: true

db:
  dialect: "sqlite3"
  dsn: "file:axonhub.db?cache=shared&_fk=1&_pragma=journal_mode(WAL)"

cache:
  mode: "memory"

log:
  name: "axonhub"
  level: "info"
  encoding: "console"
  output: "stdio"
"#;

type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    version: String,
    asset_name: String,
    executable_path: PathBuf,
    sha256: String,
    installed_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum Lifecycle {
    NotInstalled,
    Stopped,
    Starting,
    Running,
    Installing,
    Error,
}

#[derive(Debug, Clone, Serialize)]
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct Snapshot {
    platform: String,
    architecture: String,
    data_directory: String,
    config_path: String,
    installed_version: Option<String>,
    previous_version: Option<String>,
    latest_version: Option<String>,
    update_available: bool,
    lifecycle: Lifecycle,
    healthy: bool,
    pid: Option<u32>,
    port: u16,
    management_url: String,
    uptime: Option<String>,
    busy: bool,
    progress_percent: Option<u8>,
    progress_label: Option<String>,
    last_error: Option<String>,
    logs: Vec<LogEntry>,
    autostart_enabled: bool,
}

struct Runtime {
    installed: Option<Manifest>,
    previous: Option<Manifest>,
    latest_version: Option<String>,
    child: Option<Child>,
    lifecycle: Lifecycle,
    busy: bool,
    progress_percent: Option<u8>,
    progress_label: Option<String>,
    last_error: Option<String>,
    logs: VecDeque<LogEntry>,
}

struct AppState {
    app_handle: AppHandle,
    root: Mutex<PathBuf>,
    settings_path: PathBuf,
    client: reqwest::Client,
    runtime: Mutex<Runtime>,
    shutting_down: AtomicBool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManagerSettings {
    data_directory: PathBuf,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
    uptime: Option<String>,
}

impl AppState {
    fn new(app: &AppHandle) -> Result<Self> {
        let default_root = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        let settings_directory = app
            .path()
            .app_config_dir()
            .map_err(|error| error.to_string())?;
        fs::create_dir_all(&settings_directory).map_err(|error| error.to_string())?;
        let settings_path = settings_directory.join("manager-settings.json");
        let root = fs::read_to_string(&settings_path)
            .ok()
            .and_then(|content| serde_json::from_str::<ManagerSettings>(&content).ok())
            .map(|settings| settings.data_directory)
            .unwrap_or(default_root);
        fs::create_dir_all(root.join("versions")).map_err(|error| error.to_string())?;
        fs::create_dir_all(root.join("logs")).map_err(|error| error.to_string())?;
        let config_path = root.join("config.yml");
        if !config_path.exists() {
            fs::write(&config_path, DEFAULT_CONFIG).map_err(|error| error.to_string())?;
        }
        let installed = load_manifest(&root, "current.json");
        let previous = load_manifest(&root, "previous.json");
        let mut logs = VecDeque::new();
        logs.push_back(LogEntry {
            timestamp: Utc::now().to_rfc3339(),
            level: "info".into(),
            message: "AxonHub Manager 本地控制平面已初始化".into(),
        });
        Ok(Self {
            app_handle: app.clone(),
            root: Mutex::new(root),
            settings_path,
            client: reqwest::Client::builder()
                .user_agent("AxonHub-Manager/0.1")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|error| error.to_string())?,
            runtime: Mutex::new(Runtime {
                lifecycle: if installed.is_some() {
                    Lifecycle::Stopped
                } else {
                    Lifecycle::NotInstalled
                },
                installed,
                previous,
                latest_version: None,
                child: None,
                busy: false,
                progress_percent: None,
                progress_label: None,
                last_error: None,
                logs,
            }),
            shutting_down: AtomicBool::new(false),
        })
    }

    fn config_path(&self) -> PathBuf {
        self.root().join("config.yml")
    }
    fn process_log_path(&self) -> PathBuf {
        self.root().join("logs").join("axonhub.log")
    }
    fn root(&self) -> PathBuf {
        self.root.lock().expect("root mutex poisoned").clone()
    }

    fn log(&self, level: &str, message: impl Into<String>) {
        let mut runtime = self.runtime.lock().expect("runtime mutex poisoned");
        runtime.logs.push_back(LogEntry {
            timestamp: Utc::now().to_rfc3339(),
            level: level.into(),
            message: message.into(),
        });
        while runtime.logs.len() > 250 {
            runtime.logs.pop_front();
        }
    }

    fn set_busy(&self, busy: bool, percent: Option<u8>, label: Option<&str>) {
        let mut runtime = self.runtime.lock().expect("runtime mutex poisoned");
        runtime.busy = busy;
        runtime.progress_percent = percent;
        runtime.progress_label = label.map(str::to_owned);
        if busy {
            runtime.last_error = None;
        }
    }

    fn fail(&self, message: impl Into<String>) -> String {
        let message = message.into();
        let mut runtime = self.runtime.lock().expect("runtime mutex poisoned");
        runtime.busy = false;
        runtime.progress_percent = None;
        runtime.progress_label = None;
        runtime.lifecycle = Lifecycle::Error;
        runtime.last_error = Some(message.clone());
        runtime.logs.push_back(LogEntry {
            timestamp: Utc::now().to_rfc3339(),
            level: "error".into(),
            message: message.clone(),
        });
        message
    }
}

#[tauri::command]
async fn set_autostart(enabled: bool, state: State<'_, AppState>) -> Result<Snapshot> {
    let manager = state.app_handle.autolaunch();
    if enabled {
        manager.enable().map_err(|error| error.to_string())?;
        state.log("info", "已开启开机自启");
    } else {
        manager.disable().map_err(|error| error.to_string())?;
        state.log("info", "已关闭开机自启");
    }
    Ok(snapshot(&state).await)
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(runtime) = self.runtime.get_mut() {
            if let Some(child) = runtime.child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn read_manifest(path: &Path) -> Option<Manifest> {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

fn load_manifest(root: &Path, name: &str) -> Option<Manifest> {
    let path = root.join(name);
    let mut manifest = read_manifest(&path)?;
    if manifest.executable_path.is_file() {
        return Some(manifest);
    }
    let version_directory = root
        .join("versions")
        .join(manifest.version.trim_start_matches('v'));
    if let Some(executable_path) = find_executable(&version_directory) {
        manifest.executable_path = executable_path;
        let _ = write_manifest(&path, &manifest);
    }
    Some(manifest)
}

fn write_manifest(path: &Path, manifest: &Manifest) -> Result<()> {
    let content = serde_json::to_string_pretty(manifest).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

fn copy_directory(source: &Path, target: &Path) -> Result<()> {
    if target.is_file() {
        fs::remove_file(target).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(target).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_directory(&source_path, &target_path)?;
        } else {
            if target_path.is_dir() {
                fs::remove_dir_all(&target_path).map_err(|error| error.to_string())?;
            }
            fs::copy(&source_path, &target_path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn directory_is_non_empty(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    if !path.is_dir() {
        return Err("目标路径不是目录".into());
    }
    Ok(fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .next()
        .is_some())
}

fn prepare_data_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path.join("versions")).map_err(|error| error.to_string())?;
    fs::create_dir_all(path.join("logs")).map_err(|error| error.to_string())?;
    let config_path = path.join("config.yml");
    if !config_path.exists() {
        fs::write(config_path, DEFAULT_CONFIG).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod data_directory_tests {
    use super::{copy_directory, directory_is_non_empty, load_manifest, write_manifest, Manifest};
    use std::{fs, path::PathBuf};

    fn test_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "axonhub-manager-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before Unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn copy_directory_overwrites_conflicts_and_preserves_other_files() {
        let root = test_directory("overwrite");
        let source = root.join("source");
        let target = root.join("target");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::create_dir_all(target.join("nested")).unwrap();
        fs::write(source.join("nested/data.txt"), "new").unwrap();
        fs::write(target.join("nested/data.txt"), "old").unwrap();
        fs::write(target.join("keep.txt"), "keep").unwrap();

        copy_directory(&source, &target).unwrap();

        assert_eq!(
            fs::read_to_string(target.join("nested/data.txt")).unwrap(),
            "new"
        );
        assert_eq!(fs::read_to_string(target.join("keep.txt")).unwrap(), "keep");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn directory_non_empty_check_handles_missing_and_populated_directories() {
        let root = test_directory("non-empty");
        assert!(!directory_is_non_empty(&root).unwrap());
        fs::create_dir_all(&root).unwrap();
        assert!(!directory_is_non_empty(&root).unwrap());
        fs::write(root.join("data.txt"), "data").unwrap();
        assert!(directory_is_non_empty(&root).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_manifest_repairs_an_executable_path_after_the_data_directory_moves() {
        let root = test_directory("repair-manifest");
        let executable = root.join("versions/1.0.0/axonhub.exe");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, "binary").unwrap();
        let manifest_path = root.join("current.json");
        write_manifest(
            &manifest_path,
            &Manifest {
                version: "v1.0.0".into(),
                asset_name: "axonhub.zip".into(),
                executable_path: PathBuf::from("Z:/old-data/versions/1.0.0/axonhub.exe"),
                sha256: "hash".into(),
                installed_at: "now".into(),
            },
        )
        .unwrap();

        let manifest = load_manifest(&root, "current.json").unwrap();

        assert_eq!(manifest.executable_path, executable);
        assert_eq!(
            load_manifest(&root, "current.json")
                .unwrap()
                .executable_path,
            executable
        );
        fs::remove_dir_all(root).unwrap();
    }
}

fn migrated_manifest(manifest: &Manifest, source: &Path, target: &Path) -> Manifest {
    let executable_path = manifest
        .executable_path
        .strip_prefix(source)
        .map(|relative| target.join(relative))
        .unwrap_or_else(|_| manifest.executable_path.clone());
    Manifest {
        executable_path,
        ..manifest.clone()
    }
}

#[tauri::command]
fn is_data_directory_non_empty(directory: String) -> Result<bool> {
    directory_is_non_empty(Path::new(&directory))
}

#[tauri::command]
async fn change_data_directory(
    directory: String,
    migrate: bool,
    state: State<'_, AppState>,
) -> Result<Snapshot> {
    let source = state.root();
    let target = PathBuf::from(directory);
    if target == source {
        return Ok(snapshot(&state).await);
    }
    if target.starts_with(&source) || source.starts_with(&target) {
        return Err("新旧数据目录不能互相包含".into());
    }
    let was_running = state
        .runtime
        .lock()
        .expect("runtime mutex poisoned")
        .child
        .is_some();
    stop_managed_process(&state)?;
    let prepare_result = if migrate {
        copy_directory(&source, &target)
    } else {
        prepare_data_directory(&target)
    };
    if let Err(error) = prepare_result {
        if was_running {
            let _ = start_if_installed(&state).await;
        }
        return Err(format!("切换数据目录失败：{error}"));
    }
    let (installed, previous) = if migrate {
        let runtime = state.runtime.lock().expect("runtime mutex poisoned");
        (
            runtime
                .installed
                .as_ref()
                .map(|item| migrated_manifest(item, &source, &target)),
            runtime
                .previous
                .as_ref()
                .map(|item| migrated_manifest(item, &source, &target)),
        )
    } else {
        (
            load_manifest(&target, "current.json"),
            load_manifest(&target, "previous.json"),
        )
    };
    if let Some(item) = installed.as_ref() {
        write_manifest(&target.join("current.json"), item)?;
    } else if migrate {
        let _ = fs::remove_file(target.join("current.json"));
    }
    if let Some(item) = previous.as_ref() {
        write_manifest(&target.join("previous.json"), item)?;
    } else if migrate {
        let _ = fs::remove_file(target.join("previous.json"));
    }
    let settings = ManagerSettings {
        data_directory: target.clone(),
    };
    let content = serde_json::to_string_pretty(&settings).map_err(|error| error.to_string())?;
    fs::write(&state.settings_path, content).map_err(|error| error.to_string())?;
    {
        let mut runtime = state.runtime.lock().expect("runtime mutex poisoned");
        runtime.installed = installed;
        runtime.previous = previous;
        runtime.lifecycle = if runtime.installed.is_some() {
            Lifecycle::Stopped
        } else {
            Lifecycle::NotInstalled
        };
        runtime.last_error = None;
    }
    *state.root.lock().expect("root mutex poisoned") = target;
    if migrate {
        state.log(
            "info",
            format!("数据目录已迁移，原目录保留：{}", source.display()),
        );
    } else {
        state.log(
            "info",
            format!("数据目录已切换，未迁移原目录：{}", source.display()),
        );
    }
    if was_running {
        start_if_installed(&state).await?;
    }
    Ok(snapshot(&state).await)
}

fn port_from_config(path: &Path) -> u16 {
    let Ok(content) = fs::read_to_string(path) else {
        return 8090;
    };
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
        return 8090;
    };
    value
        .get("server")
        .and_then(|v| v.get("port"))
        .and_then(|v| v.as_u64())
        .and_then(|v| u16::try_from(v).ok())
        .unwrap_or(8090)
}

fn platform_asset_suffix() -> Result<String> {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "darwin",
        "linux" => "linux",
        other => return Err(format!("暂不支持当前系统：{other}")),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => return Err(format!("暂不支持当前架构：{other}")),
    };
    Ok(format!("_{os}_{arch}.zip"))
}

fn find_executable(root: &Path) -> Option<PathBuf> {
    let target = if cfg!(windows) {
        "axonhub.exe"
    } else {
        "axonhub"
    };
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(target)
            {
                return Some(path);
            }
        }
    }
    None
}

fn tail_process_log(path: &Path) -> Vec<LogEntry> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .rev()
        .take(180)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            let level = if lower.contains("error") || lower.contains("fatal") {
                "error"
            } else if lower.contains("warn") {
                "warn"
            } else {
                "info"
            };
            LogEntry {
                timestamp: Utc::now().to_rfc3339(),
                level: level.into(),
                message: line.to_owned(),
            }
        })
        .collect()
}

async fn snapshot(state: &AppState) -> Snapshot {
    let port = port_from_config(&state.config_path());
    let health = state
        .client
        .get(format!("http://127.0.0.1:{port}/health"))
        .timeout(std::time::Duration::from_millis(900))
        .send()
        .await
        .ok()
        .and_then(|response| {
            if response.status().is_success() {
                Some(response)
            } else {
                None
            }
        });
    let health = match health {
        Some(response) => response.json::<HealthResponse>().await.ok(),
        None => None,
    };
    let healthy = health.as_ref().is_some_and(|item| item.status == "healthy");
    let mut runtime = state.runtime.lock().expect("runtime mutex poisoned");
    if let Some(child) = runtime.child.as_mut() {
        if let Ok(Some(status)) = child.try_wait() {
            runtime.child = None;
            runtime.lifecycle = if status.success() {
                Lifecycle::Stopped
            } else {
                Lifecycle::Error
            };
            if !status.success() {
                runtime.last_error = Some(format!("AxonHub 进程已退出：{status}"));
            }
        }
    }
    if runtime.child.is_some() && healthy {
        runtime.lifecycle = Lifecycle::Running;
    }
    let mut logs = runtime.logs.iter().cloned().collect::<Vec<_>>();
    logs.extend(tail_process_log(&state.process_log_path()));
    if logs.len() > 300 {
        logs = logs.split_off(logs.len() - 300);
    }
    let installed_version = runtime.installed.as_ref().map(|item| item.version.clone());
    let latest_version = runtime.latest_version.clone();
    let update_available = matches!((&installed_version, &latest_version), (Some(current), Some(latest)) if current != latest);
    let autostart_enabled = state.app_handle.autolaunch().is_enabled().unwrap_or(false);
    Snapshot {
        platform: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        data_directory: state.root().display().to_string(),
        config_path: state.config_path().display().to_string(),
        installed_version,
        previous_version: runtime.previous.as_ref().map(|item| item.version.clone()),
        latest_version,
        update_available,
        lifecycle: runtime.lifecycle.clone(),
        healthy,
        pid: runtime.child.as_ref().map(Child::id),
        port,
        management_url: format!("http://127.0.0.1:{port}"),
        uptime: health.and_then(|item| item.uptime),
        busy: runtime.busy,
        progress_percent: runtime.progress_percent,
        progress_label: runtime.progress_label.clone(),
        last_error: runtime.last_error.clone(),
        logs,
        autostart_enabled,
    }
}

async fn latest_release(state: &AppState) -> Result<GithubRelease> {
    state
        .client
        .get("https://api.github.com/repos/looplj/axonhub/releases/latest")
        .send()
        .await
        .map_err(|error| format!("无法连接 GitHub：{error}"))?
        .error_for_status()
        .map_err(|error| format!("GitHub 返回错误：{error}"))?
        .json()
        .await
        .map_err(|error| format!("无法解析 Release 信息：{error}"))
}

#[tauri::command]
async fn get_snapshot(state: State<'_, AppState>) -> Result<Snapshot> {
    Ok(snapshot(&state).await)
}

#[tauri::command]
async fn check_updates(state: State<'_, AppState>) -> Result<Snapshot> {
    state.set_busy(true, Some(20), Some("正在查询 GitHub Release"));
    match latest_release(&state).await {
        Ok(release) => {
            let mut runtime = state.runtime.lock().expect("runtime mutex poisoned");
            runtime.latest_version = Some(release.tag_name.clone());
            runtime.busy = false;
            runtime.progress_percent = None;
            runtime.progress_label = None;
            runtime.logs.push_back(LogEntry {
                timestamp: Utc::now().to_rfc3339(),
                level: "info".into(),
                message: format!("已检查上游版本：{}", release.tag_name),
            });
        }
        Err(error) => return Err(state.fail(error)),
    }
    Ok(snapshot(&state).await)
}

#[tauri::command]
async fn install_latest(state: State<'_, AppState>) -> Result<Snapshot> {
    state.set_busy(true, Some(5), Some("读取最新 Release"));
    {
        state
            .runtime
            .lock()
            .expect("runtime mutex poisoned")
            .lifecycle = Lifecycle::Installing;
    }
    let result: Result<()> = async {
        let release = latest_release(&state).await?;
        let suffix = platform_asset_suffix()?;
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name.ends_with(&suffix))
            .cloned()
            .ok_or_else(|| format!("Release 中没有当前平台资产：{suffix}"))?;
        state.set_busy(true, Some(18), Some("正在下载 AxonHub"));
        let bytes = state
            .client
            .get(&asset.browser_download_url)
            .send()
            .await
            .map_err(|error| format!("下载失败：{error}"))?
            .error_for_status()
            .map_err(|error| format!("下载失败：{error}"))?
            .bytes()
            .await
            .map_err(|error| format!("读取下载内容失败：{error}"))?;
        state.set_busy(true, Some(72), Some("校验 SHA-256"));
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if let Some(expected) = asset
            .digest
            .as_deref()
            .and_then(|digest| digest.strip_prefix("sha256:"))
        {
            if !actual.eq_ignore_ascii_case(expected) {
                return Err("Release SHA-256 校验失败，安装已取消".into());
            }
        }
        let version_dir = state
            .root()
            .join("versions")
            .join(release.tag_name.trim_start_matches('v'));
        fs::create_dir_all(&version_dir).map_err(|error| error.to_string())?;
        state.set_busy(true, Some(82), Some("解压 Release 资产"));
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|error| format!("无法打开 ZIP：{error}"))?;
        archive
            .extract(&version_dir)
            .map_err(|error| format!("解压失败：{error}"))?;
        let executable_path =
            find_executable(&version_dir).ok_or("解压后未找到 axonhub 可执行文件")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable_path, fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
        let manifest = Manifest {
            version: release.tag_name.clone(),
            asset_name: asset.name,
            executable_path,
            sha256: actual,
            installed_at: Utc::now().to_rfc3339(),
        };
        stop_managed_process(&state)?;
        let mut runtime = state.runtime.lock().expect("runtime mutex poisoned");
        if let Some(current) = runtime.installed.take() {
            write_manifest(&state.root().join("previous.json"), &current)?;
            runtime.previous = Some(current);
        }
        write_manifest(&state.root().join("current.json"), &manifest)?;
        runtime.installed = Some(manifest);
        runtime.latest_version = Some(release.tag_name.clone());
        runtime.lifecycle = Lifecycle::Stopped;
        runtime.logs.push_back(LogEntry {
            timestamp: Utc::now().to_rfc3339(),
            level: "info".into(),
            message: format!("{} 安装完成并通过 SHA-256 校验", release.tag_name),
        });
        Ok(())
    }
    .await;
    state.set_busy(false, None, None);
    if let Err(error) = result {
        let error = state.fail(error);
        let _ = start_if_installed(&state).await;
        return Err(error);
    }
    start_managed_process(&state).await?;
    Ok(snapshot(&state).await)
}

async fn start_managed_process(state: &AppState) -> Result<()> {
    if state.shutting_down.load(Ordering::SeqCst) {
        return Err("应用正在退出，已取消启动 AxonHub".into());
    }
    let already_running = state
        .runtime
        .lock()
        .expect("runtime mutex poisoned")
        .child
        .is_some();
    if already_running {
        return Ok(());
    }
    let executable = {
        let mut runtime = state.runtime.lock().expect("runtime mutex poisoned");
        runtime.lifecycle = Lifecycle::Starting;
        runtime.last_error = None;
        runtime
            .installed
            .as_ref()
            .map(|item| item.executable_path.clone())
            .ok_or("请先安装 AxonHub")?
    };
    if !executable.is_file() {
        return Err(state.fail(format!(
            "启动 AxonHub 失败：可执行文件不存在：{}。请重新安装 AxonHub 或检查数据目录。",
            executable.display()
        )));
    }
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.process_log_path())
        .map_err(|error| error.to_string())?;
    let err_log = log.try_clone().map_err(|error| error.to_string())?;
    let mut command = Command::new(&executable);
    command
        .current_dir(state.root())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err_log));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let child = command
        .spawn()
        .map_err(|error| state.fail(format!("启动 AxonHub 失败：{error}")))?;
    let pid = child.id();
    {
        let mut runtime = state.runtime.lock().expect("runtime mutex poisoned");
        runtime.child = Some(child);
        runtime.lifecycle = Lifecycle::Starting;
    }
    state.log("info", format!("AxonHub 已启动，PID {pid}，等待健康检查"));
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    Ok(())
}

async fn start_if_installed(state: &AppState) -> Result<()> {
    let is_installed = state
        .runtime
        .lock()
        .expect("runtime mutex poisoned")
        .installed
        .is_some();
    if is_installed {
        start_managed_process(state).await?;
    }
    Ok(())
}

fn stop_managed_process(state: &AppState) -> Result<()> {
    let child = {
        state
            .runtime
            .lock()
            .expect("runtime mutex poisoned")
            .child
            .take()
    };
    let stopped_process = if let Some(mut child) = child {
        let pid = child.id();
        state.log("info", format!("正在停止 AxonHub，PID {pid}"));
        child
            .kill()
            .map_err(|error| state.fail(format!("停止进程失败：{error}")))?;
        child
            .wait()
            .map_err(|error| state.fail(format!("等待 AxonHub 退出失败：{error}")))?;
        true
    } else {
        false
    };
    {
        let mut runtime = state.runtime.lock().expect("runtime mutex poisoned");
        runtime.lifecycle = if runtime.installed.is_some() {
            Lifecycle::Stopped
        } else {
            Lifecycle::NotInstalled
        };
        runtime.last_error = None;
    }
    if stopped_process {
        state.log("info", "AxonHub 已停止");
    }
    Ok(())
}

#[tauri::command]
async fn start_axonhub(state: State<'_, AppState>) -> Result<Snapshot> {
    start_managed_process(&state).await?;
    Ok(snapshot(&state).await)
}

#[tauri::command]
async fn rollback_version(state: State<'_, AppState>) -> Result<Snapshot> {
    {
        let runtime = state.runtime.lock().expect("runtime mutex poisoned");
        if runtime.previous.is_none() {
            return Err("没有可回滚的历史版本".into());
        }
    }
    stop_managed_process(&state)?;
    let result: Result<()> = (|| {
        let mut runtime = state.runtime.lock().expect("runtime mutex poisoned");
        let previous = runtime.previous.take().ok_or("没有可回滚的历史版本")?;
        if let Some(current) = runtime.installed.replace(previous.clone()) {
            runtime.previous = Some(current);
        }
        write_manifest(&state.root().join("current.json"), &previous)?;
        if let Some(item) = &runtime.previous {
            write_manifest(&state.root().join("previous.json"), item)?;
        }
        runtime.lifecycle = Lifecycle::Stopped;
        runtime.logs.push_back(LogEntry {
            timestamp: Utc::now().to_rfc3339(),
            level: "warn".into(),
            message: format!("已回滚到 {}", previous.version),
        });
        Ok(())
    })();
    if let Err(error) = result {
        let _ = start_if_installed(&state).await;
        return Err(error);
    }
    start_managed_process(&state).await?;
    Ok(snapshot(&state).await)
}

#[tauri::command]
fn read_config(state: State<'_, AppState>) -> Result<String> {
    fs::read_to_string(state.config_path()).map_err(|error| error.to_string())
}

fn validate_yaml(content: &str) -> Result<()> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|error| format!("YAML 语法错误：{error}"))?;
    let port = value
        .get("server")
        .and_then(|item| item.get("port"))
        .and_then(|item| item.as_u64())
        .ok_or("缺少 server.port")?;
    if !(1..=65535).contains(&port) {
        return Err("server.port 必须在 1 到 65535 之间".into());
    }
    if value
        .get("db")
        .and_then(|item| item.get("dsn"))
        .and_then(|item| item.as_str())
        .unwrap_or_default()
        .is_empty()
    {
        return Err("db.dsn 不能为空".into());
    }
    Ok(())
}

#[tauri::command]
async fn save_config(content: String, state: State<'_, AppState>) -> Result<Snapshot> {
    validate_yaml(&content)?;
    let original = fs::read_to_string(state.config_path()).unwrap_or_default();
    stop_managed_process(&state)?;
    if let Err(error) = fs::write(state.config_path(), &content) {
        let _ = start_if_installed(&state).await;
        return Err(error.to_string());
    }
    if let Err(error) = validate_with_binary(&state) {
        let _ = fs::write(state.config_path(), original);
        let _ = start_if_installed(&state).await;
        return Err(error);
    }
    state.log("info", "config.yml 已保存并通过校验");
    start_if_installed(&state).await?;
    Ok(snapshot(&state).await)
}

fn validate_with_binary(state: &AppState) -> Result<()> {
    let executable = state
        .runtime
        .lock()
        .expect("runtime mutex poisoned")
        .installed
        .as_ref()
        .map(|item| item.executable_path.clone());
    let Some(executable) = executable else {
        return Ok(());
    };
    let output = Command::new(executable)
        .args(["config", "validate"])
        .current_dir(state.root())
        .output()
        .map_err(|error| format!("无法运行配置校验：{error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "AxonHub 配置校验失败：{}",
            String::from_utf8_lossy(&output.stdout).trim()
        ))
    }
}

#[tauri::command]
async fn validate_config(state: State<'_, AppState>) -> Result<Snapshot> {
    let content = fs::read_to_string(state.config_path()).map_err(|error| error.to_string())?;
    validate_yaml(&content)?;
    validate_with_binary(&state)?;
    state.log("info", "当前配置校验通过");
    Ok(snapshot(&state).await)
}

#[tauri::command]
async fn open_management(state: State<'_, AppState>) -> Result<Snapshot> {
    let port = port_from_config(&state.config_path());
    open::that(format!("http://127.0.0.1:{port}")).map_err(|error| error.to_string())?;
    Ok(snapshot(&state).await)
}

#[tauri::command]
async fn open_data_directory(state: State<'_, AppState>) -> Result<Snapshot> {
    open::that(state.root()).map_err(|error| error.to_string())?;
    Ok(snapshot(&state).await)
}

#[tauri::command]
async fn open_log_directory(state: State<'_, AppState>) -> Result<Snapshot> {
    open::that(state.root().join("logs")).map_err(|error| error.to_string())?;
    Ok(snapshot(&state).await)
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn request_shutdown(app: AppHandle) {
    let state = app.state::<AppState>();
    if state
        .shutting_down
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = stop_managed_process(&state) {
            state.log("error", format!("退出时停止 AxonHub 失败：{error}"));
        }
        drop(state);
        app.exit(0);
    });
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
            let state = AppState::new(app.handle()).map_err(std::io::Error::other)?;
            app.manage(state);

            let show_item =
                MenuItem::with_id(app, "show", "显示 AxonHub Manager", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            let mut tray = TrayIconBuilder::with_id("main")
                .menu(&menu)
                .tooltip("AxonHub Manager")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main_window(app),
                    "quit" => request_shutdown(app.clone()),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } | TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        }
                    ) {
                        show_main_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<AppState>();
                if let Err(error) = start_if_installed(&state).await {
                    state.fail(format!("自动启动 AxonHub 失败：{error}"));
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            check_updates,
            install_latest,
            start_axonhub,
            rollback_version,
            read_config,
            save_config,
            validate_config,
            open_management,
            open_data_directory,
            open_log_directory,
            set_autostart,
            is_data_directory_non_empty,
            change_data_directory
        ])
        .build(tauri::generate_context!())
        .expect("error while building AxonHub Manager");
    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            let state = app_handle.state::<AppState>();
            if !state.shutting_down.load(Ordering::SeqCst) {
                api.prevent_exit();
                drop(state);
                request_shutdown(app_handle.clone());
            }
        }
    });
}
