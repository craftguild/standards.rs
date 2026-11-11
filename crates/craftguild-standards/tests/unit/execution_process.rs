//! Darwin retains group membership for an unreaped zombie but refuses signals.

use nix::{
    errno::Errno,
    sys::signal::{killpg, Signal},
    unistd::Pid,
};
use std::{
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct OwnedGroup {
    leader: Child,
    zombie: Option<Child>,
    termination_attempted: bool,
}

impl Drop for OwnedGroup {
    fn drop(&mut self) {
        if !self.termination_attempted && super::leader_exited(&self.leader).is_ok() {
            let _ = killpg(Pid::from_raw(self.leader.id() as i32), Signal::SIGKILL);
            let _ = self.leader.wait();
        }
        if let Some(zombie) = &mut self.zombie {
            let _ = zombie.wait();
        }
    }
}

#[test]
fn zombie_only_owned_group_is_terminal_despite_eperm() {
    let leader = Command::new("/bin/sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let mut group = OwnedGroup {
        leader,
        zombie: None,
        termination_attempted: false,
    };
    let pgid = Pid::from_raw(group.leader.id() as i32);
    group.zombie = Some(
        Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .process_group(pgid.as_raw())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let state = Command::new("/bin/ps")
            .args([
                "-p",
                &group.zombie.as_ref().unwrap().id().to_string(),
                "-o",
                "stat=",
            ])
            .output()
            .unwrap();
        assert!(state.status.success());
        if String::from_utf8(state.stdout)
            .unwrap()
            .trim()
            .starts_with('Z')
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Owned child did not become a zombie"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    // A group containing a live leader must still prevent rollback.
    assert!(super::group_live(group.leader.id()).unwrap());
    killpg(pgid, Signal::SIGKILL).unwrap();
    while !super::leader_exited(&group.leader).unwrap() {
        assert!(Instant::now() < deadline, "Owned leader did not exit");
        std::thread::sleep(Duration::from_millis(5));
    }
    // Keep the other child unreaped so this kernel state cannot race launchd.
    assert_eq!(killpg(pgid, None), Err(Errno::EPERM));
    assert!(!super::group_live(group.leader.id()).unwrap());
    // The initial termination signal encounters the same EPERM state as well.
    group.termination_attempted = true;
    super::terminate(&mut group.leader).unwrap();
}
