use std::ffi::OsString;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};

const TEST_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
// The guest writes 0x10 to isa-debug-exit and QEMU returns (0x10 << 1) | 1.
const TEST_SUCCESS: i32 = 33;

pub fn test(firmware: &Path, image: &Path) -> Result<()> {
    let canceled = Arc::new(AtomicBool::new(false));
    #[cfg(unix)]
    for signal in [
        signal_hook::consts::SIGHUP,
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGQUIT,
        signal_hook::consts::SIGTERM,
    ] {
        signal_hook::flag::register(signal, Arc::clone(&canceled))
            .context("registering QEMU cancellation handler")?;
    }

    for device in [
        "nvme,drive=disk,serial=feedcafe",
        "virtio-blk-pci,drive=disk,disable-legacy=on",
    ] {
        eprintln!("Running QEMU guest tests with {device}");
        test_device(firmware, image, device, &canceled)
            .with_context(|| format!("testing {device}"))?;
    }
    Ok(())
}

fn test_device(firmware: &Path, image: &Path, device: &str, canceled: &AtomicBool) -> Result<()> {
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
        .args(["-smp", "1", "-device", device, "-drive"])
        .arg(drive)
        .args([
            "-display",
            "none",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
        ])
        .spawn()
        .context("starting QEMU guest tests")?;
    let status = wait_for_test(&mut child, TEST_TIMEOUT, canceled)?;
    ensure!(
        status.code() == Some(TEST_SUCCESS),
        "QEMU guest tests exited with {status} (expected exit code {TEST_SUCCESS})"
    );
    Ok(())
}

fn wait_for_test(
    child: &mut Child,
    timeout: Duration,
    canceled: &AtomicBool,
) -> Result<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if canceled.load(Ordering::Relaxed) {
            stop_test(child)?;
            bail!("QEMU guest tests canceled");
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(err) => {
                let _ = stop_test(child);
                return Err(err).context("waiting for QEMU guest tests");
            }
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            stop_test(child)?;
            bail!("QEMU guest tests timed out after {timeout:?}");
        };
        thread::sleep(remaining.min(POLL_INTERVAL));
    }
}

fn stop_test(child: &mut Child) -> Result<()> {
    child.kill().context("stopping QEMU guest tests")?;
    child.wait().context("reaping QEMU guest tests")?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn deadline_kills_and_reaps_guest() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let error = wait_for_test(
            &mut child,
            Duration::from_millis(50),
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn cancellation_kills_and_reaps_guest() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let error = wait_for_test(&mut child, TEST_TIMEOUT, &AtomicBool::new(true)).unwrap_err();
        assert!(error.to_string().contains("canceled"));
        assert!(child.try_wait().unwrap().is_some());
    }
}
