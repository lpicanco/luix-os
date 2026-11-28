//! Source policy supplements rustc's `forbid(unsafe_code)` (including macro expansions).
use anyhow::{bail, ensure, Context, Result};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    process::Command,
};
use syn::{
    visit::{self, Visit},
    Visibility,
};

#[derive(Default)]
struct Audit {
    trusted: bool,
    violations: Vec<&'static str>,
}
impl<'ast> Visit<'ast> for Audit {
    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        if !self.trusted {
            self.violations.push("unsafe block outside core");
        }
        visit::visit_expr_unsafe(self, node);
    }
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.function(&node.vis, node.sig.unsafety.is_some());
        visit::visit_item_fn(self, node);
    }
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.function(&node.vis, node.sig.unsafety.is_some());
        visit::visit_impl_item_fn(self, node);
    }
    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        self.function(&node.vis, node.unsafety.is_some());
        visit::visit_item_trait(self, node);
    }
    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if !self.trusted && node.unsafety.is_some() {
            self.violations.push("unsafe impl outside core");
        }
        visit::visit_item_impl(self, node);
    }
    fn visit_item_foreign_mod(&mut self, node: &'ast syn::ItemForeignMod) {
        if !self.trusted {
            self.violations.push("FFI outside core");
        }
        visit::visit_item_foreign_mod(self, node);
    }
    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        if !self.trusted
            && ["unsafe", "no_mangle", "export_name", "link_section"]
                .iter()
                .any(|n| node.path().is_ident(n))
        {
            self.violations.push("unsafe attribute outside core");
        }
        visit::visit_attribute(self, node);
    }
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if !self.trusted
            && node.path.segments.last().is_some_and(|s| {
                ["asm", "global_asm", "naked_asm", "include"]
                    .contains(&s.ident.to_string().as_str())
            })
        {
            self.violations
                .push("assembly/source inclusion outside core");
        }
        visit::visit_macro(self, node);
    }
}
impl Audit {
    fn function(&mut self, visibility: &Visibility, is_unsafe: bool) {
        if is_unsafe && (!self.trusted || matches!(visibility, Visibility::Public(_))) {
            self.violations
                .push("unsafe API outside private core implementation");
        }
    }
}
fn audit_source(source: &str, trusted: bool) -> Result<()> {
    let file = syn::parse_file(source).context("parsing Rust source")?;
    let mut audit = Audit {
        trusted,
        ..Audit::default()
    };
    audit.visit_file(&file);
    ensure!(
        audit.violations.is_empty(),
        "{}",
        audit.violations.join(", ")
    );
    Ok(())
}
fn walk(path: &Path, trusted: bool) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, trusted)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            audit_source(&fs::read_to_string(&path)?, trusted)
                .with_context(|| path.display().to_string())?;
        }
    }
    Ok(())
}

pub fn check(root: &Path) -> Result<()> {
    walk(&root.join("kernel/src"), false)?;
    walk(&root.join("kernel_core/src"), true)?;
    walk(&root.join("kernel_core_macros/src"), false)?;
    for file in [
        "kernel/src/lib.rs",
        "kernel/src/main.rs",
        "kernel_core_macros/src/lib.rs",
    ] {
        let source = fs::read_to_string(root.join(file))?;
        let compact: String = source.chars().filter(|c| !c.is_whitespace()).collect();
        ensure!(
            compact.contains("#![forbid(unsafe_code)]"),
            "{file} must forbid unsafe code, including macro expansions"
        );
    }
    // The user feature enables syscall assembly for userland.
    // Compile the kernel ABI alone below, so workspace feature unification cannot hide it.
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["metadata", "--locked", "--format-version", "1"])
        .output()?;
    ensure!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let packages: HashMap<_, _> = metadata["packages"]
        .as_array()
        .context("metadata packages")?
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p))
        .collect();
    let nodes: HashMap<_, _> = metadata["resolve"]["nodes"]
        .as_array()
        .context("metadata resolve graph")?
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p))
        .collect();
    let mut pending: Vec<_> = packages
        .iter()
        .filter(|(_, p)| {
            ["kernel", "kernel_core", "kernel_api"].contains(&p["name"].as_str().unwrap())
        })
        .map(|(&id, _)| id)
        .collect();
    let mut seen = HashSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let package = packages[id];
        if package["targets"].as_array().unwrap().iter().any(|t| {
            t["kind"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k == "proc-macro")
        }) {
            let relative = match package["name"].as_str().unwrap() {
                "kernel_core_macros" => "kernel_core_macros/Cargo.toml",
                "test-macros" => "tools/test-macros/Cargo.toml",
                name => bail!("unreviewed proc macro in kernel dependency graph: {name}"),
            };
            ensure!(
                package["source"].is_null()
                    && Path::new(package["manifest_path"].as_str().unwrap()) == root.join(relative),
                "proc macro must be the reviewed workspace source"
            );
        }
        if let Some(node) = nodes.get(id) {
            pending.extend(
                node["dependencies"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| d.as_str().unwrap()),
            );
        }
    }
    for args in [
        vec!["check", "--locked", "-p", "kernel", "--all-targets"],
        vec![
            "rustc",
            "--locked",
            "-p",
            "kernel_api",
            "--no-default-features",
            "--",
            "-Funsafe_code",
        ],
    ] {
        let status = Command::new("cargo")
            .current_dir(root)
            .args(args)
            .status()?;
        if !status.success() {
            bail!("kernel unsafe boundary compilation failed");
        }
    }
    check_consumers(root)?;
    println!("Unsafe boundary checked: private core implementation, safe kernel/ABI, reviewed proc macros.");
    Ok(())
}

fn check_consumers(root: &Path) -> Result<()> {
    let output = Command::new("cargo")
        .current_dir(root)
        .args([
            "check",
            "--locked",
            "-p",
            "kernel_core",
            "--message-format=json",
        ])
        .output()?;
    ensure!(
        output.status.success(),
        "core metadata build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let messages: Vec<serde_json::Value> = String::from_utf8(output.stdout)?
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let mut dependency_paths = HashSet::new();
    let mut artifact = None;
    for message in &messages {
        if message["reason"] != "compiler-artifact" {
            continue;
        }
        for filename in message["filenames"]
            .as_array()
            .context("artifact filenames")?
        {
            let path = Path::new(filename.as_str().context("artifact filename")?);
            if let Some(parent) = path.parent() {
                dependency_paths.insert(parent.to_path_buf());
            }
            if message["target"]["name"] == "kernel_core"
                && path.extension().is_some_and(|e| e == "rmeta")
            {
                artifact = Some(path.to_path_buf());
            }
        }
    }
    let artifact = artifact.context("core metadata artifact")?;
    let temp = tempfile::tempdir()?;
    let cases = [
        ("safe_consumer", "use kernel_core::{block::BlockDevice, user::ProcessMemory}; pub fn consume(d: &dyn BlockDevice) { let mut bytes = [0; 7]; let _ = d.read_block(0, &mut bytes); if let Ok(mut p) = ProcessMemory::allocate() { let _ = p.write(0, &bytes); } }", None),
        ("raw_mapper", "use kernel_core::memory::MEMORY_MAPPER;", Some("private")),
        ("raw_arch", "use kernel_core::arch::Registers;", Some("private")),
        ("legacy_driver", "use kernel_core::legacy_drivers::nvme;", Some("private")),
        ("forge_region", "fn f() { let _ = kernel_core::user::ProcessMemory { slot: 0, pages: 10 }; }", Some("private")),
        ("forge_context", "fn f(ctx: &mut kernel_core::user::SyscallContext<'_>) { ctx.frame.cpu_flags = 0x3000; }", Some("private")),
        ("unsafe_consumer", "fn f() { unsafe { core::arch::asm!(\"nop\"); } }", Some("unsafe")),
    ];
    for (name, source, expected_error) in cases {
        let path = temp.path().join(format!("{name}.rs"));
        fs::write(
            &path,
            format!("#![no_std]\n#![forbid(unsafe_code)]\n{source}\n"),
        )?;
        let mut command = Command::new("rustc");
        command
            .current_dir(root)
            .args([
                "--edition=2021",
                "--crate-type=lib",
                "--target=x86_64-unknown-none",
                "--emit=metadata",
            ])
            .arg("--out-dir")
            .arg(temp.path())
            .arg("--extern")
            .arg(format!("kernel_core={}", artifact.display()));
        for directory in &dependency_paths {
            command
                .arg("-L")
                .arg(format!("dependency={}", directory.display()));
        }
        let output = command.arg(path).output()?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        match expected_error {
            None => ensure!(output.status.success(), "safe consumer failed: {stderr}"),
            Some(error) => ensure!(
                !output.status.success() && stderr.contains(error),
                "{name}: expected rejection containing {error}: {stderr}"
            ),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_syntax_and_public_core_escape_hatches() {
        for source in [
            "fn f() { unsafe {} }",
            "unsafe impl Send for X {}",
            "pub unsafe fn f() {}",
            "#[unsafe(no_mangle)] fn f() {}",
            "fn f() { core::arch::asm!(\"nop\"); }",
        ] {
            assert!(audit_source(source, false).is_err(), "{source}");
        }
        assert!(audit_source("pub unsafe fn raw() {}", true).is_err());
        assert!(audit_source("pub unsafe trait Raw {}", true).is_err());
        assert!(audit_source("pub(crate) unsafe fn private() {}", true).is_ok());
        assert!(audit_source("fn f() { let _ = \"unsafe\"; }", false).is_ok());
    }
}
