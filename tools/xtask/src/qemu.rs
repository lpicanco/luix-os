use std::ffi::OsString;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};

const TEST_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
// The guest writes 0x10 to isa-debug-exit and QEMU returns (0x10 << 1) | 1.
const TEST_SUCCESS: i32 = 33;

pub fn test(firmware: &Path, image: &Path) -> Result<()> {
    let mut drive = OsString::from("id=disk,file=");
    drive.push(image);
    drive.push(",format=raw,if=none");
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "q35",
            "-serial",
            "stdio",
            "-snapshot",
            "-m",
            "128M",
            "-bios",
        ])
        .arg(firmware)
        .args([
            "-smp",
            "1",
            "-device",
            "nvme,drive=disk,serial=feedcafe",
            "-drive",
        ])
        .arg(drive)
        .args([
            "-display",
            "none",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
        ])
        .spawn()
        .context("starting QEMU guest tests")?;
    let status = wait_for_test(&mut child, TEST_TIMEOUT)?;
    ensure!(
        status.code() == Some(TEST_SUCCESS),
        "QEMU guest tests exited with {status} (expected exit code {TEST_SUCCESS})"
    );
    Ok(())
}

fn wait_for_test(child: &mut Child, timeout: Duration) -> Result<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err).context("waiting for QEMU guest tests");
            }
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            let killed = child.kill();
            child.wait().context("reaping QEMU after the deadline")?;
            killed.context("stopping QEMU after the deadline")?;
            bail!("QEMU guest tests timed out after {timeout:?}");
        };
        thread::sleep(remaining.min(POLL_INTERVAL));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn deadline_kills_and_reaps_guest() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let error = wait_for_test(&mut child, Duration::from_millis(50)).unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(child.try_wait().unwrap().is_some());
    }
}
