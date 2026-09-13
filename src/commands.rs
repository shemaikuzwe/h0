use crate::models::{NewVm, Vm};
use crate::schema::vms::dsl as vm;
use diesel::prelude::*;

pub fn create(conn: &mut PgConnection, new_vm: NewVm) -> anyhow::Result<Vm> {
    let created = diesel::insert_into(vm::vms)
        .values(&new_vm)
        .get_result::<Vm>(conn)?;
    Ok(created)
}

pub fn list(conn: &mut PgConnection) -> anyhow::Result<Vec<Vm>> {
    let all = vm::vms.order(vm::id.asc()).load::<Vm>(conn)?;
    Ok(all)
}

pub fn update(
    conn: &mut PgConnection,
    name: &str,
    cpu: Option<i32>,
    memory: Option<i32>,
) -> anyhow::Result<Vm> {
    let target = vm::vms.filter(vm::name.eq(name));
    let updated = match (cpu, memory) {
        (Some(c), Some(m)) => diesel::update(target)
            .set((vm::cpu.eq(c), vm::memory.eq(m)))
            .get_result(conn)?,
        (Some(c), None) => diesel::update(target).set(vm::cpu.eq(c)).get_result(conn)?,
        (None, Some(m)) => diesel::update(target)
            .set(vm::memory.eq(m))
            .get_result(conn)?,
        (None, None) => target.select(Vm::as_select()).first(conn)?,
    };
    Ok(updated)
}

pub fn delete(conn: &mut PgConnection, name: &str) -> anyhow::Result<usize> {
    let count = diesel::delete(vm::vms.filter(vm::name.eq(name))).execute(conn)?;
    if count == 0 {
        anyhow::bail!("no VM found with name '{name}'");
    }
    Ok(count)
}
