//! TTops core: platform layer, rule catalogs, scanning, guarded cleanup, and audit log.
//! Independent of Tauri so every rule and side effect is testable on temp dirs.

pub mod apps;
pub mod audit;
pub mod cleaner;
pub mod container;
pub mod duplicates;
pub mod fsutil;
pub mod guard;
pub mod model;
pub mod platform;
pub mod rules;
pub mod scanner;
pub mod settings;
pub mod startup;
pub mod tools;
