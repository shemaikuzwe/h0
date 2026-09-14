use crate::models::{CreateVm, NewVm, Vm, VmStatus, VmUpdate};
use crate::schema::vms::dsl as vm;
use crate::{libvirt, provision};
use anyhow::Context;
use diesel::PgConnection;
use diesel::prelude::*;

pub fn create(conn: &mut PgConnection, spec: CreateVm) -> anyhow::Result<Vm> {
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
        provision::create_files(&created, &spec.user, &password)?;
        let lv = libvirt::connect()?;
        lv.define(&created)?;
        lv.reserve_ip(&created)?;
        run(conn, &created.name)?;
        Ok(created)
    })
}

pub fn list(conn: &mut PgConnection) -> anyhow::Result<Vec<Vm>> {
    let all = vm::vms.order(vm::id.asc()).load::<Vm>(conn)?;
    Ok(all)
}

pub fn update(
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
        let lv = libvirt::connect()?;
        if changes.disk.is_some() {
            lv.resize_disk(&updated)?;
        }
        // new cpu/memory apply on next boot
        lv.define(&updated)?;
        if reboot {
            lv.reboot(name)?;
        }
        Ok(updated)
    })
}

pub fn run(conn: &mut PgConnection, name: &str) -> anyhow::Result<()> {
    libvirt::connect()?.start(name)?;
    set_status(conn, name, VmStatus::Running)
}
pub fn reboot(name: &str) -> anyhow::Result<()> {
    libvirt::connect()?.reboot(name)?;
    Ok(())
}

pub fn stop(conn: &mut PgConnection, name: &str) -> anyhow::Result<()> {
    libvirt::connect()?.shutdown(name)?;
    set_status(conn, name, VmStatus::Stopped)
}

pub fn delete(conn: &mut PgConnection, name: &str) -> anyhow::Result<()> {
    let target = vm::vms.filter(vm::name.eq(name));
    let existing: Vm = target
        .select(Vm::as_select())
        .first(conn)
        .with_context(|| format!("no VM found with name '{name}'"))?;
    let lv = libvirt::connect()?;
    lv.remove(name)?;
    lv.release_ip(&existing)?;
    provision::remove_files(name)?;
    diesel::delete(target).execute(conn)?;
    Ok(())
}

fn set_status(conn: &mut PgConnection, name: &str, status: VmStatus) -> anyhow::Result<()> {
    diesel::update(vm::vms.filter(vm::name.eq(name)))
        .set(vm::status.eq(status))
        .execute(conn)?;
    Ok(())
}
