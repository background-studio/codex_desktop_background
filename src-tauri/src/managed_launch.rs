use std::path::Path;
use std::time::{Duration, Instant};

pub const MSG_WAITING: &str = "已启用，等待 Codex 启动";
pub const MSG_EXISTING: &str = "Codex 已在运行，点立即接管可重启";
pub const MSG_TAKING_OVER: &str = "正在接管 Codex";
pub const MSG_WAIT_DEBUG: &str = "正在等待 Codex 调试端口就绪";
pub const MSG_DEBUG_TIMEOUT: &str = "Codex 调试端口未能在 45 秒内就绪，等待进程退出后重试";
pub const MSG_NEED_MEDIA: &str = "请先选择背景后再接管 Codex";
pub const MSG_UNCONFIGURED: &str = "尚未配置背景";
pub const MSG_AUTO_APPLIED: &str = "背景已自动应用";
pub const MSG_SUSPENDED: &str = "暂停托管";

pub const PHASE_WAITING: &str = "waiting";
pub const PHASE_BLOCKED: &str = "blocked";
pub const PHASE_STARTING: &str = "starting";
pub const PHASE_ACTIVE: &str = "active";
pub const PHASE_PAUSED: &str = "paused";
pub const PHASE_ERROR: &str = "error";

pub const DEBUG_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const DEFAULT_TAKEOVER_CONFIRMATIONS: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProcessKey {
    pub pid: u32,
    pub created_at: u64,
}

#[derive(Clone, Debug)]
pub struct ProcessRecord {
    pub key: ProcessKey,
    pub command_line: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostedPhase {
    Waiting,
    ExistingUnmanaged,
    AttachPending,
    Takeover,
    Active,
    Suspended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostedAction {
    Wait,
    ReportExistingUnmanaged,
    WaitForDebug,
    ReportDebugTimeout,
    Attach,
    Takeover,
    KeepActive,
    CleanupDisconnected,
    StaySuspended,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedDecision {
    pub action: HostedAction,
    pub candidate: Vec<ProcessKey>,
}

#[derive(Clone, Debug)]
pub struct HostedInput {
    pub processes: Vec<ProcessKey>,
    pub connected: bool,
    pub has_ready_debug_session: bool,
    pub debug_starting: bool,
    pub cmdline_pending: bool,
    pub now: Instant,
}

impl HostedInput {
    #[cfg(test)]
    pub fn new(processes: Vec<ProcessKey>) -> Self {
        Self {
            processes,
            connected: false,
            has_ready_debug_session: false,
            debug_starting: false,
            cmdline_pending: false,
            now: Instant::now(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct HostedMachine {
    armed: bool,
    paused: bool,
    baseline: Vec<ProcessKey>,
    session: Vec<ProcessKey>,
    confirmations: u8,
    required_confirmations: u8,
    phase: HostedPhase,
    debug_wait_started: Option<Instant>,
    debug_timed_out: bool,
    takeover_candidate: Vec<ProcessKey>,
}

impl Default for HostedMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl HostedMachine {
    pub fn new() -> Self {
        Self {
            armed: false,
            paused: false,
            baseline: Vec::new(),
            session: Vec::new(),
            confirmations: 0,
            required_confirmations: DEFAULT_TAKEOVER_CONFIRMATIONS,
            phase: HostedPhase::Waiting,
            debug_wait_started: None,
            debug_timed_out: false,
            takeover_candidate: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn with_confirmations(mut self, required: u8) -> Self {
        self.required_confirmations = required.max(1);
        self
    }

    #[cfg(test)]
    pub fn phase(&self) -> HostedPhase {
        self.phase
    }

    pub fn is_armed(&self) -> bool {
        self.armed
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    fn clear_debug_wait(&mut self) {
        self.debug_wait_started = None;
        self.debug_timed_out = false;
    }

    fn clear_takeover_candidate(&mut self) {
        self.takeover_candidate.clear();
        self.confirmations = 0;
    }

    fn outcome(&self, action: HostedAction) -> HostedDecision {
        HostedDecision {
            action,
            candidate: if action == HostedAction::Takeover {
                self.takeover_candidate.clone()
            } else {
                Vec::new()
            },
        }
    }

    pub fn arm(&mut self, processes: &[ProcessKey]) {
        self.armed = true;
        self.paused = false;
        self.baseline = processes.to_vec();
        self.session.clear();
        self.clear_takeover_candidate();
        self.clear_debug_wait();
        self.phase = if processes.is_empty() {
            HostedPhase::Waiting
        } else {
            HostedPhase::ExistingUnmanaged
        };
    }

    pub fn ensure_armed(&mut self, processes: &[ProcessKey]) {
        if !self.armed {
            self.arm(processes);
        }
    }

    pub fn suspend(&mut self) {
        self.paused = true;
        self.phase = HostedPhase::Suspended;
        self.clear_takeover_candidate();
        self.debug_wait_started = None;
    }

    pub fn rearm_after_apply(&mut self, processes: &[ProcessKey]) {
        self.armed = true;
        self.paused = false;
        self.baseline.clear();
        self.session = processes.to_vec();
        self.clear_takeover_candidate();
        self.clear_debug_wait();
        self.phase = HostedPhase::Active;
    }

    pub fn reset_to_waiting(&mut self) {
        self.session.clear();
        self.baseline.clear();
        self.clear_takeover_candidate();
        self.clear_debug_wait();
        if !self.paused {
            self.phase = HostedPhase::Waiting;
        }
    }

    pub fn note_takeover_failed(&mut self, processes: &[ProcessKey]) {
        self.baseline = processes.to_vec();
        self.session.clear();
        self.clear_takeover_candidate();
        self.debug_wait_started = None;
        if !self.paused {
            self.phase = HostedPhase::ExistingUnmanaged;
        }
    }

    pub fn decide(&mut self, input: &HostedInput) -> HostedDecision {
        if self.paused {
            self.phase = HostedPhase::Suspended;
            return self.outcome(HostedAction::StaySuspended);
        }
        self.ensure_armed(&input.processes);

        if input.processes.is_empty() {
            let had_session = !self.session.is_empty()
                || !self.takeover_candidate.is_empty()
                || input.connected
                || self.debug_timed_out
                || matches!(
                    self.phase,
                    HostedPhase::Active | HostedPhase::AttachPending | HostedPhase::Takeover
                );
            self.reset_to_waiting();
            return self.outcome(if had_session {
                HostedAction::CleanupDisconnected
            } else {
                HostedAction::Wait
            });
        }

        if self.debug_timed_out && overlaps(&input.processes, &self.baseline) {
            self.clear_takeover_candidate();
            self.phase = HostedPhase::ExistingUnmanaged;
            return self.outcome(HostedAction::ReportDebugTimeout);
        }

        if input.connected {
            self.session = input.processes.clone();
            self.clear_takeover_candidate();
            self.clear_debug_wait();
            self.phase = HostedPhase::Active;
            return self.outcome(HostedAction::KeepActive);
        }

        if input.has_ready_debug_session {
            self.session = input.processes.clone();
            self.clear_takeover_candidate();
            self.clear_debug_wait();
            self.phase = HostedPhase::AttachPending;
            return self.outcome(HostedAction::Attach);
        }

        if input.debug_starting || input.cmdline_pending {
            return self.decide_debug_wait(input);
        }

        self.debug_wait_started = None;

        if overlaps(&input.processes, &self.baseline) {
            self.clear_takeover_candidate();
            self.phase = HostedPhase::ExistingUnmanaged;
            return self.outcome(HostedAction::ReportExistingUnmanaged);
        }

        // 同一 Electron 会话里后拉起的同路径子进程不能当成一次新启动。
        if overlaps(&input.processes, &self.session) {
            self.clear_takeover_candidate();
            self.phase = HostedPhase::ExistingUnmanaged;
            return self.outcome(HostedAction::ReportExistingUnmanaged);
        }

        if !self.takeover_candidate.is_empty() {
            if overlaps(&input.processes, &self.takeover_candidate) {
                self.phase = HostedPhase::Takeover;
                return self.outcome(HostedAction::Takeover);
            }
            self.takeover_candidate.clear();
            self.confirmations = 0;
            self.phase = HostedPhase::AttachPending;
            return self.outcome(HostedAction::Wait);
        }

        self.confirmations = self.confirmations.saturating_add(1);
        if self.confirmations >= self.required_confirmations {
            self.takeover_candidate = input.processes.clone();
            self.phase = HostedPhase::Takeover;
            self.outcome(HostedAction::Takeover)
        } else {
            self.phase = HostedPhase::AttachPending;
            self.outcome(HostedAction::Wait)
        }
    }

    fn decide_debug_wait(&mut self, input: &HostedInput) -> HostedDecision {
        let started = *self.debug_wait_started.get_or_insert(input.now);
        if input.now.saturating_duration_since(started) >= DEBUG_WAIT_TIMEOUT {
            self.debug_timed_out = true;
            self.debug_wait_started = None;
            self.note_takeover_failed(&input.processes);
            self.debug_timed_out = true;
            self.phase = HostedPhase::ExistingUnmanaged;
            return self.outcome(HostedAction::ReportDebugTimeout);
        }
        if input.debug_starting {
            self.session = input.processes.clone();
        }
        self.clear_takeover_candidate();
        self.phase = HostedPhase::AttachPending;
        self.outcome(HostedAction::WaitForDebug)
    }
}

pub fn candidate_still_present(candidate: &[ProcessKey], current: &[ProcessKey]) -> bool {
    !candidate.is_empty() && !current.is_empty() && overlaps(current, candidate)
}

fn overlaps(left: &[ProcessKey], right: &[ProcessKey]) -> bool {
    left.iter().any(|process| right.contains(process))
}

pub fn normalize_executable_path(path: &str) -> String {
    let trimmed = path.trim();
    let stripped = trimmed
        .strip_prefix(r"\\?\")
        .or_else(|| trimmed.strip_prefix(r"//?/"))
        .unwrap_or(trimmed);
    Path::new(stripped)
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

pub fn remote_debugging_ports(command_line: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    let marker = "--remote-debugging-port=";
    let mut rest = command_line;
    while let Some(index) = rest.find(marker) {
        let after = &rest[index + marker.len()..];
        let digits: String = after
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect();
        if let Ok(port) = digits.parse::<u16>() {
            if port != 0 {
                ports.push(port);
            }
        }
        rest = after;
    }
    ports.sort_unstable();
    ports.dedup();
    ports
}

pub fn has_remote_debugging_arg(command_line: &str) -> bool {
    !remote_debugging_ports(command_line).is_empty()
}

pub fn debug_ports_from_records(records: &[ProcessRecord]) -> Vec<u16> {
    let mut ports = Vec::new();
    for record in records {
        if let Some(command_line) = &record.command_line {
            ports.extend(remote_debugging_ports(command_line));
        }
    }
    ports.sort_unstable();
    ports.dedup();
    ports
}

pub fn snapshot_matching_processes(executable: &str) -> Result<Vec<ProcessKey>, String> {
    Ok(snapshot_matching_records(executable)?
        .into_iter()
        .map(|record| record.key)
        .collect())
}

pub fn snapshot_matching_records(executable: &str) -> Result<Vec<ProcessRecord>, String> {
    snapshot_matching_records_impl(executable)
}

#[cfg(windows)]
fn snapshot_matching_records_impl(executable: &str) -> Result<Vec<ProcessRecord>, String> {
    use std::mem::{size_of, zeroed};

    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    struct SafeHandle(HANDLE);

    impl SafeHandle {
        fn new(handle: HANDLE) -> Option<Self> {
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                None
            } else {
                Some(Self(handle))
            }
        }
    }

    impl Drop for SafeHandle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *const u16,
    }

    #[link(name = "ntdll")]
    extern "system" {
        fn NtQueryInformationProcess(
            process_handle: HANDLE,
            process_information_class: u32,
            process_information: *mut core::ffi::c_void,
            process_information_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }

    fn query_command_line(handle: HANDLE) -> Option<String> {
        const PROCESS_COMMAND_LINE_INFORMATION: u32 = 60;
        unsafe {
            let mut buffer = vec![0u8; 4096];
            let mut return_length = 0u32;
            let mut status = NtQueryInformationProcess(
                handle,
                PROCESS_COMMAND_LINE_INFORMATION,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut return_length,
            );
            if status != 0 {
                if return_length as usize > buffer.len() && return_length < 64 * 1024 {
                    buffer.resize(return_length as usize, 0);
                    status = NtQueryInformationProcess(
                        handle,
                        PROCESS_COMMAND_LINE_INFORMATION,
                        buffer.as_mut_ptr().cast(),
                        buffer.len() as u32,
                        &mut return_length,
                    );
                }
                if status != 0 {
                    return None;
                }
            }
            if buffer.len() < size_of::<UnicodeString>() {
                return None;
            }
            let unicode = &*(buffer.as_ptr() as *const UnicodeString);
            if unicode.buffer.is_null() || unicode.length == 0 {
                return None;
            }
            let chars = usize::from(unicode.length) / 2;
            let start = buffer.as_ptr() as usize;
            let end = start + buffer.len();
            let pointer = unicode.buffer as usize;
            if pointer < start || pointer.saturating_add(chars.saturating_mul(2)) > end {
                return None;
            }
            Some(String::from_utf16_lossy(std::slice::from_raw_parts(
                unicode.buffer,
                chars,
            )))
        }
    }

    fn query_record(pid: u32, target: &str) -> Option<ProcessRecord> {
        if pid == 0 {
            return None;
        }
        unsafe {
            let handle = SafeHandle::new(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid))?;
            let mut image = [0u16; 1024];
            let mut size = image.len() as u32;
            if QueryFullProcessImageNameW(handle.0, 0, image.as_mut_ptr(), &mut size) == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&image[..size as usize]);
            if normalize_executable_path(&path) != target {
                return None;
            }
            let mut creation = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut exit_time = creation;
            let mut kernel = creation;
            let mut user = creation;
            if GetProcessTimes(
                handle.0,
                &mut creation,
                &mut exit_time,
                &mut kernel,
                &mut user,
            ) == 0
            {
                return None;
            }
            let created_at =
                (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
            Some(ProcessRecord {
                key: ProcessKey { pid, created_at },
                command_line: query_command_line(handle.0),
            })
        }
    }

    let target = normalize_executable_path(executable);
    let file_name = Path::new(&target)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if target.is_empty() || file_name.is_empty() {
        return Err("目标可执行路径无效。".to_string());
    }

    let snapshot = SafeHandle::new(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) })
        .ok_or_else(|| "无法创建进程快照。".to_string())?;
    let mut entry = unsafe { zeroed::<PROCESSENTRY32W>() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut records = Vec::new();
    let mut has_entry = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
    while has_entry {
        let exe_name = wide_zstring(&entry.szExeFile).to_ascii_lowercase();
        if exe_name == file_name {
            if let Some(record) = query_record(entry.th32ProcessID, &target) {
                records.push(record);
            }
        }
        has_entry = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
    }
    Ok(records)
}

#[cfg(windows)]
fn wide_zstring(value: &[u16]) -> String {
    let end = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}

#[cfg(not(windows))]
fn snapshot_matching_records_impl(_executable: &str) -> Result<Vec<ProcessRecord>, String> {
    Err("进程快照仅支持 Windows。".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(pid: u32, created_at: u64) -> ProcessKey {
        ProcessKey { pid, created_at }
    }

    fn input(processes: &[ProcessKey]) -> HostedInput {
        HostedInput::new(processes.to_vec())
    }

    fn input_at(processes: &[ProcessKey], now: Instant) -> HostedInput {
        let mut value = HostedInput::new(processes.to_vec());
        value.now = now;
        value
    }

    #[test]
    fn waits_when_no_process_exists() {
        let mut machine = HostedMachine::new().with_confirmations(1);
        assert_eq!(machine.decide(&input(&[])).action, HostedAction::Wait);
        assert_eq!(machine.phase(), HostedPhase::Waiting);
    }

    #[test]
    fn does_not_kill_processes_that_existed_before_arm() {
        let preexisting = key(11, 100);
        let child = key(12, 110);
        let mut machine = HostedMachine::new().with_confirmations(1);
        machine.arm(&[preexisting]);

        assert_eq!(
            machine.decide(&input(&[preexisting])).action,
            HostedAction::ReportExistingUnmanaged
        );
        assert_eq!(
            machine.decide(&input(&[preexisting, child])).action,
            HostedAction::ReportExistingUnmanaged
        );
        assert_eq!(machine.phase(), HostedPhase::ExistingUnmanaged);
    }

    #[test]
    fn takes_over_after_preexisting_exits_and_new_process_starts() {
        let old = key(21, 200);
        let next = key(22, 300);
        let mut machine = HostedMachine::new().with_confirmations(1);
        machine.arm(&[old]);
        assert_eq!(
            machine.decide(&input(&[old])).action,
            HostedAction::ReportExistingUnmanaged
        );
        assert_eq!(machine.decide(&input(&[])).action, HostedAction::Wait);
        assert_eq!(
            machine.decide(&input(&[next])).action,
            HostedAction::Takeover
        );
        assert_eq!(machine.phase(), HostedPhase::Takeover);
    }

    #[test]
    fn waits_for_debug_instead_of_attaching_or_killing() {
        let process = key(31, 400);
        let now = Instant::now();
        let mut starting = HostedMachine::new().with_confirmations(1);
        starting.arm(&[]);
        let mut launching = input_at(&[process], now);
        launching.debug_starting = true;
        assert_eq!(
            starting.decide(&launching).action,
            HostedAction::WaitForDebug
        );
        assert_eq!(starting.phase(), HostedPhase::AttachPending);

        let mut ready = HostedMachine::new().with_confirmations(1);
        ready.arm(&[process]);
        let mut live = input_at(&[process], now);
        live.has_ready_debug_session = true;
        assert_eq!(ready.decide(&live).action, HostedAction::Attach);
        assert_eq!(ready.phase(), HostedPhase::AttachPending);
    }

    #[test]
    fn debug_wait_times_out_without_killing_and_rearms_after_exit() {
        let process = key(32, 410);
        let next = key(33, 520);
        let now = Instant::now();
        let mut machine = HostedMachine::new().with_confirmations(1);
        machine.arm(&[]);

        let mut launching = input_at(&[process], now);
        launching.debug_starting = true;
        assert_eq!(
            machine.decide(&launching).action,
            HostedAction::WaitForDebug
        );

        let mut still_waiting = input_at(&[process], now + Duration::from_secs(44));
        still_waiting.debug_starting = true;
        assert_eq!(
            machine.decide(&still_waiting).action,
            HostedAction::WaitForDebug
        );

        let mut timed_out = input_at(&[process], now + Duration::from_secs(45));
        timed_out.debug_starting = true;
        assert_eq!(
            machine.decide(&timed_out).action,
            HostedAction::ReportDebugTimeout
        );
        assert_eq!(machine.phase(), HostedPhase::ExistingUnmanaged);

        let mut still_timed_out = input_at(&[process], now + Duration::from_secs(50));
        still_timed_out.debug_starting = true;
        assert_eq!(
            machine.decide(&still_timed_out).action,
            HostedAction::ReportDebugTimeout
        );
        assert_ne!(
            machine.decide(&still_timed_out).action,
            HostedAction::Takeover
        );

        assert_eq!(
            machine
                .decide(&input_at(&[], now + Duration::from_secs(51)))
                .action,
            HostedAction::CleanupDisconnected
        );
        assert_eq!(machine.phase(), HostedPhase::Waiting);
        assert_eq!(
            machine
                .decide(&input_at(&[next], now + Duration::from_secs(52)))
                .action,
            HostedAction::Takeover
        );
    }

    #[test]
    fn pending_command_line_does_not_look_like_a_plain_launch() {
        let process = key(34, 430);
        let mut machine = HostedMachine::new().with_confirmations(1);
        machine.arm(&[]);
        let mut pending = input(&[process]);
        pending.cmdline_pending = true;
        assert_eq!(machine.decide(&pending).action, HostedAction::WaitForDebug);
        assert_ne!(machine.decide(&pending).action, HostedAction::Takeover);
    }

    #[test]
    fn rearms_after_target_exits() {
        let first = key(41, 500);
        let second = key(42, 600);
        let mut machine = HostedMachine::new().with_confirmations(1);
        machine.rearm_after_apply(&[first]);
        let mut connected = input(&[first]);
        connected.connected = true;
        assert_eq!(machine.decide(&connected).action, HostedAction::KeepActive);
        assert_eq!(
            machine.decide(&input(&[])).action,
            HostedAction::CleanupDisconnected
        );
        assert_eq!(machine.phase(), HostedPhase::Waiting);
        assert_eq!(
            machine.decide(&input(&[second])).action,
            HostedAction::Takeover
        );
    }

    #[test]
    fn stays_suspended_and_does_not_take_over() {
        let process = key(51, 700);
        let mut machine = HostedMachine::new().with_confirmations(1);
        machine.arm(&[]);
        machine.suspend();
        assert_eq!(
            machine.decide(&input(&[process])).action,
            HostedAction::StaySuspended
        );
        assert_eq!(machine.phase(), HostedPhase::Suspended);
        assert!(machine.is_paused());
    }

    #[test]
    fn electron_child_processes_do_not_retrigger_takeover() {
        let main = key(61, 800);
        let renderer = key(62, 810);
        let gpu = key(63, 820);
        let mut unmanaged = HostedMachine::new().with_confirmations(1);
        unmanaged.arm(&[main]);
        assert_eq!(
            unmanaged.decide(&input(&[main, renderer, gpu])).action,
            HostedAction::ReportExistingUnmanaged
        );

        let mut managed = HostedMachine::new().with_confirmations(1);
        managed.rearm_after_apply(&[main]);
        let mut connected = input(&[main, renderer, gpu]);
        connected.connected = true;
        assert_eq!(managed.decide(&connected).action, HostedAction::KeepActive);
        assert_eq!(
            managed.decide(&input(&[main, renderer, gpu])).action,
            HostedAction::ReportExistingUnmanaged
        );
    }

    #[test]
    fn requires_consecutive_observations_before_takeover() {
        let process = key(71, 900);
        let mut machine = HostedMachine::new();
        machine.arm(&[]);
        let waiting = machine.decide(&input(&[process]));
        assert_eq!(waiting.action, HostedAction::Wait);
        assert!(waiting.candidate.is_empty());
        let confirmed = machine.decide(&input(&[process]));
        assert_eq!(confirmed.action, HostedAction::Takeover);
        assert_eq!(confirmed.candidate, vec![process]);
    }

    #[test]
    fn failed_generation_is_not_auto_killed() {
        let process = key(72, 910);
        let mut machine = HostedMachine::new().with_confirmations(1);
        machine.arm(&[]);
        machine.note_takeover_failed(&[process]);
        assert_eq!(
            machine.decide(&input(&[process])).action,
            HostedAction::ReportExistingUnmanaged
        );
        assert_ne!(
            machine.decide(&input(&[process])).action,
            HostedAction::Takeover
        );
    }

    #[test]
    fn manual_apply_rearms_from_suspended() {
        let process = key(81, 1000);
        let mut machine = HostedMachine::new().with_confirmations(1);
        machine.suspend();
        machine.rearm_after_apply(&[process]);
        assert!(!machine.is_paused());
        let mut connected = input(&[process]);
        connected.connected = true;
        assert_eq!(machine.decide(&connected).action, HostedAction::KeepActive);
    }

    #[test]
    fn generation_change_after_confirmed_takeover_does_not_inherit_decision() {
        let old = key(91, 1100);
        let next = key(92, 1200);
        let mut machine = HostedMachine::new();
        machine.arm(&[]);
        assert_eq!(machine.decide(&input(&[old])).action, HostedAction::Wait);
        let confirmed = machine.decide(&input(&[old]));
        assert_eq!(confirmed.action, HostedAction::Takeover);
        assert_eq!(confirmed.candidate, vec![old]);

        let replaced = machine.decide(&input(&[next]));
        assert_eq!(replaced.action, HostedAction::Wait);
        assert!(replaced.candidate.is_empty());

        assert_eq!(machine.decide(&input(&[next])).action, HostedAction::Wait);
        let next_confirmed = machine.decide(&input(&[next]));
        assert_eq!(next_confirmed.action, HostedAction::Takeover);
        assert_eq!(next_confirmed.candidate, vec![next]);
    }

    #[test]
    fn same_generation_with_child_still_takes_over() {
        let main = key(93, 1300);
        let child = key(94, 1310);
        let mut machine = HostedMachine::new();
        machine.arm(&[]);
        assert_eq!(machine.decide(&input(&[main])).action, HostedAction::Wait);
        let confirmed = machine.decide(&input(&[main]));
        assert_eq!(confirmed.action, HostedAction::Takeover);
        assert_eq!(confirmed.candidate, vec![main]);

        let with_child = machine.decide(&input(&[main, child]));
        assert_eq!(with_child.action, HostedAction::Takeover);
        assert_eq!(with_child.candidate, vec![main]);
        assert!(candidate_still_present(
            &with_child.candidate,
            &[main, child]
        ));
    }

    #[test]
    fn empty_after_confirmed_takeover_cancels_and_never_takes_over() {
        let process = key(95, 1400);
        let mut machine = HostedMachine::new();
        machine.arm(&[]);
        assert_eq!(
            machine.decide(&input(&[process])).action,
            HostedAction::Wait
        );
        assert_eq!(
            machine.decide(&input(&[process])).action,
            HostedAction::Takeover
        );

        let empty = machine.decide(&input(&[]));
        assert_eq!(empty.action, HostedAction::CleanupDisconnected);
        assert!(empty.candidate.is_empty());
        assert_ne!(empty.action, HostedAction::Takeover);
    }

    #[test]
    fn attach_requires_live_debug_session_on_reprobe() {
        let process = key(96, 1500);
        let mut machine = HostedMachine::new();
        machine.arm(&[]);
        let mut live = input(&[process]);
        live.has_ready_debug_session = true;
        assert_eq!(machine.decide(&live).action, HostedAction::Attach);

        let lost = machine.decide(&input(&[process]));
        assert_ne!(lost.action, HostedAction::Attach);
        assert_ne!(lost.action, HostedAction::Takeover);
        assert!(lost.candidate.is_empty());
    }

    #[test]
    fn manual_apply_during_confirmed_takeover_keeps_active() {
        let process = key(97, 1600);
        let mut machine = HostedMachine::new();
        machine.arm(&[]);
        assert_eq!(
            machine.decide(&input(&[process])).action,
            HostedAction::Wait
        );
        assert_eq!(
            machine.decide(&input(&[process])).action,
            HostedAction::Takeover
        );

        let mut connected = input(&[process]);
        connected.connected = true;
        let decision = machine.decide(&connected);
        assert_eq!(decision.action, HostedAction::KeepActive);
        assert!(decision.candidate.is_empty());
    }

    #[test]
    fn parses_remote_debugging_ports_from_command_line() {
        assert_eq!(
            remote_debugging_ports(
                r#""ChatGPT.exe" --remote-debugging-address=127.0.0.1 --remote-debugging-port=9335"#
            ),
            vec![9335]
        );
        assert!(has_remote_debugging_arg(
            "--remote-debugging-port=9335 --remote-debugging-address=127.0.0.1"
        ));
        assert!(!has_remote_debugging_arg("ChatGPT.exe --flag"));
    }

    #[test]
    fn normalizes_full_executable_paths() {
        assert_eq!(
            normalize_executable_path(r"C:\Program Files\WindowsApps\OpenAI.Codex\app\ChatGPT.exe"),
            r"c:\program files\windowsapps\openai.codex\app\chatgpt.exe"
        );
        assert_eq!(
            normalize_executable_path(
                r"\\?\C:/Program Files/WindowsApps/OpenAI.Codex/app/ChatGPT.exe"
            ),
            r"c:\program files\windowsapps\openai.codex\app\chatgpt.exe"
        );
    }

    #[cfg(windows)]
    #[test]
    fn snapshots_current_process_by_full_path() {
        let exe = std::env::current_exe().expect("current test executable");
        let records = snapshot_matching_records(&exe.to_string_lossy()).expect("native snapshot");
        let pid = std::process::id();
        assert!(
            records.iter().any(|item| item.key.pid == pid),
            "snapshot should include the current test process"
        );
        assert!(records.iter().all(|item| item.key.created_at > 0));
        let self_record = records
            .iter()
            .find(|item| item.key.pid == pid)
            .expect("current process record");
        assert!(self_record
            .command_line
            .as_deref()
            .is_some_and(|line| !line.is_empty()));
    }
}
