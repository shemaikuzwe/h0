use crate::models::{Backup, CreateVm, NewBackup, NewVm, Vm, VmStatus, VmUpdate};
use crate::schema::backups::dsl as bk;
use crate::schema::vms::dsl as vm;
use crate::storage::{self, Storage};
use crate::{apps, libvirt, provision};
use anyhow::Context;
use diesel::PgConnection;
use diesel::prelude::*;

pub async fn create(conn: &mut PgConnection, spec: CreateVm) -> anyhow::Result<Vm> {
    let init = apps::cloud_init(&spec.apps, spec.image, &spec.user)?;
    let password = rpassword::prompt_password("User password: ").context("Password is required")?;
    let new_vm = NewVm {
        name: spec.name,
        cpu: spec.cpu,
        memory: spec.memory,
        disk: spec.disk,
        status: VmStatus::Stopped,
        ip_address: provision::next_free_ip(conn)?,
        image: spec.image,
    };
    conn.transaction(|conn| {
        let created = diesel::insert_into(vm::vms)
            .values(&new_vm)
            .get_result::<Vm>(conn)?;
        let disk = storage::open().create_disk(&created.name, created.image, created.disk)?;
        provision::create_files(&created, &spec.user, &password, &init)?;
        let lv = libvirt::connect()?;
        lv.define(&created, &disk)?;
        lv.reserve_ip(&created)?;
        start(conn, &created.name)?;
        Ok(created)
    })
}

pub async fn list(conn: &mut PgConnection) -> anyhow::Result<Vec<Vm>> {
    let all = vm::vms.order(vm::id.asc()).load::<Vm>(conn)?;
    Ok(all)
}

pub async fn update(
    conn: &mut PgConnection,
    name: &str,
    changes: VmUpdate,
    reboot: bool,
) -> anyhow::Result<Vm> {
    let target = vm::vms.filter(vm::name.eq(name));
    // diesel rejects an empty changeset, so just return the current row
    if changes.is_empty() {
        return Ok(target.select(Vm::as_select()).first(conn)?);
    }
    conn.transaction(|conn| {
        let updated: Vm = diesel::update(target).set(&changes).get_result(conn)?;
        let storage = storage::open();
        let lv = libvirt::connect()?;
        if changes.disk.is_some() {
            storage.resize(name, updated.disk)?;
            lv.resize_disk(&updated)?;
        }
        // new cpu/memory apply on next boot
        lv.define(&updated, &storage.disk(name))?;
        if reboot {
            lv.reboot(name)?;
        }
        Ok(updated)
    })
}

pub async fn run(conn: &mut PgConnection, name: &str) -> anyhow::Result<()> {
    start(conn, name)
}
pub async fn reboot(name: &str) -> anyhow::Result<()> {
    libvirt::connect()?.reboot(name)?;
    Ok(())
}

pub async fn stop(conn: &mut PgConnection, name: &str) -> anyhow::Result<()> {
    libvirt::connect()?.shutdown(name)?;
    set_status(conn, name, VmStatus::Stopped)
}

pub async fn delete(conn: &mut PgConnection, name: &str) -> anyhow::Result<()> {
    let existing = find_vm(conn, name)?;
    let lv = libvirt::connect()?;
    lv.remove(name)?;
    lv.release_ip(&existing)?;
    storage::open().delete(name)?;
    provision::remove_files(name)?;
    // backups rows cascade
    diesel::delete(vm::vms.filter(vm::name.eq(name))).execute(conn)?;
    Ok(())
}

pub async fn snapshot_create(
    conn: &mut PgConnection,
    name: &str,
    snap: &str,
) -> anyhow::Result<()> {
    find_vm(conn, name)?;
    let lv = libvirt::connect()?;
    // no guest agent → crash-consistent snapshot, still fine for journaling filesystems
    let frozen = lv.is_active(name)? && lv.fs_freeze(name).is_ok();
    let result = storage::open().snapshot(name, snap);
    if frozen {
        lv.fs_thaw(name)?;
    }
    result
}

pub async fn snapshot_list(
    conn: &mut PgConnection,
    name: &str,
) -> anyhow::Result<Vec<storage::Snapshot>> {
    find_vm(conn, name)?;
    storage::open().snapshots(name)
}

pub async fn snapshot_revert(
    conn: &mut PgConnection,
    name: &str,
    snap: &str,
) -> anyhow::Result<()> {
    find_vm(conn, name)?;
    ensure_stopped(name)?;
    storage::open().revert(name, snap)
}

pub async fn snapshot_delete(
    conn: &mut PgConnection,
    name: &str,
    snap: &str,
) -> anyhow::Result<()> {
    find_vm(conn, name)?;
    storage::open().delete_snapshot(name, snap)
}

pub async fn backup_create(conn: &mut PgConnection, name: &str) -> anyhow::Result<Backup> {
    let existing = find_vm(conn, name)?;
    let file = provision::backup_path(name)?;
    let size_bytes = storage::open().backup(name, &file)?;
    let new_backup = NewBackup {
        vm_id: existing.id,
        file,
        size_bytes,
    };
    Ok(diesel::insert_into(bk::backups)
        .values(&new_backup)
        .get_result(conn)?)
}

pub async fn backup_list(conn: &mut PgConnection, name: &str) -> anyhow::Result<Vec<Backup>> {
    let existing = find_vm(conn, name)?;
    Ok(bk::backups
        .filter(bk::vm_id.eq(existing.id))
        .order(bk::id.asc())
        .load(conn)?)
}

pub async fn backup_restore(conn: &mut PgConnection, name: &str, id: i32) -> anyhow::Result<()> {
    let backup = find_backup(conn, name, id)?;
    ensure_stopped(name)?;
    storage::open().restore(name, &backup.file)
}

pub async fn backup_delete(conn: &mut PgConnection, name: &str, id: i32) -> anyhow::Result<()> {
    let backup = find_backup(conn, name, id)?;
    std::fs::remove_file(&backup.file)?;
    diesel::delete(bk::backups.filter(bk::id.eq(id))).execute(conn)?;
    Ok(())
}

fn find_vm(conn: &mut PgConnection, name: &str) -> anyhow::Result<Vm> {
    vm::vms
        .filter(vm::name.eq(name))
        .select(Vm::as_select())
        .first(conn)
        .with_context(|| format!("no VM found with name '{name}'"))
}

fn find_backup(conn: &mut PgConnection, name: &str, id: i32) -> anyhow::Result<Backup> {
    let existing = find_vm(conn, name)?;
    bk::backups
        .filter(bk::id.eq(id).and(bk::vm_id.eq(existing.id)))
        .select(Backup::as_select())
        .first(conn)
        .with_context(|| format!("no backup {id} for '{name}'"))
}

fn ensure_stopped(name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !libvirt::connect()?.is_active(name)?,
        "'{name}' is running, stop it first"
    );
    Ok(())
}

// sync so create() can call it inside its transaction
fn start(conn: &mut PgConnection, name: &str) -> anyhow::Result<()> {
    libvirt::connect()?.start(name)?;
    set_status(conn, name, VmStatus::Running)
}

fn set_status(conn: &mut PgConnection, name: &str, status: VmStatus) -> anyhow::Result<()> {
    diesel::update(vm::vms.filter(vm::name.eq(name)))
        .set(vm::status.eq(status))
        .execute(conn)?;
    Ok(())
}
