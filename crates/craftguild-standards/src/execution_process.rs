//! Cancellable subprocesses with isolated groups and optional caller deadlines.

use craftguild_core::Result;
#[cfg(unix)]
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
#[cfg(unix)]
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
#[cfg(unix)]
use std::time::Instant;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static CANCELLED: AtomicBool = AtomicBool::new(false);
static SIGNALS: OnceLock<std::result::Result<(), String>> = OnceLock::new();

pub fn initialize_signals() -> Result<()> {
    SIGNALS
        .get_or_init(|| {
            ctrlc::set_handler(|| CANCELLED.store(true, Ordering::SeqCst))
                .map_err(|e| e.to_string())
        })
        .clone()
        .map_err(Into::into)
}

pub fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[derive(Clone, Debug, Default)]
pub struct Runner {
    pub cleanup_failed: Arc<AtomicBool>,
    pub env: BTreeMap<String, String>,
    pub unset_env: BTreeSet<String>,
    pub toolchain: Option<String>,
    pub compiler_version: Option<String>,
    pub log: Option<PathBuf>,
    pub cancel_file: Option<PathBuf>,
    pub timeout: Option<Duration>,
}

impl Runner {
    pub fn rollback_safe(&self) -> Result<()> {
        if self.cleanup_failed.load(Ordering::SeqCst) {
            return Err(
                "Subprocess termination could not be verified; rollback is deferred".into(),
            );
        }
        Ok(())
    }
    pub fn cancelled(&self) -> bool {
        CANCELLED.load(Ordering::SeqCst) || self.cancel_file.as_ref().is_some_and(|p| p.exists())
    }

    pub fn check(&self) -> Result<()> {
        if self.cancelled() {
            Err("Execution cancelled".into())
        } else {
            Ok(())
        }
    }

    pub fn capture(&self, root: &Path, argv: &[String]) -> Result<Vec<u8>> {
        #[cfg(not(unix))]
        {
            let _ = (root, argv);
            Err("Owned subprocess execution requires Unix process groups on this engine".into())
        }
        #[cfg(unix)]
        {
            if cfg!(any(
                target_os = "cygwin",
                target_os = "horizon",
                target_os = "openbsd",
                target_os = "redox",
                target_os = "wasi"
            )) {
                return Err(
                    "Owned subprocess execution requires non-reaping exit observation".into(),
                );
            }
            self.check()?;
            let mut effective = argv.to_vec();
            if matches!(argv.first().map(String::as_str), Some("cargo" | "rustc")) {
                if let Some(toolchain) = &self.toolchain {
                    effective.splice(0..0, ["rustup".into(), "run".into(), toolchain.clone()]);
                }
            }
            let (program, args) = effective.split_first().ok_or("Empty required command")?;
            let mut stdout = tempfile::tempfile()?;
            let mut stderr = tempfile::tempfile()?;
            let mut command = Command::new(program);
            command
                .args(args)
                .current_dir(root)
                .envs(&self.env)
                .stdin(Stdio::null())
                .stdout(stdout.try_clone()?)
                .stderr(stderr.try_clone()?);
            for name in &self.unset_env {
                command.env_remove(name);
            }
            // A rustup proxy must never acquire a missing toolchain during execution.
            command.env("RUSTUP_AUTO_INSTALL", "0");
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                command.process_group(0);
            }
            self.event(json!({"argv":effective,"state":"running","at":now()}))?;
            let active = self.log.as_ref().map(|p| p.with_extension("active"));
            if let Some(path) = &active {
                // A crash between spawn and PID recording must defer recovery.
                std::fs::write(path, b"pending")?;
            }
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(error) => {
                    if let Some(path) = &active {
                        std::fs::remove_file(path)?;
                    }
                    return Err(format!("Cannot execute required tool {program}: {error}").into());
                }
            };
            let waited = (|| -> Result<()> {
                if let Some(path) = &active {
                    std::fs::write(path, child.id().to_string())?;
                }
                self.event(
                    json!({"argv":effective,"state":"spawned","pid":child.id(),"at":now()}),
                )?;
                let started = Instant::now();
                loop {
                    if self.cancelled()
                        || self.timeout.is_some_and(|limit| started.elapsed() >= limit)
                    {
                        return Err(if self.cancelled() {
                            "Execution cancelled"
                        } else {
                            "Command timed out"
                        }
                        .into());
                    }
                    if leader_exited(&child)? {
                        return Ok(());
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })();
            // Even an exited leader may leave writers behind. Stop and wait for its
            // owned group on every path, including success and logging/read failures.
            let result = match terminate(&mut child) {
                Ok(status) => {
                    if let Some(path) = &active {
                        std::fs::remove_file(path)?;
                    }
                    waited.and_then(|()| {
                        stdout.seek(SeekFrom::Start(0))?;
                        stderr.seek(SeekFrom::Start(0))?;
                        let mut bytes = Vec::new();
                        stdout.read_to_end(&mut bytes)?;
                        let mut errors = Vec::new();
                        stderr.read_to_end(&mut errors)?;
                        self.event(
                            json!({"argv":effective,"stdout":String::from_utf8_lossy(&bytes),
                            "stderr":String::from_utf8_lossy(&errors),"status":status.code(),"at":now()}),
                        )?;
                        if !status.success() {
                            std::io::stderr().write_all(&bytes)?;
                            std::io::stderr().write_all(&errors)?;
                            return Err(format!(
                                "Required command failed ({status}): {}",
                                effective.join(" ")
                            )
                            .into());
                        }
                        std::io::stderr().write_all(&errors)?;
                        Ok(bytes)
                    })
                }
                Err(error) => {
                    self.cleanup_failed.store(true, Ordering::SeqCst);
                    Err(format!("Subprocess cleanup failed: {error}").into())
                }
            };
            if result.is_err() {
                stdout.seek(SeekFrom::Start(0))?;
                stderr.seek(SeekFrom::Start(0))?;
                let mut out = Vec::new();
                let mut err = Vec::new();
                stdout.read_to_end(&mut out)?;
                stderr.read_to_end(&mut err)?;
                self.event(
                    json!({"argv":effective,"stdout":String::from_utf8_lossy(&out),
                "stderr":String::from_utf8_lossy(&err),"at":now()}),
                )?;
            }
            self.event(
                json!({"argv":effective,"state":if result.is_ok(){"succeeded"}else{"failed"},
            "error":result.as_ref().err().map(ToString::to_string),"at":now()}),
            )?;
            result
        }
    }

    pub fn run(&self, root: &Path, argv: &[String]) -> Result<()> {
        let output = self.capture(root, argv)?;
        std::io::stderr().write_all(&output)?;
        Ok(())
    }

    #[cfg(unix)]
    fn event(&self, value: serde_json::Value) -> Result<()> {
        if let Some(path) = &self.log {
            let mut bytes = serde_json::to_vec(&value)?;
            bytes.push(b'\n');
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            file.write_all(&bytes)?;
            file.sync_data()?;
        }
        Ok(())
    }
}

#[cfg(unix)]
fn leader_exited(child: &std::process::Child) -> Result<bool> {
    #[cfg(not(any(
        target_os = "cygwin",
        target_os = "horizon",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "wasi"
    )))]
    {
        use rustix::process::{waitid, Pid, WaitId, WaitIdOptions};
        loop {
            // Keep the leader's PID reserved through the final process-group signal.
            match waitid(
                WaitId::Pid(Pid::from_child(child)),
                WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
            ) {
                Ok(status) => return Ok(status.is_some()),
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => {
                    return Err(format!("Cannot observe owned child exit: {error}").into())
                }
            }
        }
    }
    #[cfg(any(
        target_os = "cygwin",
        target_os = "horizon",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "wasi"
    ))]
    {
        let _ = child;
        Err("Non-reaping child exit observation is unavailable".into())
    }
}

#[cfg(unix)]
fn terminate(child: &mut std::process::Child) -> Result<std::process::ExitStatus> {
    use nix::{
        errno::Errno,
        sys::signal::{killpg, Signal},
        unistd::Pid,
    };
    // Unknown custody must fail before any signal to a potentially reused number.
    leader_exited(child)?;
    let group = Pid::from_raw(child.id().try_into()?);
    // Product command wrappers may own nested sessions. Give their cancellation
    // handlers time to reap those children before forcing our group to stop.
    match killpg(group, Signal::SIGTERM) {
        Ok(()) | Err(Errno::ESRCH) => {}
        #[cfg(target_os = "macos")]
        Err(Errno::EPERM) if !group_live(child.id())? => {}
        Err(error) => return Err(error.into()),
    }
    let grace = Instant::now() + Duration::from_secs(5);
    while Instant::now() < grace {
        if leader_exited(child)? {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    match killpg(group, Signal::SIGKILL) {
        Ok(()) | Err(Errno::ESRCH) => {}
        #[cfg(target_os = "macos")]
        Err(Errno::EPERM) if !group_live(child.id())? => {}
        Err(error) => return Err(error.into()),
    }
    // No destructive signal is permitted after releasing the leader's wait ownership.
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            return Err("Owned subprocess leader was not reaped after termination".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    while group_live(child.id())? {
        if Instant::now() >= deadline {
            return Err("Owned subprocess group is still live after termination".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(status)
}

#[cfg(unix)]
pub(crate) fn group_live(pid: u32) -> Result<bool> {
    use nix::{errno::Errno, sys::signal::killpg, unistd::Pid};
    match killpg(Pid::from_raw(pid.try_into()?), None) {
        #[cfg(target_os = "macos")]
        Ok(()) | Err(Errno::EPERM) => macos_group_live(pid),
        #[cfg(not(target_os = "macos"))]
        Ok(()) => Ok(true),
        Err(Errno::ESRCH) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(target_os = "macos")]
fn macos_group_live(pid: u32) -> Result<bool> {
    use nix::{errno::Errno, sys::signal::killpg, unistd::Pid};
    // Darwin skips zombies when signalling a group and may return EPERM even
    // for our own terminal children. Confirm states instead of ignoring EPERM.
    let output = Command::new("/bin/ps")
        .args(["-g", &pid.to_string(), "-o", "stat="])
        .output()?;
    let states = String::from_utf8(output.stdout)?;
    if output.status.success() && output.stderr.is_empty() && !states.trim().is_empty() {
        return Ok(states.split_whitespace().any(|s| !s.starts_with('Z')));
    }
    if (output.status.success() || output.status.code() == Some(1))
        && states.trim().is_empty()
        && output.stderr.is_empty()
    {
        // Empty output alone is not evidence that an EPERM group disappeared.
        return match killpg(Pid::from_raw(pid.try_into()?), None) {
            Err(Errno::ESRCH) => Ok(false),
            Ok(()) => Ok(true),
            Err(error) => Err(error.into()),
        };
    }
    Err("Cannot verify owned subprocess group states".into())
}

#[cfg(all(test, target_os = "macos"))]
#[path = "../tests/unit/execution_process.rs"]
mod tests;

#[cfg(all(test, unix))]
#[path = "../tests/unit/execution_deadline.rs"]
mod deadline_tests;

#[cfg(all(test, unix))]
#[path = "../tests/unit/execution_custody.rs"]
mod custody_tests;
