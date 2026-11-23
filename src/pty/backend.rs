#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub(crate) use unix::*;

#[cfg(windows)]
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

#[cfg(windows)]
pub(crate) struct SpawnedPty {
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::wait_for_child;

#[cfg(not(target_os = "linux"))]
pub(crate) use wait_for_child_blocking as wait_for_child;

pub(crate) async fn wait_for_child_blocking(
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
) -> std::io::Result<portable_pty::ExitStatus> {
    tokio::task::spawn_blocking(move || child.wait())
        .await
        .map_err(std::io::Error::other)?
}

#[cfg(windows)]
pub(crate) fn spawn_with_portable_pty(
    rows: u16,
    cols: u16,
    cmd: CommandBuilder,
) -> std::io::Result<SpawnedPty> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|err| std::io::Error::other(err.to_string()))?;

    Ok(SpawnedPty {
        master: pair.master,
        child,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    // Runs on every platform: Linux uses the pidfd reaper, others the blocking wait.
    fn exit_with(code: u32) -> Box<dyn portable_pty::Child + Send + Sync> {
        #[cfg(windows)]
        let child = std::process::Command::new("cmd")
            .args(["/C", &format!("exit {code}")])
            .spawn();
        #[cfg(not(windows))]
        let child = std::process::Command::new("sh")
            .args(["-c", &format!("exit {code}")])
            .spawn();
        Box::new(child.unwrap())
    }

    #[test]
    fn wait_for_child_reports_exit_codes_on_every_platform() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for code in [0u32, 1, 42] {
            // The timer must be created inside the runtime.
            let status = rt
                .block_on(async {
                    tokio::time::timeout(
                        Duration::from_secs(10),
                        super::wait_for_child(exit_with(code)),
                    )
                    .await
                })
                .expect("child wait timed out")
                .unwrap();
            assert_eq!(status.exit_code(), code);
            assert_eq!(status.success(), code == 0);
        }
    }

    #[test]
    fn blocking_wait_reports_exit_codes() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for code in [0u32, 5] {
            let status = rt
                .block_on(super::wait_for_child_blocking(exit_with(code)))
                .unwrap();
            assert_eq!(status.exit_code(), code);
        }
    }
}
