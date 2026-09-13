use crate::schema::{sql_types::VmStatus as VmStatusSql, vms};
use diesel::prelude::*;
use diesel_derive_enum::DbEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, DbEnum)]
#[ExistingTypePath = "VmStatusSql"]
#[DbValueStyle = "verbatim"]
pub enum VmStatus {
    #[db_rename = "running"]
    Running,
    #[db_rename = "stopped"]
    Stopped,
    #[db_rename = "suspended"]
    Suspended,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = vms)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Vm {
    pub id: i32,
    pub name: String,
    pub cpu: i32,
    pub memory: i32,
    pub status: VmStatus,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = vms)]
pub struct NewVm {
    pub name: String,
    pub cpu: i32,
    pub memory: i32,
    pub status: VmStatus,
}
