use std::{
    fs,
    net::TcpListener,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wait_timeout::ChildExt;

use crate::{
    injector::{probe_browser_identity, read_browser_identity, InjectorEngine},
    managed_launch::{
        candidate_still_present, debug_ports_from_records, has_remote_debugging_arg,
        snapshot_matching_processes, snapshot_matching_records, HostedAction, HostedInput,
        HostedMachine, ProcessKey, ProcessRecord, MSG_AUTO_APPLIED, MSG_DEBUG_TIMEOUT,
        MSG_EXISTING, MSG_NEED_MEDIA, MSG_SUSPENDED, MSG_TAKING_OVER, MSG_WAITING, MSG_WAIT_DEBUG,
        PHASE_ACTIVE, PHASE_BLOCKED, PHASE_ERROR, PHASE_PAUSED, PHASE_STARTING, PHASE_WAITING,
    },
    models::RuntimeStatus,
    payload::ActivePayload,
    settings::write_json_transaction,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DISCOVER_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$packages = @(Get-AppxPackage -Name 'OpenAI.Codex' | Sort-Object Version -Descending)
foreach ($package in $packages) {
  if ("$($package.SignatureKind)" -ine 'Store' -or [bool]$package.IsDevelopmentMode) { continue }
  $exe = Join-Path "$($package.InstallLocation)" 'app\ChatGPT.exe'
  if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { continue }
  $manifest = Get-AppxPackageManifest -Package $package
  $apps = @($manifest.Package.Applications.Application | Where-Object {
    "$($_.Executable)".Replace('/', '\') -ieq 'app\ChatGPT.exe'
  })
  if ($apps.Count -ne 1) { continue }
  $id = "$($apps[0].Id)"
  $family = "$($package.PackageFamilyName)"
  if ($family -cnotmatch '^[A-Za-z0-9._-]{1,128}$' -or $id -cnotmatch '^[A-Za-z0-9._-]{1,64}$') { continue }
  [pscustomobject]@{
    packageRoot = "$($package.InstallLocation)"
    executable = $exe
    version = "$($package.Version)"
    packageFullName = "$($package.PackageFullName)"
    packageFamilyName = $family
    applicationId = $id
    appUserModelId = "$family!$id"
  } | ConvertTo-Json -Compress
  exit 0
}
throw '未找到经过验证的官方 OpenAI.Codex Store 应用。'
"#;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexInstall {
    package_root: String,
    executable: String,
    version: String,
    package_full_name: String,
    #[allow(dead_code)]
    package_family_name: String,
    #[allow(dead_code)]
    application_id: String,
    app_user_model_id: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeState {
    schema_version: u8,
    port: u16,
    browser_id: String,
    package_full_name: String,
    executable: String,
    created_at: String,
}

fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn encoded_powershell(script: &str) -> String {
    let bytes = script
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    STANDARD.encode(bytes)
}

fn run_powershell(script: &str, timeout: Duration) -> Result<String, String> {
    let mut child = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encoded_powershell(script),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| error.to_string())?;
    if child
        .wait_timeout(timeout)
        .map_err(|error| error.to_string())?
        .is_none()
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err("PowerShell 操作超时。".to_string());
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            if stdout.is_empty() {
                "PowerShell 操作失败。".to_string()
            } else {
                stdout
            }
        } else {
            stderr
        })
    }
}

fn valid_identity(value: &str) -> bool {
    let Some((family, application)) = value.split_once('!') else {
        return false;
    };
    !family.is_empty()
        && family.len() <= 128
        && !application.is_empty()
        && application.len() <= 64
        && family
            .chars()
            .chain(application.chars())
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
}

fn normalized_path(path: &str) -> String {
    Path::new(path)
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}

fn discover_codex() -> Result<CodexInstall, String> {
    let raw = run_powershell(DISCOVER_SCRIPT, Duration::from_secs(30))?;
    let install: CodexInstall =
        serde_json::from_str(&raw).map_err(|error| format!("Codex 安装信息无效：{error}"))?;
    let root = format!(
        "{}\\",
        normalized_path(&install.package_root).trim_end_matches('\\')
    );
    if !valid_identity(&install.app_user_model_id)
        || !normalized_path(&install.executable).starts_with(&root)
    {
        return Err("Codex 安装身份校验失败。".to_string());
    }
    Ok(install)
}

fn matching_records(install: &CodexInstall) -> Result<Vec<ProcessRecord>, String> {
    snapshot_matching_records(&install.executable)
}

fn matching_processes(install: &CodexInstall) -> Result<Vec<ProcessKey>, String> {
    snapshot_matching_processes(&install.executable)
}

fn process_ids_for(install: &CodexInstall) -> Result<Vec<u32>, String> {
    Ok(matching_processes(install)?
        .into_iter()
        .map(|process| process.pid)
        .collect())
}

fn debug_ports_for(install: &CodexInstall) -> Result<Vec<u16>, String> {
    let script = format!(
        r#"
$target = {}
$ports = @(Get-CimInstance Win32_Process -Filter "Name='ChatGPT.exe'" | Where-Object {{
  $_.ExecutablePath -and $_.CommandLine -and
  [IO.Path]::GetFullPath($_.ExecutablePath).Equals($target, [StringComparison]::OrdinalIgnoreCase)
}} | ForEach-Object {{
  $match = [regex]::Match("$($_.CommandLine)", '(?:^|\s)"?--remote-debugging-port=(\d+)"?(?:\s|$)')
  if ($match.Success) {{ [int]$match.Groups[1].Value }}
}})
@($ports | Sort-Object -Unique) | ConvertTo-Json -Compress
"#,
        powershell_quote(&normalized_path(&install.executable))
    );
    let raw = run_powershell(&script, Duration::from_secs(30))?;
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    let values = match value {
        Value::Array(values) => values,
        value => vec![value],
    };
    Ok(values
        .into_iter()
        .filter_map(|value| value.as_u64())
        .filter_map(|value| u16::try_from(value).ok())
        .collect())
}

fn stop_verified_codex(install: &CodexInstall) -> Result<(), String> {
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$target = {}
$processes = @(Get-CimInstance Win32_Process -Filter "Name='ChatGPT.exe'" | Where-Object {{
  $_.ExecutablePath -and [IO.Path]::GetFullPath($_.ExecutablePath).Equals($target, [StringComparison]::OrdinalIgnoreCase)
}})
foreach ($item in $processes) {{ Stop-Process -Id ([int]$item.ProcessId) -Force -ErrorAction SilentlyContinue }}
"#,
        powershell_quote(&normalized_path(&install.executable))
    );
    run_powershell(&script, Duration::from_secs(30))?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while !process_ids_for(install)?.is_empty() {
        if Instant::now() >= deadline {
            return Err("Codex 未能在 15 秒内完全退出。".to_string());
        }
        thread::sleep(Duration::from_millis(300));
    }
    Ok(())
}

fn launch_codex(install: &CodexInstall, arguments: &[String]) -> Result<(), String> {
    if !valid_identity(&install.app_user_model_id) {
        return Err("Codex AppUserModelId 无效。".to_string());
    }
    let argument_line = arguments
        .iter()
        .map(|argument| format!("\"{}\"", argument.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace CodexBackgroundStudio {{
  [ComImport, Guid("2e941141-7f97-4756-ba1d-9decde894a3d"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IApplicationActivationManager {{
    [PreserveSig] int ActivateApplication(
      [MarshalAs(UnmanagedType.LPWStr)] string appUserModelId,
      [MarshalAs(UnmanagedType.LPWStr)] string arguments,
      uint options,
      out uint processId);
  }}
  [ComImport, Guid("45ba127d-10a8-46ea-8ab7-56ea9078943c")]
  class ApplicationActivationManager {{}}
  public static class Launcher {{
    public static uint Launch(string id, string arguments) {{
      var manager = (IApplicationActivationManager)new ApplicationActivationManager();
      try {{
        uint processId;
        int result = manager.ActivateApplication(id, arguments ?? "", 0, out processId);
        Marshal.ThrowExceptionForHR(result);
        return processId;
      }} finally {{
        if (Marshal.IsComObject(manager)) Marshal.FinalReleaseComObject(manager);
      }}
    }}
  }}
}}
'@
$launchedProcessId = [CodexBackgroundStudio.Launcher]::Launch({}, {})
if ($launchedProcessId -le 0) {{ throw 'Windows 未返回 Codex 进程 ID。' }}
$launchedProcessId
"#,
        powershell_quote(&install.app_user_model_id),
        powershell_quote(&argument_line)
    );
    run_powershell(&script, Duration::from_secs(30)).map(|_| ())
}

fn select_port(preferred: u16) -> Result<u16, String> {
    for port in preferred..=preferred.saturating_add(100) {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    Err("无法为 Codex 分配本机调试端口。".to_string())
}

pub struct ManagedDecision {
    pub action: HostedAction,
    pub candidate: Vec<ProcessKey>,
}

impl ManagedDecision {
    fn from_action(action: HostedAction) -> Self {
        Self {
            action,
            candidate: Vec::new(),
        }
    }
}

pub struct CodexController {
    state_path: PathBuf,
    engine: Option<InjectorEngine>,
    state: Option<RuntimeState>,
    status: RuntimeStatus,
    hosted: HostedMachine,
    install: Option<CodexInstall>,
    empty_ticks: u32,
    health_ticks: u32,
    debug_ports_cache: Vec<u16>,
    debug_ports_generation: Option<u64>,
    debug_ports_refresh_ticks: u32,
    debug_ports_refresh_count: u32,
    last_probe_error_ticks: u32,
}

impl CodexController {
    pub fn load(data_directory: &Path) -> Self {
        let state_path = data_directory.join("runtime.json");
        let state = fs::read_to_string(&state_path)
            .ok()
            .and_then(|content| serde_json::from_str::<RuntimeState>(&content).ok())
            .filter(|state| state.schema_version == 1 && !state.browser_id.is_empty());
        Self {
            state_path,
            engine: None,
            state,
            status: RuntimeStatus::default(),
            hosted: HostedMachine::new(),
            install: None,
            empty_ticks: 0,
            health_ticks: 0,
            debug_ports_cache: Vec::new(),
            debug_ports_generation: None,
            debug_ports_refresh_ticks: 0,
            debug_ports_refresh_count: 0,
            last_probe_error_ticks: 0,
        }
    }

    pub fn status(&self) -> RuntimeStatus {
        let mut status = self.status.clone();
        if let Some(engine) = &self.engine {
            status.active_targets = engine.active_targets();
        }
        status
    }

    fn write_state(&mut self, state: Option<RuntimeState>) -> Result<(), String> {
        self.state = state;
        match &self.state {
            Some(state) => write_json_transaction(&self.state_path, state),
            None => match fs::remove_file(&self.state_path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.to_string()),
            },
        }
    }

    fn try_attach_saved(&mut self, install: &CodexInstall) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        if state.package_full_name != install.package_full_name
            || normalized_path(&state.executable) != normalized_path(&install.executable)
            || read_browser_identity(state.port).ok().as_deref() != Some(&state.browser_id)
        {
            return false;
        }
        self.engine = Some(InjectorEngine::new(state.port, state.browser_id.clone()));
        true
    }

    fn drop_engine(&mut self) {
        if let Some(mut engine) = self.engine.take() {
            let _ = engine.stop();
        }
    }

    fn cached_install(&mut self, processes_empty: bool) -> Result<CodexInstall, String> {
        let rediscover = self.install.is_none()
            || (processes_empty && self.empty_ticks > 0 && self.empty_ticks % 20 == 0);
        if rediscover {
            self.install = Some(discover_codex()?);
        }
        self.install
            .clone()
            .ok_or_else(|| "未找到经过验证的官方 OpenAI.Codex Store 应用。".to_string())
    }

    fn current_keys(&self, install: &CodexInstall) -> Result<Vec<ProcessKey>, String> {
        matching_processes(install)
    }

    fn mark_managed(&mut self, install: &CodexInstall) {
        if !self.hosted.is_armed() {
            return;
        }
        let keys = self.current_keys(install).unwrap_or_default();
        self.hosted.rearm_after_apply(&keys);
    }

    fn set_status(
        &mut self,
        phase: &str,
        message: &str,
        version: Option<String>,
        error: Option<String>,
    ) {
        self.status.phase = phase.to_string();
        self.status.message = message.to_string();
        if version.is_some() {
            self.status.codex_version = version;
        }
        self.status.last_error = error;
    }

    fn attach_existing(
        &mut self,
        install: &CodexInstall,
        payload: &ActivePayload,
    ) -> Result<bool, String> {
        if let Some(engine) = &self.engine {
            if engine.is_connected() {
                engine.update(payload.clone())?;
                return Ok(true);
            }
            self.drop_engine();
        }
        if self.try_attach_saved(install) {
            self.engine
                .as_mut()
                .expect("engine set after attach")
                .start(payload.clone())?;
            return Ok(true);
        }
        for port in debug_ports_for(install)? {
            let Ok(browser_id) = read_browser_identity(port) else {
                continue;
            };
            self.write_state(Some(RuntimeState {
                schema_version: 1,
                port,
                browser_id: browser_id.clone(),
                package_full_name: install.package_full_name.clone(),
                executable: install.executable.clone(),
                created_at: Utc::now().to_rfc3339(),
            }))?;
            let mut engine = InjectorEngine::new(port, browser_id);
            engine.start(payload.clone())?;
            self.engine = Some(engine);
            return Ok(true);
        }
        Ok(false)
    }

    fn launch_debug_session(
        &mut self,
        install: &CodexInstall,
        payload: ActivePayload,
    ) -> Result<(), String> {
        if !process_ids_for(install)?.is_empty() {
            stop_verified_codex(install)?;
        }
        let port = select_port(9335)?;
        launch_codex(
            install,
            &[
                "--remote-debugging-address=127.0.0.1".to_string(),
                format!("--remote-debugging-port={port}"),
            ],
        )?;
        let deadline = Instant::now() + Duration::from_secs(45);
        let browser_id = loop {
            if let Ok(identity) = read_browser_identity(port) {
                break identity;
            }
            if Instant::now() >= deadline {
                return Err("Codex 未能在 45 秒内打开安全的本机调试端口。".to_string());
            }
            thread::sleep(Duration::from_millis(400));
        };
        self.write_state(Some(RuntimeState {
            schema_version: 1,
            port,
            browser_id: browser_id.clone(),
            package_full_name: install.package_full_name.clone(),
            executable: install.executable.clone(),
            created_at: Utc::now().to_rfc3339(),
        }))?;
        let mut engine = InjectorEngine::new(port, browser_id);
        engine.start(payload)?;
        self.engine = Some(engine);
        Ok(())
    }

    fn release_stale_session(&mut self) -> Result<(), String> {
        self.drop_engine();
        self.write_state(None)?;
        self.debug_ports_cache.clear();
        self.debug_ports_generation = None;
        self.debug_ports_refresh_ticks = 0;
        self.debug_ports_refresh_count = 0;
        self.hosted.reset_to_waiting();
        self.status.active_targets = 0;
        Ok(())
    }

    fn apply_status_for_attach(
        &mut self,
        install: &CodexInstall,
        automatic: bool,
        live_update: bool,
    ) {
        let message = if automatic {
            MSG_AUTO_APPLIED
        } else if live_update {
            "背景已实时应用"
        } else {
            "已重新连接背景会话"
        };
        self.set_status(PHASE_ACTIVE, message, Some(install.version.clone()), None);
        self.mark_managed(install);
    }

    fn refresh_debug_ports(
        &mut self,
        install: &CodexInstall,
        records: &[ProcessRecord],
    ) -> Vec<u16> {
        let generation = records.iter().map(|record| record.key.created_at).min();
        let native_ports = debug_ports_from_records(records);
        let cmdline_unknown = records.iter().any(|record| record.command_line.is_none());
        if self.debug_ports_generation != generation {
            self.debug_ports_generation = generation;
            self.debug_ports_cache = native_ports.clone();
            self.debug_ports_refresh_ticks = 0;
            self.debug_ports_refresh_count = 0;
        } else if !native_ports.is_empty() {
            self.debug_ports_cache = native_ports;
        } else if self.debug_ports_cache.is_empty() && cmdline_unknown {
            self.debug_ports_refresh_ticks = self.debug_ports_refresh_ticks.saturating_add(1);
            if self.debug_ports_refresh_ticks % 4 == 0 && self.debug_ports_refresh_count < 8 {
                self.debug_ports_refresh_count = self.debug_ports_refresh_count.saturating_add(1);
                if let Ok(ports) = debug_ports_for(install) {
                    self.debug_ports_cache = ports;
                }
            }
        }
        self.debug_ports_cache.clone()
    }

    pub fn probe_managed(&mut self) -> Result<ManagedDecision, String> {
        if self.hosted.is_paused() {
            self.set_status(PHASE_PAUSED, MSG_SUSPENDED, None, None);
            return Ok(ManagedDecision::from_action(HostedAction::StaySuspended));
        }

        let cached_empty = match &self.install {
            Some(install) => match matching_records(install) {
                Ok(records) => records.is_empty(),
                Err(_) => true,
            },
            None => true,
        };
        if cached_empty {
            self.empty_ticks = self.empty_ticks.saturating_add(1);
        } else {
            self.empty_ticks = 0;
        }

        let install = match self.cached_install(cached_empty) {
            Ok(install) => {
                self.last_probe_error_ticks = 0;
                install
            }
            Err(error) => {
                self.last_probe_error_ticks = self.last_probe_error_ticks.saturating_add(1);
                if self.status.phase == PHASE_ACTIVE
                    && !self
                        .engine
                        .as_ref()
                        .is_some_and(InjectorEngine::is_connected)
                {
                    self.drop_engine();
                    self.set_status(PHASE_WAITING, MSG_WAITING, None, None);
                } else if self.status.phase != PHASE_ACTIVE {
                    self.set_status(PHASE_ERROR, &error, None, Some(error.clone()));
                }
                return Ok(ManagedDecision::from_action(HostedAction::Wait));
            }
        };

        let records = match matching_records(&install) {
            Ok(records) => {
                self.last_probe_error_ticks = 0;
                records
            }
            Err(error) => {
                self.last_probe_error_ticks = self.last_probe_error_ticks.saturating_add(1);
                if !self
                    .engine
                    .as_ref()
                    .is_some_and(InjectorEngine::is_connected)
                {
                    self.drop_engine();
                    if self.status.phase == PHASE_ACTIVE || self.last_probe_error_ticks >= 3 {
                        self.set_status(PHASE_WAITING, MSG_WAITING, Some(install.version), None);
                    } else if self.status.phase != PHASE_ACTIVE {
                        self.set_status(
                            PHASE_WAITING,
                            &format!("进程探测暂时失败：{error}"),
                            Some(install.version),
                            None,
                        );
                    }
                }
                return Ok(ManagedDecision::from_action(HostedAction::Wait));
            }
        };
        let keys = records.iter().map(|record| record.key).collect::<Vec<_>>();
        let mut connected = false;
        if keys.is_empty() {
            if self.engine.is_some() {
                self.drop_engine();
            }
        } else if self.engine.is_some() {
            self.health_ticks = self.health_ticks.saturating_add(1);
            connected = if self.health_ticks % 6 == 0 {
                let live = self
                    .engine
                    .as_ref()
                    .is_some_and(InjectorEngine::is_connected);
                if !live {
                    self.drop_engine();
                }
                live
            } else {
                true
            };
        }

        let mut has_ready_debug_session = false;
        let mut debug_starting = false;
        let mut cmdline_pending = false;
        if !keys.is_empty() && !connected {
            if let Some(state) = &self.state {
                if probe_browser_identity(state.port).ok().as_deref()
                    == Some(state.browser_id.as_str())
                {
                    has_ready_debug_session = true;
                }
            }
            if !has_ready_debug_session {
                let ports = self.refresh_debug_ports(&install, &records);
                if ports
                    .iter()
                    .any(|port| probe_browser_identity(*port).is_ok())
                {
                    has_ready_debug_session = true;
                } else {
                    let has_debug_arg = records.iter().any(|record| {
                        record
                            .command_line
                            .as_deref()
                            .is_some_and(has_remote_debugging_arg)
                    });
                    let cmdline_unknown =
                        records.iter().any(|record| record.command_line.is_none());
                    debug_starting = has_debug_arg || !ports.is_empty();
                    cmdline_pending = cmdline_unknown && !has_debug_arg && ports.is_empty();
                }
            }
        }

        let decision = self.hosted.decide(&HostedInput {
            processes: keys,
            connected,
            has_ready_debug_session,
            debug_starting,
            cmdline_pending,
            now: Instant::now(),
        });
        match decision.action {
            HostedAction::Wait => {
                self.set_status(PHASE_WAITING, MSG_WAITING, Some(install.version), None);
            }
            HostedAction::ReportExistingUnmanaged => {
                if self.status.message == MSG_NEED_MEDIA {
                    self.set_status(
                        PHASE_BLOCKED,
                        MSG_NEED_MEDIA,
                        Some(install.version),
                        self.status.last_error.clone(),
                    );
                } else {
                    self.set_status(
                        PHASE_BLOCKED,
                        MSG_EXISTING,
                        Some(install.version),
                        self.status.last_error.clone(),
                    );
                }
            }
            HostedAction::WaitForDebug => {
                self.set_status(PHASE_WAITING, MSG_WAIT_DEBUG, Some(install.version), None);
            }
            HostedAction::ReportDebugTimeout => {
                self.set_status(
                    PHASE_ERROR,
                    MSG_DEBUG_TIMEOUT,
                    Some(install.version),
                    Some(MSG_DEBUG_TIMEOUT.to_string()),
                );
            }
            HostedAction::Attach | HostedAction::Takeover => {
                self.set_status(PHASE_STARTING, MSG_TAKING_OVER, Some(install.version), None);
            }
            HostedAction::KeepActive => {
                self.status.phase = PHASE_ACTIVE.to_string();
                self.status.codex_version = Some(install.version);
                self.status.last_error = None;
            }
            HostedAction::CleanupDisconnected => {
                self.release_stale_session()?;
                self.set_status(PHASE_WAITING, MSG_WAITING, Some(install.version), None);
            }
            HostedAction::StaySuspended => {
                self.set_status(PHASE_PAUSED, MSG_SUSPENDED, Some(install.version), None);
            }
        }
        Ok(ManagedDecision {
            action: decision.action,
            candidate: decision.candidate,
        })
    }

    pub fn try_attach(&mut self, payload: ActivePayload) -> Result<bool, String> {
        if self.hosted.is_paused() {
            return Ok(false);
        }
        self.set_status(PHASE_STARTING, MSG_TAKING_OVER, None, None);
        let install = self.cached_install(false).or_else(|_| discover_codex())?;
        self.install = Some(install.clone());
        match self.attach_existing(&install, &payload) {
            Ok(true) => {
                self.apply_status_for_attach(&install, true, true);
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(error) => {
                self.drop_engine();
                let keys = matching_processes(&install).unwrap_or_default();
                self.hosted.note_takeover_failed(&keys);
                self.set_status(PHASE_ERROR, &error, None, Some(error.clone()));
                Err(error)
            }
        }
    }

    pub fn note_payload_unavailable(&mut self, error: &str) -> Result<(), String> {
        let keys = self
            .install
            .as_ref()
            .and_then(|install| matching_processes(install).ok())
            .unwrap_or_default();
        if !keys.is_empty() {
            self.hosted.note_takeover_failed(&keys);
        }
        if error.contains("请先从媒体库选择") {
            self.set_status(PHASE_BLOCKED, MSG_NEED_MEDIA, None, None);
        } else {
            self.set_status(PHASE_ERROR, error, None, Some(error.to_string()));
        }
        Ok(())
    }

    pub fn run_managed_action(&mut self, payload: ActivePayload) -> Result<(), String> {
        if self.hosted.is_paused() {
            return Ok(());
        }
        let decision = self.probe_managed()?;
        match decision.action {
            HostedAction::Attach => self.try_attach(payload).map(|_| ()),
            HostedAction::Takeover => {
                if decision.candidate.is_empty() {
                    return Ok(());
                }
                let current = self
                    .install
                    .as_ref()
                    .and_then(|install| matching_processes(install).ok())
                    .unwrap_or_default();
                if current.is_empty() || !candidate_still_present(&decision.candidate, &current) {
                    return Ok(());
                }
                self.takeover(payload).map(|_| ())
            }
            _ => Ok(()),
        }
    }

    pub fn takeover(&mut self, payload: ActivePayload) -> Result<RuntimeStatus, String> {
        self.apply_inner(payload, true, true)
    }

    pub fn reconnect_saved(&mut self, payload: ActivePayload) -> Result<bool, String> {
        if self.state.is_none() {
            return Ok(false);
        }
        let install = discover_codex()?;
        self.install = Some(install.clone());
        if !self.try_attach_saved(&install) {
            self.write_state(None)?;
            self.status = RuntimeStatus::default();
            return Ok(false);
        }
        let result = self
            .engine
            .as_mut()
            .expect("engine set after saved session validation")
            .start(payload);
        match result {
            Ok(()) => {
                self.apply_status_for_attach(&install, true, true);
                self.status.message = "已自动恢复背景会话".to_string();
                Ok(true)
            }
            Err(error) => {
                self.drop_engine();
                self.set_status(PHASE_ERROR, &error, None, Some(error.clone()));
                Err(error)
            }
        }
    }

    pub fn apply(
        &mut self,
        payload: ActivePayload,
        restart_existing: bool,
    ) -> Result<RuntimeStatus, String> {
        self.apply_inner(payload, restart_existing, false)
    }

    fn apply_inner(
        &mut self,
        payload: ActivePayload,
        restart_existing: bool,
        automatic: bool,
    ) -> Result<RuntimeStatus, String> {
        self.set_status(
            PHASE_STARTING,
            if automatic {
                MSG_TAKING_OVER
            } else {
                "正在连接 Codex"
            },
            None,
            None,
        );
        let result: Result<RuntimeStatus, String> = (|| {
            let install = discover_codex()?;
            self.install = Some(install.clone());
            let live_update = self
                .engine
                .as_ref()
                .is_some_and(InjectorEngine::is_connected);
            if self.attach_existing(&install, &payload)? {
                self.apply_status_for_attach(&install, automatic, live_update);
                return Ok(self.status());
            }
            let running = process_ids_for(&install)?;
            if !running.is_empty() && !restart_existing {
                return Err("Codex 需要重启一次以启用背景。".to_string());
            }
            self.launch_debug_session(&install, payload)?;
            self.set_status(
                PHASE_ACTIVE,
                if automatic {
                    MSG_AUTO_APPLIED
                } else {
                    "背景已应用"
                },
                Some(install.version.clone()),
                None,
            );
            self.mark_managed(&install);
            Ok(self.status())
        })();
        if let Err(error) = &result {
            let keys = self
                .install
                .as_ref()
                .and_then(|install| matching_processes(install).ok())
                .unwrap_or_default();
            if automatic && !keys.is_empty() {
                self.hosted.note_takeover_failed(&keys);
            }
            self.status.phase = if error.contains("需要重启一次") {
                if self.hosted.is_armed() {
                    PHASE_BLOCKED.to_string()
                } else {
                    "idle".to_string()
                }
            } else {
                PHASE_ERROR.to_string()
            };
            self.status.message = if error.contains("需要重启一次") && self.hosted.is_armed()
            {
                MSG_EXISTING.to_string()
            } else {
                error.clone()
            };
            self.status.last_error = Some(error.clone());
        }
        result
    }

    pub fn pause(&mut self) -> Result<RuntimeStatus, String> {
        if let Some(engine) = &self.engine {
            engine.pause()?;
        }
        self.hosted.suspend();
        if self.hosted.is_armed() {
            self.set_status(PHASE_PAUSED, MSG_SUSPENDED, None, None);
        } else {
            self.set_status("paused", "背景已暂停", None, None);
        }
        Ok(self.status())
    }

    pub fn restore(&mut self) -> Result<RuntimeStatus, String> {
        self.status.phase = "restoring".to_string();
        self.status.message = "正在恢复官方外观".to_string();
        self.status.last_error = None;
        let hosted = self.hosted.is_armed();
        let result: Result<RuntimeStatus, String> = (|| {
            self.drop_engine();
            let install = discover_codex()?;
            self.install = Some(install.clone());
            if !process_ids_for(&install)?.is_empty() {
                stop_verified_codex(&install)?;
                launch_codex(&install, &[])?;
            }
            self.write_state(None)?;
            self.hosted.suspend();
            if hosted {
                self.set_status(PHASE_PAUSED, MSG_SUSPENDED, Some(install.version), None);
            } else {
                self.set_status("idle", "已恢复官方外观", Some(install.version), None);
            }
            self.status.active_targets = 0;
            Ok(self.status())
        })();
        if let Err(error) = &result {
            self.set_status(PHASE_ERROR, error, None, Some(error.clone()));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_store_identity_and_powershell_quoting() {
        assert!(valid_identity("OpenAI.Codex_8wekyb3d8bbwe!App"));
        assert!(!valid_identity("OpenAI.Codex!App;Start-Process"));
        assert_eq!(powershell_quote("a'b"), "'a''b'");
    }

    #[test]
    fn selects_an_available_loopback_port() {
        let port = select_port(39_000).expect("available test port");
        assert!((39_000..=39_100).contains(&port));
    }

    #[test]
    #[ignore = "requires the official Windows Store Codex installation"]
    fn discovers_installed_store_codex_and_reads_processes() {
        let install = discover_codex().expect("discover official Codex");
        assert!(valid_identity(&install.app_user_model_id));
        process_ids_for(&install).expect("query verified Codex processes");
        for port in debug_ports_for(&install).expect("query verified Codex debug ports") {
            read_browser_identity(port).expect("verify Codex browser identity");
        }
    }
}
