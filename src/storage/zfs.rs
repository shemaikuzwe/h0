use super::{Disk, Snapshot, Storage};
use crate::models::Image;
use anyhow::Context;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub const POOL: &str = "tank";

pub struct Zfs;

impl Storage for Zfs {
    fn create_disk(&self, vm: &str, image: Image, size_gb: i32) -> anyhow::Result<Disk> {
        let base = format!("{POOL}/images/{image}@base");
        zfs([
            "clone",
            "-o",
            &format!("volsize={size_gb}G"),
            &base,
            &vol(vm),
        ])?;
        settle()?;
        Ok(self.disk(vm))
    }

    fn disk(&self, vm: &str) -> Disk {
        Disk::Block(PathBuf::from(format!("/dev/zvol/{}", vol(vm))))
    }

    fn resize(&self, vm: &str, size_gb: i32) -> anyhow::Result<()> {
        zfs(["set", &format!("volsize={size_gb}G"), &vol(vm)])
    }

    fn delete(&self, vm: &str) -> anyhow::Result<()> {
        zfs(["destroy", "-r", &vol(vm)])
    }

    fn snapshot(&self, vm: &str, name: &str) -> anyhow::Result<()> {
        zfs(["snapshot", &format!("{}@{name}", vol(vm))])
    }

    fn snapshots(&self, vm: &str) -> anyhow::Result<Vec<Snapshot>> {
        // -H no header, -p raw epoch seconds, -d 1 this dataset only, -s creation oldest first
        let out = output(&[
            "zfs",
            "list",
            "-H",
            "-p",
            "-t",
            "snapshot",
            "-d",
            "1",
            "-o",
            "name,creation",
            "-s",
            "creation",
            &vol(vm),
        ])?;
        out.lines()
            .filter_map(|l| l.split_once('\t'))
            .filter(|(name, _)| !name.contains("@bk-"))
            .map(|(name, secs)| {
                Ok(Snapshot {
                    name: name
                        .split_once('@')
                        .context("bad snapshot name")?
                        .1
                        .to_owned(),
                    created_at: chrono::DateTime::from_timestamp(secs.parse()?, 0)
                        .context("bad creation time")?
                        .naive_local(),
                })
            })
            .collect()
    }

    fn revert(&self, vm: &str, name: &str) -> anyhow::Result<()> {
        // -r also destroys every snapshot newer than `name`
        zfs(["rollback", "-r", &format!("{}@{name}", vol(vm))])
    }

    fn delete_snapshot(&self, vm: &str, name: &str) -> anyhow::Result<()> {
        zfs(["destroy", &format!("{}@{name}", vol(vm))])
    }

    fn backup(&self, vm: &str, file: &str) -> anyhow::Result<i64> {
        let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
        let snap = format!("{}@bk-{ts}", vol(vm));
        zfs(["snapshot", &snap])?;
        let result = pipe(
            &["sudo", "-n", "zfs", "send", &snap],
            &["zstd", "-q", "-o", file],
        );
        zfs(["destroy", &snap])?;
        result?;
        Ok(i64::try_from(std::fs::metadata(file)?.len())?)
    }

    fn restore(&self, vm: &str, file: &str) -> anyhow::Result<()> {
        // a full stream can't land on an existing dataset; the clone is disposable
        zfs(["destroy", "-r", &vol(vm)])?;
        pipe(
            &["zstd", "-d", "-c", file],
            &["sudo", "-n", "zfs", "receive", &vol(vm)],
        )?;
        settle()
    }
}

fn zfs<const N: usize>(args: [&str; N]) -> anyhow::Result<()> {
    let mut full = vec!["sudo", "-n", "zfs"];
    full.extend(args);
    output(&full).map(drop)
}

/// udev creates /dev/zvol/* asynchronously; wait so libvirt can open the disk right away.
fn settle() -> anyhow::Result<()> {
    output(&["udevadm", "settle"]).map(drop)
}

fn output(cmd: &[&str]) -> anyhow::Result<String> {
    let out = Command::new(cmd[0])
        .args(&cmd[1..])
        .output()
        .with_context(|| format!("running {}", cmd[0]))?;
    anyhow::ensure!(
        out.status.success(),
        "{} failed: {}",
        cmd.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?)
}

/// `a | b`, failing if either side fails.
fn pipe(a: &[&str], b: &[&str]) -> anyhow::Result<()> {
    let mut left = Command::new(a[0])
        .args(&a[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("running {}", a[0]))?;
    let right = Command::new(b[0])
        .args(&b[1..])
        .stdin(left.stdout.take().context("no stdout")?)
        .output()
        .with_context(|| format!("running {}", b[0]))?;
    let left = left.wait_with_output()?;
    anyhow::ensure!(
        left.status.success(),
        "{} failed: {}",
        a.join(" "),
        String::from_utf8_lossy(&left.stderr)
    );
    anyhow::ensure!(
        right.status.success(),
        "{} failed: {}",
        b.join(" "),
        String::from_utf8_lossy(&right.stderr)
    );
    Ok(())
}

fn vol(vm: &str) -> String {
    format!("{POOL}/vms/{vm}")
}
