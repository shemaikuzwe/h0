use std::fmt::{self};

use crate::apps::App;
use crate::schema::{sql_types::VmImage as VmImageSql, sql_types::VmStatus as VmStatusSql, vms};
use clap::ValueEnum;
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
            Self::Stopped => write!(f, "{}", "stopped".red()),
            Self::Suspended => write!(f, "{}", "suspended".yellow()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, DbEnum)]
#[ExistingTypePath = "VmImageSql"]
#[DbValueStyle = "verbatim"]
pub enum Image {
    #[db_rename = "ubuntu24"]
    Ubuntu24,
    #[db_rename = "ubuntu22"]
    Ubuntu22,
    #[db_rename = "centos10"]
    Centos10,
    #[db_rename = "kali"]
    Kali,
}
impl fmt::Display for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Image::Ubuntu24 => f.pad("ubuntu24"),
            Image::Ubuntu22 => f.pad("ubuntu22"),
            Image::Centos10 => f.pad("centos10"),
            Image::Kali => f.pad("kali"),
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
    pub image: Image,
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
    pub image: Image,
}

#[derive(Debug)]
pub struct CreateVm {
    pub name: String,
    pub cpu: i32,
    pub memory: i32,
    pub disk: i32,
    pub user: String,
    pub image: Image,
    pub apps: Vec<App>,
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
