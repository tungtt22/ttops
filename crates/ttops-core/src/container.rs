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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoppedContainer {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
}

pub trait ContainerProvider: Send + Sync {
    fn available(&self) -> bool;
    fn build_cache_bytes(&self) -> Result<u64, String>;
    fn unused_images(&self) -> Result<Vec<ContainerImage>, String>;
    fn prune_build_cache(&self) -> Result<(), String>;
    fn remove_image(&self, id: &str) -> Result<(), String>;
    /// Untagged images left behind by rebuilds.
    fn dangling_images(&self) -> Result<Vec<ContainerImage>, String> {
        Ok(Vec::new())
    }
    /// Containers that exist but are not running.
    fn stopped_containers(&self) -> Result<Vec<StoppedContainer>, String> {
        Ok(Vec::new())
    }
    /// Removes a stopped container; never forces removal of a running one.
    fn remove_container(&self, _id: &str) -> Result<(), String> {
        Err("container removal unavailable".into())
    }
    /// Volumes not referenced by any container (listed for review only).
    fn unused_volumes(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
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
    fn dangling_images(&self) -> Result<Vec<ContainerImage>, String> {
        let rows = self.output(&["images", "-f", "dangling=true", "--format", "{{json .}}"])?;
        Ok(json_rows(&rows)
            .map(|row| ContainerImage {
                id: field(&row, "ID"),
                repo_tag: "<none>:<none>".into(),
                size_bytes: parse_size(&field(&row, "Size")),
                used: false,
                locally_built: true,
            })
            .collect())
    }
    fn stopped_containers(&self) -> Result<Vec<StoppedContainer>, String> {
        let rows = self.output(&[
            "ps",
            "-a",
            "-s",
            "--filter",
            "status=exited",
            "--filter",
            "status=created",
            "--format",
            "{{json .}}",
        ])?;
        Ok(json_rows(&rows)
            .map(|row| StoppedContainer {
                id: field(&row, "ID"),
                name: field(&row, "Names"),
                // "12kB (virtual 1.2GB)": the writable layer is what removal frees.
                size_bytes: parse_size(field(&row, "Size").split(' ').next().unwrap_or("0B")),
            })
            .collect())
    }
    fn remove_container(&self, id: &str) -> Result<(), String> {
        self.output(&["rm", id]).map(|_| ())
    }
    fn unused_volumes(&self) -> Result<Vec<String>, String> {
        let names = self.output(&["volume", "ls", "-q", "-f", "dangling=true"])?;
        Ok(names
            .lines()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .collect())
    }
}
fn json_rows(text: &str) -> impl Iterator<Item = serde_json::Value> + '_ {
    text.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
}
fn field(row: &serde_json::Value, key: &str) -> String {
    row.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned()
}
pub(crate) fn parse_size(value: &str) -> u64 {
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
