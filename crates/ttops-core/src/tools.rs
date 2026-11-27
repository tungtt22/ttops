//! External developer tools whose own CLI performs the cleanup (Homebrew).
//! Previews always come from a dry run; execution is permanent and audited by the cleaner.

use std::path::PathBuf;
use std::process::Command;

use crate::model::ToolCommand;

pub trait ToolProvider: Send + Sync {
    fn brew_available(&self) -> bool;
    /// Bytes that `brew cleanup` would free and the paths it would remove.
    fn brew_cleanup_preview(&self) -> Result<(u64, Vec<String>), String>;
    /// Formulae that `brew autoremove` would uninstall.
    fn brew_autoremove_preview(&self) -> Result<Vec<String>, String>;
    fn run(&self, command: ToolCommand) -> Result<(), String>;
}

/// No external tools (tests and unsupported platforms).
pub struct NoTools;
impl ToolProvider for NoTools {
    fn brew_available(&self) -> bool {
        false
    }
    fn brew_cleanup_preview(&self) -> Result<(u64, Vec<String>), String> {
        Ok((0, Vec::new()))
    }
    fn brew_autoremove_preview(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn run(&self, _: ToolCommand) -> Result<(), String> {
        Err("tool unavailable".into())
    }
}

/// Homebrew via its CLI. Apps launched from Finder get a minimal `PATH`,
/// so the standard install prefixes are probed before `PATH`.
pub struct BrewProvider {
    brew: Option<PathBuf>,
}

impl BrewProvider {
    pub fn detect() -> Self {
        let candidates = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"];
        let brew = candidates
            .iter()
            .map(PathBuf::from)
            .find(|path| path.is_file())
            .or_else(|| {
                std::env::var_os("PATH").and_then(|paths| {
                    std::env::split_paths(&paths)
                        .map(|dir| dir.join("brew"))
                        .find(|path| path.is_file())
                })
            });
        Self { brew }
    }

    fn output(&self, args: &[&str]) -> Result<String, String> {
        let brew = self.brew.as_ref().ok_or("brew not found")?;
        let output = Command::new(brew)
            .args(args)
            .env("HOMEBREW_NO_AUTO_UPDATE", "1")
            .env("HOMEBREW_NO_ENV_HINTS", "1")
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

const BREW_CLEANUP: &[&str] = &["cleanup", "-s", "--prune=all"];

impl ToolProvider for BrewProvider {
    fn brew_available(&self) -> bool {
        self.brew.is_some()
    }
    fn brew_cleanup_preview(&self) -> Result<(u64, Vec<String>), String> {
        let mut args = BREW_CLEANUP.to_vec();
        args.push("--dry-run");
        Ok(parse_brew_cleanup(&self.output(&args)?))
    }
    fn brew_autoremove_preview(&self) -> Result<Vec<String>, String> {
        Ok(parse_brew_autoremove(
            &self.output(&["autoremove", "--dry-run"])?,
        ))
    }
    fn run(&self, command: ToolCommand) -> Result<(), String> {
        match command {
            ToolCommand::BrewCleanup => self.output(BREW_CLEANUP),
            ToolCommand::BrewAutoremove => self.output(&["autoremove"]),
        }
        .map(|_| ())
    }
}

/// Parses `brew cleanup --dry-run`: the "would free approximately X" summary
/// and every "Would remove:" line.
pub fn parse_brew_cleanup(output: &str) -> (u64, Vec<String>) {
    let lines: Vec<String> = output
        .lines()
        .filter(|line| line.starts_with("Would remove:"))
        .map(str::to_owned)
        .collect();
    let bytes = output
        .lines()
        .find_map(|line| {
            let rest = line.split("would free approximately ").nth(1)?;
            Some(crate::container::parse_size(
                rest.split_whitespace().next()?,
            ))
        })
        .unwrap_or(0);
    (bytes, lines)
}

/// Parses `brew autoremove --dry-run`: formula names listed after the header.
pub fn parse_brew_autoremove(output: &str) -> Vec<String> {
    output
        .lines()
        .skip_while(|line| !line.starts_with("==> Would autoremove"))
        .skip(1)
        .map(str::trim)
        .take_while(|line| !line.is_empty() && !line.starts_with("==>"))
        .map(str::to_owned)
        .collect()
}
