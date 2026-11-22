//! Principle IV: protected-path guard tests, temp dirs only.

use std::fs;

use tempfile::TempDir;
use ttops_core::guard::Guard;
use ttops_core::platform::{Platform, TestPlatform};

fn setup() -> (TempDir, TestPlatform) {
    let dir = TempDir::new().unwrap();
    let p = TestPlatform::new(dir.path(), "");
    fs::create_dir_all(dir.path().join("system/core")).unwrap();
    fs::create_dir_all(p.home().join(".ssh")).unwrap();
    fs::create_dir_all(p.cache_dir().join("app")).unwrap();
    (dir, p)
}

#[test]
fn allows_ordinary_cache_dir() {
    let (_d, p) = setup();
    let g = Guard::new(&p);
    assert!(g.check(&p.cache_dir().join("app")).is_ok());
}

#[test]
fn refuses_protected_root_and_children() {
    let (d, p) = setup();
    let g = Guard::new(&p);
    assert!(g.check(&d.path().join("system")).is_err());
    assert!(g.check(&d.path().join("system/core")).is_err());
    assert!(g.check(&p.home().join(".ssh")).is_err());
}

#[test]
fn refuses_ancestors_of_protected_roots() {
    let (d, p) = setup();
    let g = Guard::new(&p);
    assert!(g.check(&p.home()).is_err(), "home contains .ssh");
    assert!(g.check(d.path()).is_err());
    assert!(g.check(std::path::Path::new("/")).is_err());
}

#[test]
fn refuses_case_variants() {
    let (_d, p) = setup();
    let g = Guard::new(&p);
    assert!(g.check(&p.home().join(".SSH")).is_err());
}

#[cfg(unix)]
#[test]
fn refuses_symlink_into_protected_root() {
    let (d, p) = setup();
    let link = p.cache_dir().join("sneaky");
    std::os::unix::fs::symlink(d.path().join("system/core"), &link).unwrap();
    let g = Guard::new(&p);
    assert!(g.check(&link).is_err());
}

#[test]
fn refuses_credential_name_patterns() {
    let (_d, p) = setup();
    let g = Guard::new(&p);
    for name in [
        "login.keychain-db",
        "id_rsa",
        "id_rsa.pub",
        "id_ed25519",
        "server.pem",
        "credentials",
        ".env",
        "Example.app",
    ] {
        let path = p.cache_dir().join("app").join(name);
        fs::write(&path, b"x").unwrap();
        assert!(g.check(&path).is_err(), "{name} must be refused");
    }
}

#[test]
fn nonexistent_path_is_checked_literally() {
    let (d, p) = setup();
    let g = Guard::new(&p);
    assert!(g.check(&d.path().join("system/missing")).is_err());
    assert!(g.check(&p.cache_dir().join("missing")).is_ok());
}
