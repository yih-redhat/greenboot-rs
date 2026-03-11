// SPDX-License-Identifier: BSD-3-Clause

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::str;

use crate::grub::get_boot_counter;

/// Detects if the system is managed by bootc or is a rpm-ostree system.
/// First checks for `/run/ostree-booted`, then inspects `status.booted.image`
/// from `bootc status --booted --json` to distinguish between the two.
pub fn detect_os_deployment() -> Option<&'static str> {
    if !Path::new("/run/ostree-booted").exists() {
        log::info!("'/run/ostree-booted' not found, not an ostree-based system");
        return None;
    }

    let output = match Command::new("bootc")
        .args(["status", "--booted", "--json"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return None,
    };

    if !output.status.success() {
        log::error!("'bootc status --booted --json' exited with non-zero status");
        return None;
    }

    let json: Value = match serde_json::from_slice::<Value>(&output.stdout) {
        Ok(json) => json,
        Err(_) => {
            log::error!("Failed to parse JSON from 'bootc status --booted --json'");
            return None;
        }
    };

    if let Some(image_type) = json
        .get("status")
        .and_then(|s| s.get("booted"))
        .and_then(|b| b.get("image"))
        .filter(|v| !v.is_null())
    {
        log::info!("System detected as bootc (status.booted.image: {image_type})");
        Some("bootc")
    } else {
        log::info!("System detected as rpm-ostree (status.booted.image is null or absent)");
        Some("rpm-ostree")
    }
}

/// reboots the system if boot_counter is greater than 0 or can be forced too
pub fn handle_reboot(force: bool) -> Result<()> {
    if !force {
        let boot_counter = get_boot_counter()?;
        if boot_counter <= Some(0) {
            bail!("countdown ended, check greenboot-rollback status")
        };
    }
    log::info!("restarting the system");
    Command::new("systemctl").arg("reboot").status()?;
    Ok(())
}

/// Rollback to the previous deployment if the boot counter allows.
pub fn handle_rollback() -> Result<()> {
    let boot_counter = get_boot_counter()?;

    match boot_counter {
        // Exit early if boot_counter is not set
        None => {
            bail!("System is unhealthy but boot_counter is not set, manual intervention required")
        }
        // Proceed with rollback if boot_counter is <= 0
        Some(counter) if counter <= 0 => {
            log::info!("Greenboot will now attempt to rollback to a previous deployment.");
            if let Some(deployment_cmd) = detect_os_deployment() {
                log::info!("Deployment manager '{deployment_cmd}' detected, attempting rollback.");
                let status = Command::new(deployment_cmd)
                    .arg("rollback")
                    .status()
                    .context(format!("Failed to execute '{deployment_cmd} rollback'"))?;

                if !status.success() {
                    bail!(
                        "Rollback with '{}' failed with status: {}",
                        deployment_cmd,
                        status
                    );
                }
            } else {
                bail!("Rollback only supported in bootc or rpm-ostree environment.");
            }
            Ok(())
        }
        // Reject if boot_counter is > 0
        Some(counter) => bail!("Rollback not initiated as boot_counter is {}", counter),
    }
}

/// writes greenboot status to motd.d/boot-status
pub fn handle_motd(state: &str) -> Result<()> {
    std::fs::write("/etc/motd.d/boot-status", format!("{state}.").as_bytes())
        .map_err(|err| anyhow!("Error writing motd: {}", err))
}
