//! Ordinary execution has no implicit deadline; cancellation still reaps helpers.

use super::Runner;
use std::{path::PathBuf, time::Duration};

#[test]
fn ordinary_commands_wait_for_completion_without_an_implicit_deadline() {
    let runner = Runner::default();
    assert!(runner.timeout.is_none());
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        runner
            .capture(
                root.path(),
                &[
                    "/bin/sh".into(),
                    "-c".into(),
                    "sleep 0.05; printf complete".into()
                ]
            )
            .unwrap(),
        b"complete"
    );
}

#[test]
fn explicit_deadlines_still_stop_commands() {
    let runner = Runner {
        timeout: Some(Duration::from_millis(50)),
        ..Runner::default()
    };
    let root = tempfile::tempdir().unwrap();
    let error = runner
        .capture(root.path(), &["/bin/sleep".into(), "30".into()])
        .unwrap_err();
    assert!(error.to_string().contains("timed out"));
    runner.rollback_safe().unwrap();
}

struct DetachedCleanup(PathBuf);

impl Drop for DetachedCleanup {
    fn drop(&mut self) {
        if !self.0.join("reaped").exists() {
            if let Ok(pid) = std::fs::read_to_string(self.0.join("ready")) {
                if let Ok(pid) = pid.parse::<i32>() {
                    let _ = nix::sys::signal::killpg(
                        nix::unistd::Pid::from_raw(pid),
                        nix::sys::signal::Signal::SIGKILL,
                    );
                }
            }
        }
    }
}

#[test]
fn cancellation_allows_wrappers_to_reap_their_nested_sessions() {
    let root = tempfile::tempdir().unwrap();
    let _cleanup = DetachedCleanup(root.path().to_path_buf());
    std::fs::write(
        root.path().join("wrapper.py"),
        r#"import os, pathlib, signal, subprocess, sys, time
child = None
def stop(signum, frame):
    if child is not None:
        os.killpg(child.pid, signal.SIGTERM)
        child.wait(timeout=3)
        pathlib.Path('reaped').write_text(str(child.returncode))
    sys.exit(128 + signum)
signal.signal(signal.SIGTERM, stop)
child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'], start_new_session=True)
pathlib.Path('ready').write_text(str(child.pid))
while True:
    time.sleep(0.01)
"#,
    )
    .unwrap();
    let path = root.path().to_path_buf();
    let cancel = std::thread::spawn(move || {
        for _ in 0..1000 {
            if path.join("ready").exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        std::fs::write(path.join("cancel"), "cancel").unwrap();
    });
    let runner = Runner {
        cancel_file: Some(root.path().join("cancel")),
        timeout: Some(Duration::from_secs(15)),
        ..Runner::default()
    };
    let result = runner.capture(root.path(), &["python3".into(), "wrapper.py".into()]);
    cancel.join().unwrap();
    assert!(result.unwrap_err().to_string().contains("cancelled"));
    assert_eq!(
        std::fs::read_to_string(root.path().join("reaped")).unwrap(),
        "-15"
    );
    runner.rollback_safe().unwrap();
}
