//! GDB Driver - Drive GDB via Machine Interface (MI) for correctness comparison
//!
//! Provides a GDB/MI interface driver for E2E tests to compare kdb behavior
//! against GDB as the reference implementation.
//!
//! # ASSUM Safety
//!
//! - #ASSUME_GDB_AVAILABLE: GDB must be installed and in PATH
//! - #ASSUME_MI_VERSION: GDB/MI version 2 (GDB 6.0+) compatibility
//! - #ASSUME_STDOUT_READABLE: GDB process stdout/stdin are piped

use super::error::{E2EError, E2EResult};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Token counter for GDB/MI async commands
static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

/// GDB/MI response types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GdbMiResponse {
    /// Result record (^done, ^running, ^connected, ^error, ^exit)
    Result {
        token: Option<u64>,
        class: String,
        data: HashMap<String, String>,
    },
    /// Exec async record (*stopped, *running)
    Exec {
        class: String,
        data: HashMap<String, String>,
    },
    /// Status async record (+download, etc.)
    Status {
        class: String,
        data: HashMap<String, String>,
    },
    /// Notify async record (=thread-group-added, etc.)
    Notify {
        class: String,
        data: HashMap<String, String>,
    },
    /// Console stream output (~"text")
    Console(String),
    /// Target stream output (@"text")
    Target(String),
    /// Log stream output (&"text")
    Log(String),
    /// GDB prompt (gdb)
    Prompt,
    /// Unknown or unparseable response
    Unknown(String),
}

/// GDB stop reason (from *stopped async record)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GdbStopReason {
    /// Hit a breakpoint
    BreakpointHit { number: u32, address: u64 },
    /// End stepping range (step completed)
    EndSteppingRange { address: u64 },
    /// Signal received
    SignalReceived { name: String, meaning: String },
    /// Function finished (finish command)
    FunctionFinished,
    /// Exited normally
    ExitedNormally,
    /// Exited with status
    Exited { status: i32 },
    /// Unknown reason
    Unknown(String),
}

/// GDB register values
#[derive(Debug, Clone, Default)]
pub struct GdbRegisters {
    /// Register name to value mapping
    pub values: HashMap<String, u64>,
    /// Common registers (convenience)
    pub rip: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub rax: u64,
}

/// GDB stack frame
#[derive(Debug, Clone)]
pub struct GdbStackFrame {
    /// Frame level (0 = current)
    pub level: u32,
    /// Address
    pub addr: u64,
    /// Function name
    pub func: Option<String>,
    /// Source file
    pub file: Option<String>,
    /// Line number
    pub line: Option<u32>,
}

/// Drive GDB via Machine Interface (MI)
///
/// This driver spawns a GDB process in MI mode and communicates via
/// stdin/stdout using the GDB/MI protocol.
///
/// # Example
///
/// ```ignore
/// let mut gdb = GdbDriver::new()?;
/// gdb.attach(pid)?;
/// let reason = gdb.continue_until_stop()?;
/// let regs = gdb.get_registers()?;
/// gdb.quit()?;
/// ```
pub struct GdbDriver {
    /// GDB child process
    process: Option<Child>,
    /// Buffered reader for GDB stdout
    reader: Option<BufReader<std::process::ChildStdout>>,
    /// Collected responses during last command
    responses: Vec<GdbMiResponse>,
    /// Current attached PID (if any)
    attached_pid: Option<u32>,
    /// Default timeout for commands
    default_timeout: Duration,
}

impl GdbDriver {
    /// Create a new GDB driver
    ///
    /// Spawns GDB in MI mode (--interpreter=mi2).
    ///
    /// # Errors
    ///
    /// - `SpawnFailed` if GDB cannot be started
    pub fn new() -> E2EResult<Self> {
        Self::with_gdb_path("gdb")
    }

    /// Create a GDB driver with a custom GDB path
    pub fn with_gdb_path(gdb_path: &str) -> E2EResult<Self> {
        let mut process = Command::new(gdb_path)
            .arg("--interpreter=mi2")
            .arg("--quiet")
            .arg("--nx") // Don't read .gdbinit
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| E2EError::spawn_failed(gdb_path, e))?;

        let stdout = process.stdout.take().expect("stdout should be piped");
        let reader = BufReader::new(stdout);

        let mut driver = Self {
            process: Some(process),
            reader: Some(reader),
            responses: Vec::new(),
            attached_pid: None,
            default_timeout: Duration::from_secs(10),
        };

        // Wait for initial prompt
        driver.wait_for_prompt(Duration::from_secs(5))?;

        Ok(driver)
    }

    /// Set default timeout for commands
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.default_timeout = timeout;
    }

    /// Check if GDB is attached to a process
    pub fn is_attached(&self) -> bool {
        self.attached_pid.is_some()
    }

    /// Get the attached PID
    pub fn attached_pid(&self) -> Option<u32> {
        self.attached_pid
    }

    /// Get responses from last command
    pub fn last_responses(&self) -> &[GdbMiResponse] {
        &self.responses
    }

    /// Send a raw MI command and wait for response
    ///
    /// # Arguments
    ///
    /// * `command` - The MI command (without token or newline)
    ///
    /// # Returns
    ///
    /// The result response from GDB
    pub fn send_command(&mut self, command: &str) -> E2EResult<GdbMiResponse> {
        self.send_command_timeout(command, self.default_timeout)
    }

    /// Send a raw MI command with custom timeout
    pub fn send_command_timeout(
        &mut self,
        command: &str,
        timeout: Duration,
    ) -> E2EResult<GdbMiResponse> {
        let token = TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);

        // Get stdin handle
        let stdin = self
            .process
            .as_mut()
            .and_then(|p| p.stdin.as_mut())
            .ok_or_else(|| E2EError::GdbCommunicationError {
                reason: "GDB stdin not available".to_string(),
            })?;

        // Write command with token
        let full_command = format!("{}{}\n", token, command);
        stdin
            .write_all(full_command.as_bytes())
            .map_err(|e| E2EError::GdbCommunicationError {
                reason: format!("Failed to write to GDB: {}", e),
            })?;
        stdin.flush().map_err(|e| E2EError::GdbCommunicationError {
            reason: format!("Failed to flush GDB stdin: {}", e),
        })?;

        // Collect responses until we get a result with our token
        self.responses.clear();
        let result = self.wait_for_result(Some(token), timeout)?;

        Ok(result)
    }

    /// Wait for a result response
    fn wait_for_result(
        &mut self,
        expected_token: Option<u64>,
        timeout: Duration,
    ) -> E2EResult<GdbMiResponse> {
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(E2EError::DebuggerTimeout {
                    timeout_ms: timeout.as_millis() as u64,
                });
            }

            let response = self.read_response()?;

            match &response {
                GdbMiResponse::Result { token, .. } => {
                    if expected_token.is_none() || *token == expected_token {
                        self.responses.push(response.clone());
                        return Ok(response);
                    }
                }
                GdbMiResponse::Prompt => {
                    // If we hit prompt without a result, something's wrong
                    if expected_token.is_some() {
                        return Err(E2EError::GdbCommunicationError {
                            reason: "Got prompt before result".to_string(),
                        });
                    }
                    return Ok(response);
                }
                _ => {
                    self.responses.push(response);
                }
            }
        }
    }

    /// Wait for the GDB prompt
    fn wait_for_prompt(&mut self, timeout: Duration) -> E2EResult<()> {
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(E2EError::DebuggerTimeout {
                    timeout_ms: timeout.as_millis() as u64,
                });
            }

            let response = self.read_response()?;
            if matches!(response, GdbMiResponse::Prompt) {
                return Ok(());
            }
        }
    }

    /// Read a single response line from GDB
    fn read_response(&mut self) -> E2EResult<GdbMiResponse> {
        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| E2EError::GdbCommunicationError {
                reason: "GDB stdout not available".to_string(),
            })?;

        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| E2EError::GdbCommunicationError {
                reason: format!("Failed to read from GDB: {}", e),
            })?;

        Self::parse_mi_response(&line)
    }

    /// Parse a GDB/MI response line
    fn parse_mi_response(line: &str) -> E2EResult<GdbMiResponse> {
        let line = line.trim();

        if line.is_empty() {
            return Ok(GdbMiResponse::Unknown(String::new()));
        }

        // Check for prompt
        if line == "(gdb)" || line == "(gdb) " {
            return Ok(GdbMiResponse::Prompt);
        }

        // Parse based on first character
        let first_char = line.chars().next().unwrap_or(' ');

        match first_char {
            // Result record: [token]^class,data
            '^' | '0'..='9' => Self::parse_result_record(line),
            // Exec async: *class,data
            '*' => Self::parse_async_record(line, "exec"),
            // Status async: +class,data
            '+' => Self::parse_async_record(line, "status"),
            // Notify async: =class,data
            '=' => Self::parse_async_record(line, "notify"),
            // Console stream: ~"text"
            '~' => Ok(GdbMiResponse::Console(Self::parse_stream_text(&line[1..]))),
            // Target stream: @"text"
            '@' => Ok(GdbMiResponse::Target(Self::parse_stream_text(&line[1..]))),
            // Log stream: &"text"
            '&' => Ok(GdbMiResponse::Log(Self::parse_stream_text(&line[1..]))),
            // Unknown
            _ => Ok(GdbMiResponse::Unknown(line.to_string())),
        }
    }

    /// Parse a result record
    fn parse_result_record(line: &str) -> E2EResult<GdbMiResponse> {
        // Format: [token]^class[,data]
        let mut token = None;
        let mut rest = line;

        // Extract token if present (digits before ^)
        if let Some(caret_pos) = line.find('^') {
            if caret_pos > 0 {
                if let Ok(t) = line[..caret_pos].parse::<u64>() {
                    token = Some(t);
                }
            }
            rest = &line[caret_pos + 1..];
        }

        // Split class and data
        let (class, data_str) = if let Some(comma_pos) = rest.find(',') {
            (&rest[..comma_pos], Some(&rest[comma_pos + 1..]))
        } else {
            (rest, None)
        };

        let data = data_str
            .map(Self::parse_mi_data)
            .unwrap_or_default();

        Ok(GdbMiResponse::Result {
            token,
            class: class.to_string(),
            data,
        })
    }

    /// Parse an async record
    fn parse_async_record(line: &str, record_type: &str) -> E2EResult<GdbMiResponse> {
        // Format: [*+=]class[,data]
        let rest = &line[1..];

        let (class, data_str) = if let Some(comma_pos) = rest.find(',') {
            (&rest[..comma_pos], Some(&rest[comma_pos + 1..]))
        } else {
            (rest, None)
        };

        let data = data_str
            .map(Self::parse_mi_data)
            .unwrap_or_default();

        match record_type {
            "exec" => Ok(GdbMiResponse::Exec {
                class: class.to_string(),
                data,
            }),
            "status" => Ok(GdbMiResponse::Status {
                class: class.to_string(),
                data,
            }),
            "notify" => Ok(GdbMiResponse::Notify {
                class: class.to_string(),
                data,
            }),
            _ => Ok(GdbMiResponse::Unknown(line.to_string())),
        }
    }

    /// Parse MI data (simplified key=value parsing)
    fn parse_mi_data(data: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();

        // Simple key="value" or key=value parsing
        // This is a simplified parser; full MI parsing is more complex
        for part in data.split(',') {
            if let Some(eq_pos) = part.find('=') {
                let key = part[..eq_pos].trim();
                let mut value = part[eq_pos + 1..].trim();

                // Remove quotes if present
                if value.starts_with('"') && value.ends_with('"') {
                    value = &value[1..value.len() - 1];
                }

                result.insert(key.to_string(), value.to_string());
            }
        }

        result
    }

    /// Parse stream text (remove surrounding quotes)
    fn parse_stream_text(text: &str) -> String {
        let text = text.trim();
        if text.starts_with('"') && text.ends_with('"') {
            // Unescape common escape sequences
            text[1..text.len() - 1]
                .replace("\\n", "\n")
                .replace("\\t", "\t")
                .replace("\\\"", "\"")
                .replace("\\\\", "\\")
        } else {
            text.to_string()
        }
    }

    /// Attach to a process
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to attach to
    ///
    /// # Errors
    ///
    /// - `AttachFailed` if attachment fails
    pub fn attach(&mut self, pid: u32) -> E2EResult<()> {
        let response = self.send_command(&format!("-target-attach {}", pid))?;

        match response {
            GdbMiResponse::Result { class, .. } if class == "done" => {
                self.attached_pid = Some(pid);
                Ok(())
            }
            GdbMiResponse::Result { class, data, .. } if class == "error" => {
                Err(E2EError::attach_failed(
                    pid,
                    data.get("msg").cloned().unwrap_or_else(|| "Unknown error".to_string()),
                ))
            }
            _ => Err(E2EError::attach_failed(pid, "Unexpected response")),
        }
    }

    /// Detach from the current process
    pub fn detach(&mut self) -> E2EResult<()> {
        let pid = self.attached_pid.ok_or(E2EError::NotAttached)?;

        let response = self.send_command("-target-detach")?;

        match response {
            GdbMiResponse::Result { class, .. } if class == "done" => {
                self.attached_pid = None;
                Ok(())
            }
            GdbMiResponse::Result { class, data, .. } if class == "error" => Err(
                E2EError::DetachFailed {
                    pid,
                    reason: data.get("msg").cloned().unwrap_or_else(|| "Unknown error".to_string()),
                },
            ),
            _ => Err(E2EError::DetachFailed {
                pid,
                reason: "Unexpected response".to_string(),
            }),
        }
    }

    /// Set a breakpoint at an address
    ///
    /// # Arguments
    ///
    /// * `location` - Address (e.g., "*0x400000") or symbol name
    ///
    /// # Returns
    ///
    /// Breakpoint number on success
    pub fn set_breakpoint(&mut self, location: &str) -> E2EResult<u32> {
        let response = self.send_command(&format!("-break-insert {}", location))?;

        match response {
            GdbMiResponse::Result { class, data, .. } if class == "done" => {
                // Parse breakpoint number from bkpt={number="N",...}
                // Simplified: look for number= in data
                data.get("number")
                    .and_then(|n| n.parse().ok())
                    .ok_or_else(|| E2EError::breakpoint_failed(location, "No breakpoint number in response"))
            }
            GdbMiResponse::Result { class, data, .. } if class == "error" => {
                Err(E2EError::breakpoint_failed(
                    location,
                    data.get("msg").cloned().unwrap_or_else(|| "Unknown error".to_string()),
                ))
            }
            _ => Err(E2EError::breakpoint_failed(location, "Unexpected response")),
        }
    }

    /// Continue execution and wait for stop
    ///
    /// # Returns
    ///
    /// The reason why execution stopped
    pub fn continue_until_stop(&mut self) -> E2EResult<GdbStopReason> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        let response = self.send_command("-exec-continue")?;

        match response {
            GdbMiResponse::Result { class, .. } if class == "running" => {
                // Wait for *stopped async record
                self.wait_for_stop()
            }
            GdbMiResponse::Result { class, data, .. } if class == "error" => Err(E2EError::generic(
                "continue",
                data.get("msg").cloned().unwrap_or_else(|| "Unknown error".to_string()),
            )),
            _ => Err(E2EError::generic("continue", "Unexpected response")),
        }
    }

    /// Wait for a stop event
    fn wait_for_stop(&mut self) -> E2EResult<GdbStopReason> {
        let start = Instant::now();

        loop {
            if start.elapsed() > self.default_timeout {
                return Err(E2EError::DebuggerTimeout {
                    timeout_ms: self.default_timeout.as_millis() as u64,
                });
            }

            let response = self.read_response()?;

            if let GdbMiResponse::Exec { class, data } = response {
                if class == "stopped" {
                    return Self::parse_stop_reason(&data);
                }
            }
        }
    }

    /// Parse stop reason from *stopped data
    fn parse_stop_reason(data: &HashMap<String, String>) -> E2EResult<GdbStopReason> {
        let reason = data.get("reason").map(|s| s.as_str()).unwrap_or("unknown");

        match reason {
            "breakpoint-hit" => {
                let number = data
                    .get("bkptno")
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                let address = data
                    .get("addr")
                    .and_then(|a| u64::from_str_radix(a.trim_start_matches("0x"), 16).ok())
                    .unwrap_or(0);
                Ok(GdbStopReason::BreakpointHit { number, address })
            }
            "end-stepping-range" => {
                let address = data
                    .get("addr")
                    .and_then(|a| u64::from_str_radix(a.trim_start_matches("0x"), 16).ok())
                    .unwrap_or(0);
                Ok(GdbStopReason::EndSteppingRange { address })
            }
            "signal-received" => {
                let name = data.get("signal-name").cloned().unwrap_or_default();
                let meaning = data.get("signal-meaning").cloned().unwrap_or_default();
                Ok(GdbStopReason::SignalReceived { name, meaning })
            }
            "function-finished" => Ok(GdbStopReason::FunctionFinished),
            "exited-normally" => Ok(GdbStopReason::ExitedNormally),
            "exited" => {
                let status = data
                    .get("exit-code")
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(-1);
                Ok(GdbStopReason::Exited { status })
            }
            _ => Ok(GdbStopReason::Unknown(reason.to_string())),
        }
    }

    /// Single-step one instruction
    pub fn step(&mut self) -> E2EResult<GdbStopReason> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        let response = self.send_command("-exec-step-instruction")?;

        match response {
            GdbMiResponse::Result { class, .. } if class == "running" => self.wait_for_stop(),
            GdbMiResponse::Result { class, data, .. } if class == "error" => Err(E2EError::StepFailed {
                reason: data.get("msg").cloned().unwrap_or_else(|| "Unknown error".to_string()),
            }),
            _ => Err(E2EError::StepFailed {
                reason: "Unexpected response".to_string(),
            }),
        }
    }

    /// Get current registers
    pub fn get_registers(&mut self) -> E2EResult<GdbRegisters> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        let response = self.send_command("-data-list-register-values x")?;

        match response {
            GdbMiResponse::Result { class, data, .. } if class == "done" => {
                // Parse register values
                // Simplified: actual parsing would handle register-values=[{...},...]
                let mut regs = GdbRegisters::default();

                // Get common registers by name
                for (name, value) in &data {
                    if let Ok(v) = u64::from_str_radix(value.trim_start_matches("0x"), 16) {
                        regs.values.insert(name.clone(), v);
                        match name.as_str() {
                            "rip" | "pc" => regs.rip = v,
                            "rsp" | "sp" => regs.rsp = v,
                            "rbp" | "fp" => regs.rbp = v,
                            "rax" => regs.rax = v,
                            _ => {}
                        }
                    }
                }

                Ok(regs)
            }
            GdbMiResponse::Result { class, data, .. } if class == "error" => {
                Err(E2EError::RegisterReadFailed {
                    reason: data.get("msg").cloned().unwrap_or_else(|| "Unknown error".to_string()),
                })
            }
            _ => Err(E2EError::RegisterReadFailed {
                reason: "Unexpected response".to_string(),
            }),
        }
    }

    /// Get stack trace
    ///
    /// # Arguments
    ///
    /// * `max_frames` - Maximum number of frames to retrieve
    pub fn get_stack_trace(&mut self, max_frames: u32) -> E2EResult<Vec<GdbStackFrame>> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        let response = self.send_command(&format!("-stack-list-frames 0 {}", max_frames - 1))?;

        match response {
            GdbMiResponse::Result { class, data, .. } if class == "done" => {
                // Parse stack frames
                // Simplified: actual parsing would handle stack=[frame={...},...]
                let mut frames = Vec::new();

                // For each frame in the response...
                // This is simplified; real implementation would parse the nested structure
                for (key, value) in &data {
                    if key.starts_with("frame") || key == "level" {
                        // Parse individual frame
                        let frame = GdbStackFrame {
                            level: 0,
                            addr: 0,
                            func: Some(value.clone()),
                            file: None,
                            line: None,
                        };
                        frames.push(frame);
                    }
                }

                Ok(frames)
            }
            GdbMiResponse::Result { class, data, .. } if class == "error" => {
                Err(E2EError::StackTraceFailed {
                    reason: data.get("msg").cloned().unwrap_or_else(|| "Unknown error".to_string()),
                })
            }
            _ => Err(E2EError::StackTraceFailed {
                reason: "Unexpected response".to_string(),
            }),
        }
    }

    /// Read memory
    ///
    /// # Arguments
    ///
    /// * `address` - Starting address
    /// * `length` - Number of bytes to read
    pub fn read_memory(&mut self, address: u64, length: usize) -> E2EResult<Vec<u8>> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        let response = self.send_command(&format!(
            "-data-read-memory-bytes 0x{:x} {}",
            address, length
        ))?;

        match response {
            GdbMiResponse::Result { class, data, .. } if class == "done" => {
                // Parse memory contents
                // Format: memory=[{begin="0x...",offset="0",end="0x...",contents="..."}]
                let contents = data.get("contents").cloned().unwrap_or_default();

                // Decode hex string to bytes
                let bytes: Vec<u8> = (0..contents.len())
                    .step_by(2)
                    .filter_map(|i| {
                        if i + 2 <= contents.len() {
                            u8::from_str_radix(&contents[i..i + 2], 16).ok()
                        } else {
                            None
                        }
                    })
                    .collect();

                Ok(bytes)
            }
            GdbMiResponse::Result { class, data, .. } if class == "error" => {
                Err(E2EError::MemoryReadFailed {
                    addr: address,
                    reason: data.get("msg").cloned().unwrap_or_else(|| "Unknown error".to_string()),
                })
            }
            _ => Err(E2EError::MemoryReadFailed {
                addr: address,
                reason: "Unexpected response".to_string(),
            }),
        }
    }

    /// Quit GDB
    pub fn quit(&mut self) -> E2EResult<()> {
        let _ = self.send_command("-gdb-exit");

        // Wait for process to exit
        if let Some(ref mut process) = self.process {
            let _ = process.wait();
        }

        self.process = None;
        self.reader = None;
        self.attached_pid = None;

        Ok(())
    }
}

impl Drop for GdbDriver {
    fn drop(&mut self) {
        // Try to cleanly exit GDB
        if self.process.is_some() {
            let _ = self.quit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_result_record() {
        let response = GdbDriver::parse_mi_response("123^done,value=\"test\"").unwrap();
        match response {
            GdbMiResponse::Result { token, class, data } => {
                assert_eq!(token, Some(123));
                assert_eq!(class, "done");
                assert_eq!(data.get("value"), Some(&"test".to_string()));
            }
            _ => panic!("Expected Result"),
        }
    }

    #[test]
    fn test_parse_exec_record() {
        let response = GdbDriver::parse_mi_response("*stopped,reason=\"breakpoint-hit\"").unwrap();
        match response {
            GdbMiResponse::Exec { class, data } => {
                assert_eq!(class, "stopped");
                assert_eq!(data.get("reason"), Some(&"breakpoint-hit".to_string()));
            }
            _ => panic!("Expected Exec"),
        }
    }

    #[test]
    fn test_parse_console_stream() {
        let response = GdbDriver::parse_mi_response("~\"Hello\\nWorld\"").unwrap();
        match response {
            GdbMiResponse::Console(text) => {
                assert_eq!(text, "Hello\nWorld");
            }
            _ => panic!("Expected Console"),
        }
    }

    #[test]
    fn test_parse_prompt() {
        let response = GdbDriver::parse_mi_response("(gdb)").unwrap();
        assert!(matches!(response, GdbMiResponse::Prompt));
    }

    #[test]
    fn test_parse_stop_reason_breakpoint() {
        let mut data = HashMap::new();
        data.insert("reason".to_string(), "breakpoint-hit".to_string());
        data.insert("bkptno".to_string(), "1".to_string());
        data.insert("addr".to_string(), "0x400000".to_string());

        let reason = GdbDriver::parse_stop_reason(&data).unwrap();
        match reason {
            GdbStopReason::BreakpointHit { number, address } => {
                assert_eq!(number, 1);
                assert_eq!(address, 0x400000);
            }
            _ => panic!("Expected BreakpointHit"),
        }
    }
}
