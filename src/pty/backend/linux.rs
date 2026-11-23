use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};

use portable_pty::Child;
use tokio::io::unix::AsyncFd;

fn child_pidfd(child: &dyn Child) -> std::io::Result<AsyncFd<OwnedFd>> {
    let pid = child
        .process_id()
        .ok_or_else(|| std::io::Error::other("pane child has no process ID"))?;
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    AsyncFd::new(unsafe { OwnedFd::from_raw_fd(fd as _) })
}

pub(crate) async fn wait_for_child(
    mut child: Box<dyn Child + Send + Sync>,
) -> std::io::Result<portable_pty::ExitStatus> {
    let pidfd = match child_pidfd(child.as_ref()) {
        Ok(pidfd) => pidfd,
        Err(err) => {
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, Ordering::Relaxed) {
                tracing::warn!(%err, "pidfd unavailable; using blocking pane child wait");
            } else {
                tracing::debug!(%err, "pidfd unavailable; using blocking pane child wait");
            }
            return super::wait_for_child_blocking(child).await;
        }
    };
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        let mut ready = pidfd.readable().await?;
        ready.clear_ready();
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::{mpsc, Notify};

    use crate::events::AppEvent;
    use crate::layout::PaneId;
    use crate::pane::{AgentDetection, PaneLaunchEnv, PaneRuntime};
    use crate::render_signal::RenderSignal;

    fn pidfd_supported() -> bool {
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, std::process::id(), 0) };
        if fd < 0 {
            return false;
        }
        drop(unsafe { OwnedFd::from_raw_fd(fd as _) });
        true
    }

    #[test]
    fn closed_pane_children_are_reaped_when_blocking_pool_is_busy() {
        // Without pidfd the blocking fallback needs the pool this test fills.
        if !pidfd_supported() {
            return;
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let blocker = rt.spawn_blocking(move || {
            started_tx.send(()).unwrap();
            let _ = release_rx.recv();
        });
        started_rx.recv().unwrap();

        let reaped = rt.block_on(async {
            let (events, mut event_rx) = mpsc::channel(32);
            let runtime = PaneRuntime::spawn_shell_command(
                PaneId::from_raw(42),
                24,
                80,
                std::env::temp_dir(),
                "exec sleep 30",
                &PaneLaunchEnv::default(),
                AgentDetection::Disabled,
                0,
                crate::terminal_theme::TerminalTheme::default(),
                None,
                events,
                Arc::new(Notify::new()),
                Arc::new(RenderSignal::new()),
            )
            .unwrap();
            runtime.shutdown();
            tokio::time::timeout(Duration::from_secs(1), async {
                while let Some(event) = event_rx.recv().await {
                    if let AppEvent::PaneDied {
                        pane_id,
                        exit_reason,
                    } = event
                    {
                        return pane_id == PaneId::from_raw(42)
                            && !matches!(
                                exit_reason,
                                crate::platform::ChildExitReason::WaitFailed
                            );
                    }
                }
                false
            })
            .await
        });
        release_tx.send(()).unwrap();
        rt.block_on(blocker).unwrap();
        assert!(matches!(reaped, Ok(true)));
    }

    // Linux-only: the pidfd reaper. These spawn plain `sh` children so they
    // exercise `wait_for_child` directly, without a PTY.

    fn spawn_sh(script: &str) -> Box<dyn portable_pty::Child + Send + Sync> {
        Box::new(
            std::process::Command::new("sh")
                .args(["-c", script])
                .spawn()
                .unwrap(),
        )
    }

    fn assert_not_zombie(pid: u32) {
        let mut status = 0;
        let result = unsafe { libc::waitpid(pid as libc::pid_t, &mut status, libc::WNOHANG) };
        assert_eq!(result, -1, "pid {pid} still waitable after reaping");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
    }

    fn reap(
        rt: &tokio::runtime::Runtime,
        child: Box<dyn portable_pty::Child + Send + Sync>,
    ) -> portable_pty::ExitStatus {
        let pid = child.process_id().unwrap();
        let status = rt
            .block_on(tokio::time::timeout(
                Duration::from_secs(5),
                super::wait_for_child(child),
            ))
            .expect("reaper timed out")
            .expect("reaper failed");
        assert_not_zombie(pid);
        status
    }

    fn current_thread_rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn reaper_reports_zero_nonzero_and_boundary_exit_codes() {
        let rt = current_thread_rt();
        for code in [0u32, 1, 7, 255] {
            let status = reap(&rt, spawn_sh(&format!("exit {code}")));
            assert_eq!(status.exit_code(), code);
            assert_eq!(status.success(), code == 0);
            assert_eq!(status.signal(), None);
            assert!(matches!(
                crate::platform::classify_child_exit(&status),
                crate::platform::ChildExitReason::Exited
            ));
        }
    }

    #[test]
    fn reaper_reports_a_child_killed_by_a_signal() {
        let rt = current_thread_rt();
        let child = spawn_sh("exec sleep 30");
        let pid = child.process_id().unwrap();
        let status = rt.block_on(async {
            let wait = tokio::spawn(super::wait_for_child(child));
            // Let the reaper register its pidfd before the child dies.
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert_eq!(unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) }, 0);
            tokio::time::timeout(Duration::from_secs(5), wait)
                .await
                .expect("reaper timed out")
                .unwrap()
                .unwrap()
        });
        assert!(!status.success());
        assert!(status.signal().is_some());
        assert!(matches!(
            crate::platform::classify_child_exit(&status),
            crate::platform::ChildExitReason::Interrupted
        ));
        assert_not_zombie(pid);
    }

    #[test]
    fn reaper_handles_a_child_that_exited_before_registration() {
        let child = spawn_sh("exit 3");
        let pid = child.process_id().unwrap();
        // Wait until the kernel shows the child as a zombie, before any pidfd exists.
        let stat = format!("/proc/{pid}/stat");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let text = std::fs::read_to_string(&stat).unwrap();
            let state = text.rsplit(')').next().unwrap().trim_start();
            if state.starts_with('Z') {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "child never exited");
            std::thread::sleep(Duration::from_millis(5));
        }
        let status = reap(&current_thread_rt(), child);
        assert_eq!(status.exit_code(), 3);
    }

    #[test]
    fn many_children_closing_at_once_are_all_reaped_with_a_busy_blocking_pool() {
        if !pidfd_supported() {
            return;
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let blocker = rt.spawn_blocking(move || {
            started_tx.send(()).unwrap();
            let _ = release_rx.recv();
        });
        started_rx.recv().unwrap();

        let children: Vec<_> = (0..32u32)
            .map(|code| {
                let child = spawn_sh(&format!("exit {code}"));
                (code, child.process_id().unwrap(), child)
            })
            .collect();
        let results = rt.block_on(async {
            let waits: Vec<_> = children
                .into_iter()
                .map(|(code, pid, child)| (code, pid, tokio::spawn(super::wait_for_child(child))))
                .collect();
            tokio::time::timeout(Duration::from_secs(5), async {
                let mut results = Vec::new();
                for (code, pid, wait) in waits {
                    results.push((code, pid, wait.await.unwrap()));
                }
                results
            })
            .await
        });
        release_tx.send(()).unwrap();
        rt.block_on(blocker).unwrap();

        let results = results.expect("reaper stalled behind the busy blocking pool");
        assert_eq!(results.len(), 32);
        for (code, pid, status) in results {
            assert_eq!(status.unwrap().exit_code(), code);
            assert_not_zombie(pid);
        }
    }

    #[derive(Debug, Clone)]
    struct NoPidChild;

    impl portable_pty::ChildKiller for NoPidChild {
        fn kill(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
            Box::new(self.clone())
        }
    }

    impl portable_pty::Child for NoPidChild {
        fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
            Ok(Some(portable_pty::ExitStatus::with_exit_code(9)))
        }

        fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
            Ok(portable_pty::ExitStatus::with_exit_code(9))
        }

        fn process_id(&self) -> Option<u32> {
            None
        }
    }

    #[test]
    fn child_without_a_pid_falls_back_to_the_blocking_wait() {
        let rt = current_thread_rt();
        // Twice: the first fallback warns, later ones only log at debug.
        for _ in 0..2 {
            let status = rt
                .block_on(super::wait_for_child(Box::new(NoPidChild)))
                .unwrap();
            assert_eq!(status.exit_code(), 9);
        }
    }
}
