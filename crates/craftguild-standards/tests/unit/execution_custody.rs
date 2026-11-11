//! Final group signals precede reaping, including an early-exiting TERM leader.

use std::{
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct OwnedGroup {
    leader: Child,
    termination_attempted: bool,
}

impl Drop for OwnedGroup {
    fn drop(&mut self) {
        if !self.termination_attempted && super::leader_exited(&self.leader).is_ok() {
            let _ = rustix::process::kill_process_group(
                rustix::process::Pid::from_child(&self.leader),
                rustix::process::Signal::KILL,
            );
            let _ = self.leader.wait();
        }
    }
}

#[test]
fn term_exited_leader_retains_custody_until_ignoring_descendant_is_killed() {
    let root = tempfile::tempdir().unwrap();
    let script = r#"import pathlib, signal, subprocess, sys, time
child = subprocess.Popen([sys.executable, '-c', '''import os, pathlib, signal, time
signal.signal(signal.SIGTERM, signal.SIG_IGN)
pathlib.Path('descendant').write_text(str(os.getpid()))
time.sleep(30)
'''])
while True:
    time.sleep(0.01)
"#;
    let child = Command::new("python3")
        .args(["-c", script])
        .current_dir(root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let mut group = OwnedGroup {
        leader: child,
        termination_attempted: false,
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while !root.path().join("descendant").exists() {
        assert!(Instant::now() < deadline, "Descendant did not become ready");
        std::thread::sleep(Duration::from_millis(5));
    }
    let descendant: i32 = std::fs::read_to_string(root.path().join("descendant"))
        .unwrap()
        .parse()
        .unwrap();
    group.termination_attempted = true;
    let status = super::terminate(&mut group.leader).unwrap();
    assert!(!status.success());
    assert_eq!(
        rustix::process::test_kill_process(rustix::process::Pid::from_raw(descendant).unwrap()),
        Err(rustix::io::Errno::SRCH)
    );
    assert!(super::leader_exited(&group.leader).is_err());
}

#[test]
fn reaped_leader_refuses_further_group_termination() {
    let mut child = Command::new("/bin/sh")
        .args(["-c", "exit 0"])
        .process_group(0)
        .spawn()
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert!(super::terminate(&mut child).is_err());
}
