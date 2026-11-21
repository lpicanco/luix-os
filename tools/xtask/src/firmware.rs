use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use ureq::tls::{RootCerts, TlsConfig};
use ureq::Agent;

const LIMINE: Asset = Asset {
    name: "limine-12.10.0-binary.tar.gz",
    url: "https://github.com/luix-os/limine/releases/download/v12.10.0/limine-binary.tar.gz",
    sha256: "365ba8785d4e4b55c3368738a642681b31f606afec9f77179e07fc26ebe841e2",
};
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
    LIMINE.ensure_cached(cache)
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
