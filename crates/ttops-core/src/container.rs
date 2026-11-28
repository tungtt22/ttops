//! Docker-compatible container provider. The scanner only reads its data;
//! cleanup invokes narrow, per-resource operations (never force removal).

use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerImage {
    pub id: String,
    pub repo_tag: String,
    pub size_bytes: u64,
    pub used: bool,
    pub locally_built: bool,
}

pub trait ContainerProvider: Send + Sync {
    fn available(&self) -> bool;
    fn build_cache_bytes(&self) -> Result<u64, String>;
    fn unused_images(&self) -> Result<Vec<ContainerImage>, String>;
    fn prune_build_cache(&self) -> Result<(), String>;
    fn remove_image(&self, id: &str) -> Result<(), String>;
}

#[derive(Default)]
pub struct DockerProvider;
impl DockerProvider {
    fn output(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("docker")
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
impl ContainerProvider for DockerProvider {
    fn available(&self) -> bool {
        self.output(&["info", "--format", "{{.ServerVersion}}"])
            .is_ok()
    }
    fn build_cache_bytes(&self) -> Result<u64, String> {
        let text = self.output(&["system", "df", "--format", "{{json .}}"])?;
        Ok(text
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|row| row.get("Type").and_then(|v| v.as_str()) == Some("Build Cache"))
            .filter_map(|row| {
                row.get("Size")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
            })
            .map(|size| parse_size(&size))
            .sum())
    }
    fn unused_images(&self) -> Result<Vec<ContainerImage>, String> {
        let containers = self.output(&["ps", "-a", "--format", "{{.Image}}"])?;
        let used: std::collections::HashSet<_> = containers.lines().collect();
        let rows = self.output(&["images", "--format", "{{json .}}"])?;
        let mut images = Vec::new();
        for line in rows.lines() {
            let row: serde_json::Value = match serde_json::from_str(line) {
                Ok(row) => row,
                Err(_) => continue,
            };
            let id = row
                .get("ID")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned();
            let repo = row
                .get("Repository")
                .and_then(|v| v.as_str())
                .unwrap_or("<none>");
            let tag = row.get("Tag").and_then(|v| v.as_str()).unwrap_or("<none>");
            let repo_tag = format!("{repo}:{tag}");
            let inspect =
                self.output(&["image", "inspect", &id, "--format", "{{json .RepoDigests}}"])?;
            let locally_built = inspect.trim() == "null" || inspect.trim() == "[]";
            if !used.contains(repo_tag.as_str()) && !locally_built {
                images.push(ContainerImage {
                    id,
                    repo_tag,
                    size_bytes: parse_size(
                        row.get("Size").and_then(|v| v.as_str()).unwrap_or("0B"),
                    ),
                    used: false,
                    locally_built,
                });
            }
        }
        Ok(images)
    }
    fn prune_build_cache(&self) -> Result<(), String> {
        self.output(&["builder", "prune", "-a", "-f"]).map(|_| ())
    }
    fn remove_image(&self, id: &str) -> Result<(), String> {
        self.output(&["rmi", id]).map(|_| ())
    }
}
fn parse_size(value: &str) -> u64 {
    let normalized = value.replace(',', "").trim().to_ascii_uppercase();
    let split = normalized
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(normalized.len());
    let number = normalized[..split].trim().parse::<f64>().unwrap_or(0.0);
    let factor: u64 = match normalized[split..].trim() {
        "KB" | "KIB" => 1_u64 << 10,
        "MB" | "MIB" => 1_u64 << 20,
        "GB" | "GIB" => 1_u64 << 30,
        "TB" | "TIB" => 1_u64 << 40,
        _ => 1,
    };
    (number * factor as f64) as u64
}
