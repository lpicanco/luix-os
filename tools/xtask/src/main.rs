mod firmware;
mod image;
mod qemu;

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, ensure, Context, Result};

const USAGE: &str = "Usage:\n  cargo xtask setup [limine|ovmf]\n  cargo xtask image [--test|--bios] [kernel]\n  cargo xtask test-qemu";
const NORMAL_IMAGE: &str = "luix-os.img";
const TEST_IMAGE: &str = "luix-os-test.img";
const BIOS_IMAGE: &str = "luix-os-bios.img";

struct Paths {
    root: &'static Path,
    cache: PathBuf,
    bin: PathBuf,
    images: PathBuf,
}

impl Paths {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("xtask must be under tools/xtask");
        let target = env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("target"));
        Self {
            root,
            cache: root.join(".cache/luix/downloads"),
            bin: target.join("x86_64-unknown-none/debug"),
            images: target.join("images"),
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let paths = Paths::new();

    match args.split_first() {
        Some((command, rest)) if command == "setup" => setup(&paths.cache, rest),
        Some((command, rest)) if command == "image" => create_image(&paths, rest),
        Some((command, [])) if command == "test-qemu" => {
            let ovmf = firmware::ovmf(&paths.cache)?;
            qemu::test(&ovmf, &paths.images.join(TEST_IMAGE))
        }
        Some((command, [])) if command == "--help" || command == "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        _ => bail!("{USAGE}"),
    }
}

fn setup(cache: &Path, args: &[OsString]) -> Result<()> {
    match args {
        [] => {
            firmware::limine(cache)?;
            firmware::ovmf(cache)?;
        }
        [asset] if asset == "limine" => {
            firmware::limine(cache)?;
        }
        [asset] if asset == "ovmf" => {
            firmware::ovmf(cache)?;
        }
        _ => bail!("{USAGE}"),
    }
    Ok(())
}

fn create_image(paths: &Paths, args: &[OsString]) -> Result<()> {
    let (name, kernel, args) = match args.first().and_then(|arg| arg.to_str()) {
        Some("--test") => (TEST_IMAGE, "kernel-test", &args[1..]),
        Some("--bios") => (BIOS_IMAGE, "kernel", &args[1..]),
        Some(arg) if arg.starts_with('-') => bail!("{USAGE}"),
        _ => (NORMAL_IMAGE, "kernel", args),
    };
    ensure!(args.len() <= 1, "{USAGE}");

    let output = paths.images.join(name);
    let kernel = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.bin.join(kernel));
    let bootx64 = firmware::limine(&paths.cache)?;
    let bios = if name == BIOS_IMAGE {
        Some(build_bios_installer(&paths.cache)?)
    } else {
        None
    };
    let installer = bios.as_ref().map(|build| build.path().join("limine"));
    image::write(
        &output,
        &kernel,
        &paths.bin,
        paths.root,
        &bootx64,
        name == TEST_IMAGE,
        installer.as_deref(),
    )?;
    println!("{}", output.display());
    Ok(())
}

fn build_bios_installer(cache: &Path) -> Result<tempfile::TempDir> {
    let source = firmware::bios(cache)?;
    let build =
        tempfile::tempdir().context("creating a build directory for the Limine BIOS installer")?;
    fs::copy(
        source.join("limine-bios.sys"),
        build.path().join("limine-bios.sys"),
    )
    .context("staging the Limine BIOS boot file")?;
    let cc = env::var_os("CC");
    let status = Command::new(cc.as_deref().unwrap_or_else(|| OsStr::new("cc")))
        .args(["-std=c99", "-O2"])
        .arg(source.join("limine.c"))
        .arg("-o")
        .arg(build.path().join("limine"))
        .status()
        .context("running the C compiler for the Limine BIOS installer")?;
    ensure!(
        status.success(),
        "building the Limine BIOS installer failed with {status}"
    );
    Ok(build)
}
