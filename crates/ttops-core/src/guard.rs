//! Deny cleanup of protected or structurally unsafe paths.

use std::path::{Component, Path, PathBuf};

use crate::platform::Platform;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("protected path: {0}")]
pub struct Protected(pub PathBuf);

/// A platform-specific protected-root guard. It deliberately rejects both a
/// protected root's descendants and its ancestors, so callers cannot clean a
/// parent directory that contains credentials or system content.
pub struct Guard {
    home: PathBuf,
    protected_roots: Vec<PathBuf>,
}

impl Guard {
    pub fn new(platform: &dyn Platform) -> Self {
        let home = normalize_existing_or_lexical(&platform.home());
        let protected_roots = platform
            .protected_roots()
            .into_iter()
            .map(|path| normalize_existing_or_lexical(&path))
            .collect();
        Self {
            home,
            protected_roots,
        }
    }

    pub fn check(&self, path: &Path) -> Result<(), Protected> {
        let checked = normalize_existing_or_lexical(path);
        if is_volume_root(&checked)
            || same_path(&checked, &self.home)
            || self
                .protected_roots
                .iter()
                .any(|root| overlaps(&checked, root))
            || checked.components().any(protected_component)
        {
            return Err(Protected(checked));
        }
        Ok(())
    }

    /// True when every path below `dir` is rejected by [`Guard::check`]: `dir`
    /// equals or lies inside a protected root, or has a protected component.
    /// Unlike `check`, an ancestor of a protected root (such as the home folder)
    /// is not a protected subtree, so walkers may still descend into it.
    pub fn protects_subtree(&self, dir: &Path) -> bool {
        let checked = normalize_existing_or_lexical(dir);
        self.protected_roots
            .iter()
            .any(|root| path_starts_with(&checked, root))
            || checked.components().any(protected_component)
    }
}

impl Guard {
    /// The App Uninstaller exception (constitution 1.2.0): accepts only a real
    /// (non-symlink) `.app` directory that is a direct child of an applications
    /// folder. Every other protected root and component rule still applies.
    pub fn check_app_bundle(&self, path: &Path, app_dirs: &[PathBuf]) -> Result<(), Protected> {
        let refuse = || Err(Protected(path.to_path_buf()));
        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            return refuse();
        };
        if !metadata.is_dir()
            || !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
        {
            return refuse();
        }
        let checked = normalize_existing_or_lexical(path);
        let Some(parent) = checked.parent() else {
            return refuse();
        };
        let app_dirs: Vec<PathBuf> = app_dirs
            .iter()
            .map(|dir| normalize_existing_or_lexical(dir))
            .collect();
        if !app_dirs.iter().any(|dir| same_path(parent, dir)) {
            return refuse();
        }
        let other_roots_overlap = self
            .protected_roots
            .iter()
            .filter(|root| !app_dirs.iter().any(|dir| same_path(root, dir)))
            .any(|root| overlaps(&checked, root));
        let parent_components_protected = parent.components().any(protected_component);
        if other_roots_overlap || parent_components_protected || same_path(&checked, &self.home) {
            return refuse();
        }
        Ok(())
    }
}

fn normalize_existing_or_lexical(path: &Path) -> PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }

    // Canonicalise the nearest existing ancestor, then restore the missing tail.
    // This catches a request such as `/tmp-link/system/missing` even when the
    // final child does not exist yet.
    let mut ancestor = path.to_path_buf();
    let mut missing = Vec::new();
    while !ancestor.exists() {
        let Some(name) = ancestor.file_name().map(|name| name.to_os_string()) else {
            return lexical_normalize(path);
        };
        missing.push(name);
        if !ancestor.pop() {
            return lexical_normalize(path);
        }
    }
    let Ok(mut resolved) = std::fs::canonicalize(&ancestor) else {
        return lexical_normalize(path);
    };
    for component in missing.iter().rev() {
        resolved.push(component);
    }
    resolved
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    };
    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn overlaps(left: &Path, right: &Path) -> bool {
    path_starts_with(left, right) || path_starts_with(right, left)
}

fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    let path: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    let prefix: Vec<String> = prefix
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    path.starts_with(&prefix)
}

fn same_path(left: &Path, right: &Path) -> bool {
    path_starts_with(left, right) && path_starts_with(right, left)
}

fn is_volume_root(path: &Path) -> bool {
    path.parent().is_none()
}

fn protected_component(component: Component<'_>) -> bool {
    let Component::Normal(name) = component else {
        return false;
    };
    let name = name.to_string_lossy().to_ascii_lowercase();
    name == "credentials"
        || name == ".env"
        || name.ends_with(".pem")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || name.contains(".keychain")
        || name.ends_with(".app")
}
