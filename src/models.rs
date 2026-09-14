use std::fmt::{self};

use crate::schema::{sql_types::VmStatus as VmStatusSql, vms};
use colored::Colorize;
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

impl fmt::Display for VmStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => write!(f, "{}", "running".green()),
            Self::Stopped => write!(f, "{}", "stoppped".red()),
            Self::Suspended => write!(f, "{}", "suspended".yellow()),
        }
    }
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
    pub ip_address: String,
    pub disk: i32,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = vms)]
pub struct NewVm {
    pub name: String,
    pub cpu: i32,
    pub memory: i32,
    pub status: VmStatus,
    pub ip_address: String,
    pub disk: i32,
}

#[derive(Debug)]
pub struct CreateVm {
    pub name: String,
    pub cpu: i32,
    pub memory: i32,
    pub disk: i32,
    pub user: String,
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = vms)]
pub struct VmUpdate {
    pub cpu: Option<i32>,
    pub memory: Option<i32>,
    pub disk: Option<i32>,
}

impl VmUpdate {
    pub fn is_empty(&self) -> bool {
        self.cpu.is_none() && self.memory.is_none() && self.disk.is_none()
    }
}
