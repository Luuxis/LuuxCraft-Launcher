//! Java management.
//!
//! `crust_core` already downloads the Mojang runtime matching a version (with
//! an Azul Zulu fallback) and accepts a custom `java.path`. This module adds
//! what a launcher needs around it: listing the installations present on the
//! machine (including the runtimes it manages), probing an executable for its
//! real version, and telling which major version an instance requires.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crust_core::foundation::os::Platform;
use crust_core::resolver::Resolver;
use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::java_major;

/// Java 8 for versions whose manifest has no `javaVersion` (before 1.17),
/// which is what `crust_core` assumes as well.
const LEGACY_JAVA_MAJOR: u32 = 8;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub vendor: String,
    pub arch: String,
    /// Where it was found: `custom`, `path`, `javaHome`, `system`, `managed`.
    pub source: String,
    /// Downloaded by the launcher (`<root>/runtime`).
    pub managed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRequirement {
    pub instance_id: String,
    pub minecraft_version: String,
    pub major: u32,
    /// `panel` when the instance forces a version, `mojang` otherwise.
    pub source: String,
}

/// Lists Java installations found on this machine.
#[tauri::command]
pub async fn java_detect(state: State<'_, AppState>) -> AppResult<Vec<JavaInstall>> {
    let mut candidates: Vec<(PathBuf, &'static str)> = Vec::new();
    let settings = state.settings();
    if let Some(custom) = settings.java.path.as_deref() {
        candidates.push((PathBuf::from(custom), "custom"));
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        candidates.push((
            PathBuf::from(home).join("bin").join(executable_name()),
            "javaHome",
        ));
    }
    candidates.push((PathBuf::from(executable_name()), "path"));
    for dir in system_java_dirs() {
        if let Some(java) = crust_core::launcher::java::locate_java(&dir, Platform::current()) {
            candidates.push((java, "system"));
        }
    }
    let runtime_dir = state.game_root().join("runtime");
    for dir in list_dirs(&runtime_dir) {
        if let Some(java) = crust_core::launcher::java::locate_java(&dir, Platform::current()) {
            candidates.push((java, "managed"));
        } else {
            // Mojang runtimes: runtime/<component>/<platform>/<component>/bin/java
            for nested in list_dirs(&dir) {
                if let Some(java) =
                    crust_core::launcher::java::locate_java(&nested, Platform::current())
                {
                    candidates.push((java, "managed"));
                }
            }
        }
    }

    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut installs = Vec::new();
    for (path, source) in candidates {
        let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !seen.insert(key) {
            continue;
        }
        match probe(&path).await {
            Ok(mut install) => {
                install.source = source.to_owned();
                install.managed = source == "managed";
                installs.push(install);
            }
            Err(error) => log::debug!(
                "java candidate {} skipped: {}",
                path.display(),
                error.message
            ),
        }
    }
    installs.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| a.path.cmp(&b.path)));
    log::info!("java detection: {} installation(s)", installs.len());
    Ok(installs)
}

/// Runs `java -XshowSettings:properties -version` on an executable.
#[tauri::command]
pub async fn java_probe(path: String) -> AppResult<JavaInstall> {
    probe(Path::new(&path)).await
}

/// The Java major version an instance needs, from the panel when it forces
/// one, otherwise from Mojang's version manifest (`javaVersion.majorVersion`).
#[tauri::command]
pub async fn java_required(
    state: State<'_, AppState>,
    instance_id: String,
) -> AppResult<JavaRequirement> {
    let instance = state.instance(&instance_id).await?;
    if let Some(forced) = instance.java_version.as_deref().and_then(java_major) {
        return Ok(JavaRequirement {
            instance_id,
            minecraft_version: instance.minecraft_version,
            major: forced,
            source: "panel".into(),
        });
    }
    let major = state.java_major_for(&instance.minecraft_version).await?;
    Ok(JavaRequirement {
        instance_id,
        minecraft_version: instance.minecraft_version,
        major,
        source: "mojang".into(),
    })
}

impl AppState {
    /// Resolves (and caches) the Java major required by a Minecraft version.
    pub async fn java_major_for(&self, minecraft_version: &str) -> AppResult<u32> {
        if let Some(major) = self
            .java_majors
            .lock()
            .expect("java majors mutex")
            .get(minecraft_version)
            .copied()
        {
            return Ok(major);
        }
        let resolved = Resolver::new(self.http.clone())
            .resolve_version(minecraft_version)
            .await
            .map_err(|error| AppError::from(crust_core::launcher::Error::Resolver(error)))?;
        let major = resolved
            .json
            .java_version
            .as_ref()
            .map(|java| java.major_version)
            .unwrap_or(LEGACY_JAVA_MAJOR);
        self.java_majors
            .lock()
            .expect("java majors mutex")
            .insert(minecraft_version.to_owned(), major);
        Ok(major)
    }
}

pub async fn probe(path: &Path) -> AppResult<JavaInstall> {
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        tokio::process::Command::new(path)
            .args(["-XshowSettings:properties", "-version"])
            .stdin(std::process::Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| AppError::new("java_missing", format!("{} did not answer", path.display())))?
    .map_err(|error| AppError::new("java_missing", format!("{}: {error}", path.display())))?;

    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let install = parse_properties(&text, path).ok_or_else(|| {
        AppError::new(
            "java_incompatible",
            format!("{} is not a usable Java runtime", path.display()),
        )
    })?;
    Ok(install)
}

fn parse_properties(text: &str, path: &Path) -> Option<JavaInstall> {
    let mut version = None;
    let mut vendor = None;
    let mut home = None;
    let mut arch = None;
    for line in text.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "java.version" => version = Some(value.to_owned()),
            "java.vendor" => vendor = Some(value.to_owned()),
            "java.home" => home = Some(value.to_owned()),
            "os.arch" => arch = Some(value.to_owned()),
            _ => {}
        }
    }
    let version = version.or_else(|| {
        // Older runtimes without -XshowSettings still print `java version "1.8.0_x"`.
        text.lines()
            .find(|line| line.contains("version \""))
            .and_then(|line| line.split('"').nth(1))
            .map(str::to_owned)
    })?;
    let major = java_major(&version)?;
    let resolved_path = match home {
        Some(home) if path.components().count() == 1 => {
            let candidate = Path::new(&home).join("bin").join(executable_name());
            if candidate.is_file() {
                candidate
            } else {
                path.to_path_buf()
            }
        }
        _ => path.to_path_buf(),
    };
    Some(JavaInstall {
        path: resolved_path.display().to_string(),
        version,
        major,
        vendor: vendor.unwrap_or_else(|| "unknown".to_owned()),
        arch: arch.unwrap_or_else(|| "unknown".to_owned()),
        source: "system".to_owned(),
        managed: false,
    })
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn list_dirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

/// Well-known installation roots per platform (each child is a JDK/JRE home).
fn system_java_dirs() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
        if let Some(home) = &home {
            roots.push(home.join("Library/Java/JavaVirtualMachines"));
        }
        roots.push(PathBuf::from("/opt/homebrew/opt"));
        roots.push(PathBuf::from("/usr/local/opt"));
    } else if cfg!(target_os = "windows") {
        for base in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Some(dir) = std::env::var_os(base) {
                let dir = PathBuf::from(dir);
                for vendor in [
                    "Java",
                    "Eclipse Adoptium",
                    "Eclipse Foundation",
                    "Zulu",
                    "Microsoft",
                    "Amazon Corretto",
                    "BellSoft",
                    "AdoptOpenJDK",
                ] {
                    roots.push(dir.join(vendor));
                }
            }
        }
    } else {
        roots.push(PathBuf::from("/usr/lib/jvm"));
        roots.push(PathBuf::from("/usr/lib64/jvm"));
        roots.push(PathBuf::from("/opt/java"));
        if let Some(home) = &home {
            roots.push(home.join(".sdkman/candidates/java"));
            roots.push(home.join(".jdks"));
        }
    }
    let mut dirs = Vec::new();
    for root in roots {
        for child in list_dirs(&root) {
            if cfg!(target_os = "macos") && (root.ends_with("opt")) {
                // Homebrew: /opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk
                let name = child
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if !name.starts_with("openjdk") {
                    continue;
                }
                dirs.push(child.join("libexec").join("openjdk.jdk"));
            } else {
                dirs.push(child);
            }
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_show_settings_output() {
        let text = "Property settings:\n    java.home = /opt/jdk-21\n    java.vendor = Eclipse Adoptium\n    java.version = 21.0.6\n    os.arch = aarch64\nopenjdk version \"21.0.6\" 2025-01-21 LTS\n";
        let install = parse_properties(text, Path::new("/opt/jdk-21/bin/java")).unwrap();
        assert_eq!(install.major, 21);
        assert_eq!(install.vendor, "Eclipse Adoptium");
        assert_eq!(install.arch, "aarch64");
    }

    /// Probes the `java` of this machine: `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn probes_the_system_java() {
        let install = probe(Path::new(executable_name()))
            .await
            .expect("java on the PATH");
        assert!(install.major >= 8, "{install:?}");
        assert!(Path::new(&install.path).is_file(), "{}", install.path);
    }

    #[test]
    fn falls_back_to_version_banner() {
        let text = "java version \"1.8.0_392\"\nJava(TM) SE Runtime Environment\n";
        let install = parse_properties(text, Path::new("/x/bin/java")).unwrap();
        assert_eq!(install.major, 8);
        assert!(parse_properties("nothing here", Path::new("/x")).is_none());
    }
}
