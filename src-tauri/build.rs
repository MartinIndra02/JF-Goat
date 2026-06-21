use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    tauri_build::build();
    emit_release_pipeline_hints();

    #[cfg(target_os = "windows")]
    {
        if let Err(e) = ensure_mpv_import_lib() {
            panic!("failed to prepare mpv import library: {}", e);
        }
    }
}

fn emit_release_pipeline_hints() {
    let profile = env::var("PROFILE").unwrap_or_default();
    if profile != "release" {
        return;
    }

    if env::var_os("TAURI_SIGNING_PRIVATE_KEY").is_none() {
        println!(
            "cargo:warning=TAURI_SIGNING_PRIVATE_KEY is not set; updater signatures will not be produced."
        );
    }

    let has_windows_cert_path = env::var_os("WINDOWS_CERTIFICATE_PATH").is_some();
    let has_windows_cert_password = env::var_os("WINDOWS_CERTIFICATE_PASSWORD").is_some();

    if !(has_windows_cert_path && has_windows_cert_password) {
        println!(
            "cargo:warning=WINDOWS_CERTIFICATE_PATH/WINDOWS_CERTIFICATE_PASSWORD not fully set; Windows binaries may remain unsigned."
        );
    }
}

#[cfg(target_os = "windows")]
fn ensure_mpv_import_lib() -> Result<(), String> {
    println!("cargo:rerun-if-changed=bin/mpv-2.dll");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|e| e.to_string())?);
    let mpv_dll = manifest_dir.join("bin").join("mpv-2.dll");
    if !mpv_dll.exists() {
        return Err(format!("missing required file: {}", mpv_dll.display()));
    }
    let meta = fs::metadata(&mpv_dll)
        .map_err(|e| format!("failed to read metadata for {}: {}", mpv_dll.display(), e))?;
    if meta.len() == 0 {
        return Err(format!(
            "{} is empty (0 bytes). Replace it with a valid x64 libmpv DLL.",
            mpv_dll.display()
        ));
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|e| e.to_string())?);
    let def_path = out_dir.join("mpv.def");
    let lib_path = out_dir.join("mpv.lib");

    let (dumpbin, lib_exe) = find_msvc_tools()?;

    let dump_output = Command::new(&dumpbin)
        .arg("/exports")
        .arg(&mpv_dll)
        .output()
        .map_err(|e| format!("failed to run dumpbin: {}", e))?;

    if !dump_output.status.success() {
        return Err(format!(
            "dumpbin failed with status {}",
            dump_output.status
        ));
    }

    let stdout = String::from_utf8(dump_output.stdout)
        .map_err(|e| format!("dumpbin output was not valid UTF-8: {}", e))?;

    let exports = parse_dumpbin_exports(&stdout);
    if exports.is_empty() {
        return Err("no exports found in mpv-2.dll".to_string());
    }

    let mut def = String::from("LIBRARY mpv-2.dll\nEXPORTS\n");
    for sym in exports {
        def.push_str("    ");
        def.push_str(&sym);
        def.push('\n');
    }
    fs::write(&def_path, def).map_err(|e| format!("failed writing {}: {}", def_path.display(), e))?;

    let lib_status = Command::new(&lib_exe)
        .arg(format!("/def:{}", def_path.display()))
        .arg("/machine:x64")
        .arg(format!("/out:{}", lib_path.display()))
        .status()
        .map_err(|e| format!("failed to run lib.exe: {}", e))?;

    if !lib_status.success() {
        return Err(format!("lib.exe failed with status {}", lib_status));
    }

    println!("cargo:rustc-link-search=native={}", out_dir.display());

    // Copy mpv-2.dll to the target executable directory
    let target_dir = out_dir
        .parent() // build
        .and_then(|p| p.parent()) // build/jfgoat-<hash>
        .and_then(|p| p.parent()) // target/<profile>
        .ok_or("Failed to determine target directory")?;
    
    let dest_dll = target_dir.join("mpv-2.dll");
    fs::copy(&mpv_dll, &dest_dll)
        .map_err(|e| format!("failed to copy mpv-2.dll to {}: {}", dest_dll.display(), e))?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn parse_dumpbin_exports(output: &str) -> Vec<String> {
    let mut exports = Vec::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        let is_ordinal = parts[0].chars().all(|c| c.is_ascii_digit());
        let is_hint = parts[1].chars().all(|c| c.is_ascii_hexdigit());
        if !is_ordinal || !is_hint {
            continue;
        }

        exports.push(parts[3].to_string());
    }

    exports
}

#[cfg(target_os = "windows")]
fn find_msvc_tools() -> Result<(PathBuf, PathBuf), String> {
    let roots = [
        PathBuf::from(r"C:\Program Files\Microsoft Visual Studio"),
        PathBuf::from(r"C:\Program Files (x86)\Microsoft Visual Studio"),
    ];

    let mut best: Option<(PathBuf, PathBuf)> = None;

    for root in roots {
        if !root.exists() {
            continue;
        }

        for year_dir in list_dirs(&root) {
            for edition_dir in list_dirs(&year_dir) {
                let msvc_root = edition_dir.join("VC").join("Tools").join("MSVC");
                if !msvc_root.exists() {
                    continue;
                }

                for ver_dir in list_dirs(&msvc_root) {
                    let bin = ver_dir.join("bin").join("HostX64").join("x64");
                    let dumpbin = bin.join("dumpbin.exe");
                    let lib_exe = bin.join("lib.exe");
                    if dumpbin.exists() && lib_exe.exists() {
                        best = Some((dumpbin, lib_exe));
                    }
                }
            }
        }
    }

    best.ok_or_else(|| "unable to locate dumpbin.exe/lib.exe in a Visual Studio installation".to_string())
}

#[cfg(target_os = "windows")]
fn list_dirs(path: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(read_dir) = fs::read_dir(path) else {
        return out;
    };

    for entry in read_dir.flatten() {
        let p = entry.path();
        if p.is_dir() {
            out.push(p);
        }
    }
    out.sort();
    out
}
