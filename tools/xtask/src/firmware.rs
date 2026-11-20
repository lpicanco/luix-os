use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use ureq::tls::{RootCerts, TlsConfig};
use ureq::Agent;

const LIMINE_REVISION: &str = "ca3a5b20d2501e3819983c5cf838c70c30ee532a";
const LIMINE: Asset = Asset {
    name: "BOOTX64.EFI",
    url: "https://raw.githubusercontent.com/luix-os/limine/ca3a5b20d2501e3819983c5cf838c70c30ee532a/BOOTX64.EFI",
    sha256: "e9071610898f6a40be55e0aa60226b2a31ba763daf494ab29affc0ceae17bb54",
};
const LIMINE_LICENSE: Asset = Asset {
    name: "Limine-LICENSE.txt",
    url: "https://raw.githubusercontent.com/luix-os/limine/ca3a5b20d2501e3819983c5cf838c70c30ee532a/LICENSE",
    sha256: "2da3926086b570229dd844ae2ed427d51fb8981b3e97625c19c1d507ba7648f9",
};
const LIMINE_BIOS: [Asset; 3] = [
    Asset {
        name: "limine.c",
        url: "https://raw.githubusercontent.com/luix-os/limine/ca3a5b20d2501e3819983c5cf838c70c30ee532a/limine.c",
        sha256: "e379a58290983c3e1355a3ecd004a755f1ab269a6fa7de87783b4c40daa5301b",
    },
    Asset {
        name: "limine-bios-hdd.h",
        url: "https://raw.githubusercontent.com/luix-os/limine/ca3a5b20d2501e3819983c5cf838c70c30ee532a/limine-bios-hdd.h",
        sha256: "af8c911369ddca5e39154b708fafbf4e82ee329cdb5d3d6f3e2b701b3abda418",
    },
    Asset {
        name: "limine-bios.sys",
        url: "https://raw.githubusercontent.com/luix-os/limine/ca3a5b20d2501e3819983c5cf838c70c30ee532a/limine-bios.sys",
        sha256: "66f02e013fe044f3a954cb72474c29796f6541d990019c23269068100044f556",
    },
];
const OVMF: Asset = Asset {
    name: "OVMF.fd",
    url: "https://raw.githubusercontent.com/luix-os/edk2-nightly/b7099c7daab0ac05a8fdf1fd0dcb21212d3c9f77/bin/RELEASEX64_OVMF.fd",
    sha256: "90a1f21fafb839cd73859c999b78091ca9da03996182c184cb17ebec1cdc17b7",
};
// The X64 nightly uses OvmfPkgX64.dsc, which includes EDK II's OpenSSL library.
const OVMF_LICENSES: [Asset; 2] = [
    Asset {
        name: "OVMF-EDK-II-LICENSE.txt",
        url: "https://raw.githubusercontent.com/tianocore/edk2/556dcfb466e9f05affeaff881d738807f8156d3c/License.txt",
        sha256: "50ce20c9cfdb0e19ee34fe0a51fc0afe961f743697b068359ab2f862b494df80",
    },
    Asset {
        name: "OVMF-OpenSSL-LICENSE.txt",
        url: "https://raw.githubusercontent.com/openssl/openssl/8cf17aaeb4599f8af87fefd810b5b5fee90fe69e/LICENSE.txt",
        sha256: "7d5450cb2d142651b8afa315b5f238efc805dad827d91ba367d8516bc9d49e7a",
    },
];

const DOWNLOAD_LIMIT: u64 = 8 * 1024 * 1024;

struct Asset {
    name: &'static str,
    url: &'static str,
    sha256: &'static str,
}

pub fn limine(cache: &Path) -> Result<PathBuf> {
    let binary = LIMINE.ensure_cached(cache)?;
    LIMINE_LICENSE.ensure_cached(cache)?;
    Ok(binary)
}

pub fn bios(cache: &Path) -> Result<PathBuf> {
    let source = cache.join(format!("limine-{LIMINE_REVISION}"));
    LIMINE_LICENSE.ensure_cached(cache)?;
    for asset in LIMINE_BIOS {
        asset.ensure_cached(&source)?;
    }
    Ok(source)
}

pub fn ovmf(cache: &Path) -> Result<PathBuf> {
    let binary = OVMF.ensure_cached(cache)?;
    for notice in OVMF_LICENSES {
        notice.ensure_cached(cache)?;
    }
    Ok(binary)
}

impl Asset {
    fn ensure_cached(&self, cache: &Path) -> Result<PathBuf> {
        let destination = cache.join(self.name);
        match File::open(&destination) {
            Ok(file) => {
                let checksum =
                    sha256(file).with_context(|| format!("checking {}", destination.display()))?;
                if checksum == self.sha256 {
                    return Ok(destination);
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| format!("opening {}", destination.display()));
            }
        }

        fs::create_dir_all(cache)
            .with_context(|| format!("creating firmware cache {}", cache.display()))?;
        let mut staged = NamedTempFile::new_in(cache)
            .with_context(|| format!("staging download of {}", self.name))?;
        let tls = TlsConfig::builder()
            .root_certs(RootCerts::PlatformVerifier)
            .build();
        let config = Agent::config_builder()
            .tls_config(tls)
            .timeout_global(Some(Duration::from_secs(60)))
            .build();
        let agent = Agent::new_with_config(config);
        let mut response = agent
            .get(self.url)
            .call()
            .with_context(|| format!("downloading {}", self.url))?;
        let size = io::copy(
            &mut response.body_mut().as_reader().take(DOWNLOAD_LIMIT + 1),
            staged.as_file_mut(),
        )
        .with_context(|| format!("saving download of {}", self.name))?;
        ensure!(
            size <= DOWNLOAD_LIMIT,
            "download of {} is too large",
            self.name
        );
        staged.as_file_mut().rewind()?;
        let checksum = sha256(staged.as_file_mut())
            .with_context(|| format!("checking download of {}", self.name))?;
        ensure!(
            checksum == self.sha256,
            "invalid SHA-256 for {} (expected {}, got {checksum})",
            self.name,
            self.sha256,
        );
        staged
            .persist(&destination)
            .with_context(|| format!("publishing verified file {}", destination.display()))?;
        Ok(destination)
    }
}

fn sha256(mut input: impl Read) -> io::Result<String> {
    let mut digest = Sha256::new();
    io::copy(&mut input, &mut digest)?;
    Ok(format!("{:x}", digest.finalize()))
}
