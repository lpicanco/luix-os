use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{ensure, Context, Result};
use fatfs::{Dir, FatType, FileSystem, FormatVolumeOptions, FsOptions, ReadWriteSeek};
use fscommon::{BufStream, StreamSlice};
use gptman::{GPTPartitionEntry, GPT};
use tempfile::NamedTempFile;
use uuid::{uuid, Uuid};

const SECTOR_BYTES: u16 = 512;
const ESP_START_LBA: u64 = 2048;
const ESP_TYPE: Uuid = uuid!("c12a7328-f81f-11d2-ba4b-00a0c93ec93b");

pub fn write(
    output: &Path,
    kernel: &Path,
    bin: &Path,
    root: &Path,
    bootx64: &Path,
    test: bool,
    bios: Option<&Path>,
) -> Result<()> {
    let output_dir = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(output_dir).with_context(|| format!("create {}", output_dir.display()))?;
    let mut disk = NamedTempFile::new_in(output_dir).context("create temporary disk image")?;

    // The guest NVMe tests expect a 42 MiB namespace.
    let size_mib = if test { 42 } else { 64 };
    disk.as_file().set_len(size_mib * 1024 * 1024)?;
    let (start, end) = make_gpt(disk.as_file_mut(), test)?;
    let mut partition = StreamSlice::new(BufStream::new(disk.as_file_mut()), start, end)?;
    let options = FormatVolumeOptions::new()
        .fat_type(FatType::Fat32)
        .bytes_per_sector(SECTOR_BYTES)
        // The kernel FAT reader currently assumes one sector per cluster.
        .bytes_per_cluster(u32::from(SECTOR_BYTES))
        .fats(2);
    fatfs::format_volume(&mut partition, options).context("format EFI partition as FAT32")?;

    let fat = FileSystem::new(&mut partition, FsOptions::new()).context("open EFI partition")?;
    {
        let files = fat.root_dir();
        files.create_dir("EFI")?;
        files.create_dir("EFI/BOOT")?;
        files.create_dir("boot")?;
        files.create_dir("boot/limine")?;
        copy_into(&files, kernel, "boot/kernel")?;
        copy_into(&files, &bin.join("init"), "boot/init")?;
        copy_into(
            &files,
            &root.join("kernel/limine.cfg"),
            "boot/limine/limine.cfg",
        )?;
        copy_into(&files, bootx64, "EFI/BOOT/BOOTX64.EFI")?;

        if test {
            copy_into(&files, &bin.join("spawn-test"), "boot/spawn.tst")?;
            copy_into(&files, &bin.join("exit-test"), "boot/exit.tst")?;
            files.create_dir("test")?;
            files.create_dir("test/deep")?;
            files.create_dir("test/deep/inside")?;
            files.create_dir("long-dir-name")?;
            let mut fixture = files.create_file("test/deep/inside/deepfile.txt")?;
            fixture.write_all(b"This is a file inside a long path\n")?;
            fixture.flush()?;
            copy_into(&files, &root.join("README.md"), "boot/README.md")?;
            copy_into(&files, &root.join("README.md"), "README.md")?;
        }

        if let Some(installer) = bios {
            copy_into(
                &files,
                &installer.with_file_name("limine-bios.sys"),
                "boot/limine/limine-bios.sys",
            )?;
        }
    }
    fat.unmount().context("finalize EFI partition")?;
    partition.flush().context("flush EFI partition")?;
    drop(partition);

    if let Some(installer) = bios {
        let status = Command::new(installer)
            .arg("bios-install")
            .arg(disk.path())
            .status()
            .context("run Limine BIOS installer")?;
        ensure!(status.success(), "Limine BIOS installer returned {status}");
    }

    disk.as_file().sync_all().context("sync disk image")?;
    disk.persist(output)
        .with_context(|| format!("publish disk image to {}", output.display()))?;
    Ok(())
}

fn make_gpt(disk: &mut File, test: bool) -> Result<(u64, u64)> {
    let sector_bytes = u64::from(SECTOR_BYTES);
    let mut gpt = GPT::new_from(disk, sector_bytes, Uuid::new_v4().to_bytes_le())?;
    let last_lba = gpt.header.last_usable_lba;
    gpt[1] = GPTPartitionEntry {
        partition_type_guid: ESP_TYPE.to_bytes_le(),
        unique_partition_guid: Uuid::new_v4().to_bytes_le(),
        starting_lba: ESP_START_LBA,
        ending_lba: last_lba,
        attribute_bits: 0,
        partition_name: if test { "luix-os-test" } else { "" }.into(),
    };
    gpt.write_into(disk)
        .context("write primary and backup GPT")?;
    GPT::write_protective_mbr_into(disk, sector_bytes).context("write protective MBR")?;
    Ok((ESP_START_LBA * sector_bytes, (last_lba + 1) * sector_bytes))
}

fn copy_into(files: &Dir<'_, impl ReadWriteSeek>, source: &Path, output: &str) -> Result<()> {
    let mut source = File::open(source).with_context(|| format!("open {}", source.display()))?;
    let mut file = files
        .create_file(output)
        .with_context(|| format!("create {output} in EFI partition"))?;
    io::copy(&mut source, &mut file).with_context(|| format!("copy {output} to EFI partition"))?;
    file.flush().with_context(|| format!("flush {output}"))
}
