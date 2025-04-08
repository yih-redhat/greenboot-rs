use anyhow::{Result, bail};
use glob::glob;
use std::error::Error;
use std::path::Path;
use std::process::Command;

/// dir that greenboot looks for the health check and other scripts
static GREENBOOT_INSTALL_PATHS: [&str; 2] = ["/usr/lib/greenboot", "/etc/greenboot"];

/// runs all the scripts in required.d and wanted.d
pub fn run_diagnostics() -> Result<()> {
    let mut required_script_failure: bool = false;
    let mut path_exists: bool = false;
    for path in GREENBOOT_INSTALL_PATHS {
        let greenboot_required_path = format!("{path}/check/required.d/");
        if !Path::new(&greenboot_required_path).is_dir() {
            log::warn!("skipping test as {greenboot_required_path} is not a dir");
            continue;
        }
        path_exists = true;
        let errors = run_scripts("required", &greenboot_required_path);
        if !errors.is_empty() {
            log::error!("required script error:");
            errors.iter().for_each(|e| log::error!("{e}"));
            if !required_script_failure {
                required_script_failure = true;
            }
        }
    }
    if !path_exists {
        bail!("cannot find any required.d folder");
    }
    for path in GREENBOOT_INSTALL_PATHS {
        let greenboot_wanted_path = format!("{path}/check/wanted.d/");
        let errors = run_scripts("wanted", &greenboot_wanted_path);
        if !errors.is_empty() {
            log::warn!("wanted script runner error:");
            errors.iter().for_each(|e| log::error!("{e}"));
        }
    }

    if required_script_failure {
        bail!("health-check failed!");
    }
    Ok(())
}

/// runs all the scripts in red.d when health-check fails
pub fn run_red() -> Vec<Box<dyn Error>> {
    let mut errors = Vec::new();
    for path in GREENBOOT_INSTALL_PATHS {
        let red_path = format!("{path}/red.d/");
        let e = run_scripts("red", &red_path);
        if !e.is_empty() {
            errors.extend(e);
        }
    }
    errors
}

/// runs all the scripts green.d when health-check passes
pub fn run_green() -> Vec<Box<dyn Error>> {
    let mut errors = Vec::new();
    for path in GREENBOOT_INSTALL_PATHS {
        let green_path = format!("{path}/green.d/");
        let e = run_scripts("green", &green_path);
        if !e.is_empty() {
            errors.extend(e);
        }
    }
    errors
}

/// takes in a path and runs all the .sh files within the path
/// returns false if any script fails
fn run_scripts(name: &str, path: &str) -> Vec<Box<dyn Error>> {
    let mut errors = Vec::new();
    let scripts = format!("{path}*.sh");
    match glob(&scripts) {
        Ok(s) => {
            for entry in s.flatten() {
                log::info!("running {name} check {}", entry.to_string_lossy());
                let output = Command::new("bash").arg("-C").arg(entry.clone()).output();
                match output {
                    Ok(o) => {
                        if !o.status.success() {
                            errors.push(Box::new(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!(
                                    "{name} script {} failed! \n{} \n{}",
                                    entry.to_string_lossy(),
                                    String::from_utf8_lossy(&o.stdout),
                                    String::from_utf8_lossy(&o.stderr)
                                ),
                            )) as Box<dyn Error>);
                        } else {
                            log::info!("{name} script {} success!", entry.to_string_lossy());
                        }
                    }
                    Err(e) => {
                        errors.push(Box::new(e) as Box<dyn Error>);
                    }
                }
            }
        }
        Err(e) => errors.push(Box::new(e) as Box<dyn Error>),
    }
    errors
}

#[cfg(test)]
mod test {

    use super::*;
    use anyhow::{Context, Result};
    use std::fs;

    static GREENBOOT_INSTALL_PATHS: [&str; 2] = ["/usr/lib/greenboot", "/etc/greenboot"];

    /// validate when the required folder is not found
    #[test]
    fn missing_required_folder() {
        assert_eq!(
            run_diagnostics().unwrap_err().to_string(),
            String::from("cannot find any required.d folder")
        );
    }

    #[test]
    fn test_passed_diagnostics() {
        setup_folder_structure(true)
            .context("Test setup failed")
            .unwrap();
        let state = run_diagnostics();
        assert!(state.is_ok());
        tear_down().context("Test teardown failed").unwrap();
    }

    #[test]
    fn test_failed_diagnostics() {
        setup_folder_structure(false)
            .context("Test setup failed")
            .unwrap();
        let failed_msg = run_diagnostics().unwrap_err().to_string();
        assert_eq!(failed_msg, String::from("health-check failed!"));
        tear_down().context("Test teardown failed").unwrap();
    }

    fn setup_folder_structure(passing: bool) -> Result<()> {
        let required_path = format!("{}/check/required.d", GREENBOOT_INSTALL_PATHS[1]);
        let wanted_path = format!("{}/check/wanted.d", GREENBOOT_INSTALL_PATHS[1]);
        let passing_test_scripts = "testing_assets/passing_script.sh";
        let failing_test_scripts = "testing_assets/failing_script.sh";

        fs::create_dir_all(&required_path).expect("cannot create folder");
        fs::create_dir_all(&wanted_path).expect("cannot create folder");
        let _a = fs::copy(
            passing_test_scripts,
            format!("{}/passing_script.sh", &required_path),
        )
        .context("unable to copy test assets");

        let _a = fs::copy(
            passing_test_scripts,
            format!("{}/passing_script.sh", &wanted_path),
        )
        .context("unable to copy test assets");

        let _a = fs::copy(
            failing_test_scripts,
            format!("{}/failing_script.sh", &wanted_path),
        )
        .context("unable to copy test assets");

        if !passing {
            let _a = fs::copy(
                failing_test_scripts,
                format!("{}/failing_script.sh", &required_path),
            )
            .context("unable to copy test assets");
            return Ok(());
        }
        Ok(())
    }

    fn tear_down() -> Result<()> {
        fs::remove_dir_all(GREENBOOT_INSTALL_PATHS[1]).expect("Unable to delete folder");
        Ok(())
    }
}
