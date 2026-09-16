pub mod zfs;

use crate::models::Image;
use std::path::PathBuf;

/// How libvirt should reference a VM disk.
pub enum Disk {
    Block(PathBuf),
    File { path: PathBuf, format: &'static str },
}

pub struct Snapshot {
    pub name: String,
    pub created_at: chrono::NaiveDateTime,
}

pub trait Storage {
    fn create_disk(&self, vm: &str, image: Image, size_gb: i32) -> anyhow::Result<Disk>;
    fn disk(&self, vm: &str) -> Disk;
    fn resize(&self, vm: &str, size_gb: i32) -> anyhow::Result<()>;
    fn delete(&self, vm: &str) -> anyhow::Result<()>;

    fn snapshot(&self, vm: &str, name: &str) -> anyhow::Result<()>;
    fn snapshots(&self, vm: &str) -> anyhow::Result<Vec<Snapshot>>;
    fn revert(&self, vm: &str, name: &str) -> anyhow::Result<()>;
    fn delete_snapshot(&self, vm: &str, name: &str) -> anyhow::Result<()>;

    /// Writes a self-contained copy of the disk to `file`; returns its size in bytes.
    fn backup(&self, vm: &str, file: &str) -> anyhow::Result<i64>;
    /// Replaces the disk with the contents of `file` (VM must be stopped).
    fn restore(&self, vm: &str, file: &str) -> anyhow::Result<()>;
}

/// The only place a driver is named; a second driver becomes a match here when one exists.
pub fn open() -> impl Storage {
    zfs::Zfs
}
