use anyhow::{Context, Result};
use std::process::Command;

use super::types::DefaultApp;

/// Ensure duti is available. If missing, try to install via Homebrew silently.
pub fn ensure_available() -> Result<()> {
    if Command::new("duti").arg("-h").output().is_ok() {
        return Ok(());
    }

    // Try auto-install via Homebrew
    eprintln!("duti not found, installing via Homebrew...");

    let install = Command::new("brew")
        .arg("install")
        .arg("duti")
        .output();

    match install {
        Ok(output) if output.status.success() => {
            eprintln!("duti installed successfully.");
            Ok(())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("already installed") {
                Ok(())
            } else {
                anyhow::bail!(
                    "Failed to install duti via Homebrew.\n\
                     Install manually: brew install duti\n\
                     Error: {}",
                    stderr.trim()
                )
            }
        }
        Err(_) => {
            anyhow::bail!(
                "duti is not installed and Homebrew is not available.\n\
                 Install Homebrew first, then run: brew install duti"
            )
        }
    }
}

/// Query the current default application for a file extension.
/// Parses the output of `duti -x <ext>`.
pub fn query_default(ext: &str) -> Result<Option<DefaultApp>> {
    let ext = ext.trim_start_matches('.');
    let output = Command::new("duti")
        .arg("-x")
        .arg(ext)
        .output()
        .context("failed to run duti -x")?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    // duti -x output format:
    //   AppName
    //   /path/to/App.app
    //   com.bundle.id
    if lines.len() >= 3 {
        Ok(Some(DefaultApp {
            name: lines[0].trim().to_string(),
            bundle_id: lines[2].trim().to_string(),
        }))
    } else if !lines.is_empty() && !lines[0].trim().is_empty() {
        Ok(Some(DefaultApp {
            name: lines[0].trim().to_string(),
            bundle_id: String::new(),
        }))
    } else {
        Ok(None)
    }
}

/// Set the default application for a UTI.
pub fn set_default(bundle_id: &str, uti: &str) -> Result<()> {
    let output = Command::new("duti")
        .arg("-s")
        .arg(bundle_id)
        .arg(uti)
        .arg("all")
        .output()
        .context("failed to run duti -s")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("duti -s failed: {}", stderr.trim());
    }

    Ok(())
}
