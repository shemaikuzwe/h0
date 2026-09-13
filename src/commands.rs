use crate::models::{CreateVm, NewVm, Vm, VmStatus, VmUpdate};
use crate::schema::vms::dsl as vm;
use crate::{libvirt, provision};
use anyhow::Context;
use diesel::prelude::*;

// 192.168.122.100-254 on libvirt's default network
const IP_POOL: std::ops::RangeInclusive<u8> = 100..=254;

fn next_free_ip(conn: &mut PgConnection) -> anyhow::Result<String> {
    let used: Vec<String> = vm::vms.select(vm::ip_address).load(conn)?;
    IP_POOL
        .map(|n| format!("192.168.122.{n}"))
        .find(|ip| !used.contains(ip))
        .context("no free IP addresses left")
}

pub fn create(conn: &mut PgConnection, spec: CreateVm) -> anyhow::Result<Vm> {
    let new_vm = NewVm {
        name: spec.name,
        cpu: spec.cpu,
        memory: spec.memory,
        disk: spec.disk,
        status: VmStatus::Stopped,
        ip_address: next_free_ip(conn)?,
    };
    let created = diesel::insert_into(vm::vms)
        .values(&new_vm)
        .get_result::<Vm>(conn)?;
    provision::create_files(&created)?;
    let lv = libvirt::connect()?;
    lv.define(&created)?;
    lv.reserve_ip(&created)?;
    Ok(created)
}

pub fn list(conn: &mut PgConnection) -> anyhow::Result<Vec<Vm>> {
    let all = vm::vms.order(vm::id.asc()).load::<Vm>(conn)?;
    Ok(all)
}

pub fn update(conn: &mut PgConnection, name: &str, changes: VmUpdate) -> anyhow::Result<Vm> {
    let target = vm::vms.filter(vm::name.eq(name));
    // diesel rejects an empty changeset, so just return the current row
    if changes.is_empty() {
        return Ok(target.select(Vm::as_select()).first(conn)?);
    }
    let updated: Vm = diesel::update(target).set(&changes).get_result(conn)?;
    if changes.disk.is_some() {
        provision::resize_disk(&updated)?;
    }
    // new cpu/memory apply on next boot
    libvirt::connect()?.define(&updated)?;
    Ok(updated)
}

pub fn run(conn: &mut PgConnection, name: &str) -> anyhow::Result<()> {
    libvirt::connect()?.start(name)?;
    set_status(conn, name, VmStatus::Running)
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
