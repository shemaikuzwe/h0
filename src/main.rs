use clap::{Parser, Subcommand};
use colored::Colorize;
use h0::{
    apps::App,
    commands, console, establish_connection,
    models::{CreateVm, Image, VmUpdate},
};

#[derive(Parser)]
#[command(version = "1.0")]
#[command(about = "Create and manage VMs", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Creates a VM (default cpu 1, memory 500MB, disk 10GB)")]
    Create(CreateVmArgs),
    #[command(about = "Lists all VMs", alias = "ls")]
    List,
    #[command(about = "Updates a VM by name")]
    Update(UpdateVmArgs),
    #[command(about = "Deletes a VM by name")]
    Delete { name: String },
    #[command(about = "Runs a stopped vm")]
    Run { name: String },
    #[command(about = "Gracefully shuts down a running VM")]
    Stop { name: String },
    #[command(about = "Reboots vm")]
    Reboot { name: String },
    #[command(about = "Opens a web console attached to the VM's serial port")]
    Console {
        name: String,
        #[arg(long, help = "Port to bind server")]
        port: Option<u32>,
    },
    #[command(about = "Manages disk snapshots (revert requires a stopped VM)")]
    Snapshot {
        #[command(subcommand)]
        cmd: SnapshotCmd,
    },
    #[command(about = "Manages disk backups (restore requires a stopped VM)")]
    Backup {
        #[command(subcommand)]
        cmd: BackupCmd,
    },
}

#[derive(Debug, Subcommand)]
enum SnapshotCmd {
    Create {
        vm: String,
        name: String,
    },
    #[command(alias = "ls")]
    List {
        vm: String,
    },
    Revert {
        vm: String,
        name: String,
    },
    Delete {
        vm: String,
        name: String,
    },
}

#[derive(Debug, Subcommand)]
enum BackupCmd {
    Create {
        vm: String,
    },
    #[command(alias = "ls")]
    List {
        vm: String,
    },
    Restore {
        vm: String,
        id: i32,
    },
    Delete {
        vm: String,
        id: i32,
    },
}

#[derive(Debug, Parser)]
struct UpdateVmArgs {
    name: String,
    #[arg(long, help = "VCPU size (defualt 1)")]
    cpu: Option<i32>,
    #[arg(long, help = "Memory size (default 500MB)")]
    memory: Option<i32>,
    #[arg(long, help = "Disk size in GB (default 10)")]
    disk: Option<i32>,
    #[arg(long, help = "Reboot VM for the changes to take effect.(default True)")]
    reboot: Option<bool>,
}

#[derive(Debug, Parser)]
struct CreateVmArgs {
    name: String,
    #[arg(long, help = "VCPU size (defualt 1)")]
    cpu: Option<i32>,
    #[arg(long, help = "Memory size (default 500MB)")]
    memory: Option<i32>,
    #[arg(long, help = "Disk size in GB (default 10)")]
    disk: Option<i32>,
    #[arg(long, help = "Default non-root VM user ")]
    user: String,
    #[arg(long, value_enum, default_value_t = Image::Ubuntu24, help = "Base image")]
    image: Image,
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        help = "Apps to install on first boot (docker,nginx,caddy)"
    )]
    apps: Vec<App>,
}

impl Commands {
    pub async fn execute(&self) -> anyhow::Result<()> {
        let mut conn = establish_connection()?;
        match self {
            Commands::Create(args) => {
                let vm = commands::create(
                    &mut conn,
                    CreateVm {
                        name: args.name.to_owned(),
                        cpu: args.cpu.unwrap_or(1),
                        memory: args.memory.unwrap_or(500),
                        disk: args.disk.unwrap_or(10),
                        user: args.user.to_owned(),
                        image: args.image,
                        apps: args.apps.clone(),
                    },
                )
                .await?;
                println!(
                    "created {}: ssh {}@{}",
                    vm.name.green(),
                    args.user.blue(),
                    vm.ip_address.blue()
                );
            }
            Commands::Delete { name } => {
                commands::delete(&mut conn, name).await?;
                println!("deleted {}", name.red());
            }
            Commands::List => {
                let all = commands::list(&mut conn).await?;
                if all.is_empty() {
                    println!("no VMs");
                    return Ok(());
                }
                println!(
                    "{:<10}{:<10}{:<8}{:<10}{:<8}{:<16}{}",
                    "NAME", "IMAGE", "CPUS", "MEMORY", "DISK", "IP", "STATUS"
                );
                for vm in all {
                    let memory = format!("{} MB", vm.memory);
                    let disk = format!("{} GB", vm.disk);
                    let status = format!("{}", vm.status);
                    println!(
                        "{:<10}{:<10}{:<8}{:<10}{:<8}{:<16}{}",
                        vm.name, vm.image, vm.cpu, memory, disk, vm.ip_address, status
                    );
                }
            }
            Commands::Update(args) => {
                let changes = VmUpdate {
                    cpu: args.cpu,
                    memory: args.memory,
                    disk: args.disk,
                };
                let vm =
                    commands::update(&mut conn, &args.name, changes, args.reboot.unwrap_or(true))
                        .await?;
                println!("updated {}", vm.name.green());
            }
            Commands::Run { name } => {
                commands::run(&mut conn, name).await?;
                println!("started {}", name.green());
            }
            Commands::Stop { name } => {
                commands::stop(&mut conn, name).await?;
                println!("stopping {}", name.red());
            }
            Commands::Reboot { name } => {
                commands::reboot(name).await?;
                println!("Rebooted {}", name.green())
            }
            Commands::Console { name, port } => console::serve(name, port.unwrap_or(8080)).await?,
            Commands::Snapshot { cmd } => match cmd {
                SnapshotCmd::Create { vm, name } => {
                    commands::snapshot_create(&mut conn, vm, name).await?;
                    println!("snapshot {} created", name.green());
                }
                SnapshotCmd::List { vm } => {
                    let all = commands::snapshot_list(&mut conn, vm).await?;
                    if all.is_empty() {
                        println!("no snapshots");
                        return Ok(());
                    }
                    println!("{:<20}{}", "NAME", "CREATED");
                    for s in all {
                        println!("{:<20}{}", s.name, s.created_at.format("%Y-%m-%d %H:%M:%S"));
                    }
                }
                SnapshotCmd::Revert { vm, name } => {
                    commands::snapshot_revert(&mut conn, vm, name).await?;
                    println!("reverted {} to {}", vm.green(), name.green());
                }
                SnapshotCmd::Delete { vm, name } => {
                    commands::snapshot_delete(&mut conn, vm, name).await?;
                    println!("snapshot {} deleted", name.red());
                }
            },
            Commands::Backup { cmd } => match cmd {
                BackupCmd::Create { vm } => {
                    let b = commands::backup_create(&mut conn, vm).await?;
                    println!("backup {} created: {}", b.id.to_string().green(), b.file);
                }
                BackupCmd::List { vm } => {
                    let all = commands::backup_list(&mut conn, vm).await?;
                    if all.is_empty() {
                        println!("no backups");
                        return Ok(());
                    }
                    println!("{:<6}{:<10}{:<22}{}", "ID", "SIZE", "CREATED", "FILE");
                    for b in all {
                        let size = format!("{} MB", b.size_bytes / 1_000_000);
                        let created = b.created_at.format("%Y-%m-%d %H:%M:%S");
                        println!("{:<6}{:<10}{:<22}{}", b.id, size, created, b.file);
                    }
                }
                BackupCmd::Restore { vm, id } => {
                    commands::backup_restore(&mut conn, vm, *id).await?;
                    println!("restored {} from backup {}", vm.green(), id);
                }
                BackupCmd::Delete { vm, id } => {
                    commands::backup_delete(&mut conn, vm, *id).await?;
                    println!("backup {} deleted", id.to_string().red());
                }
            },
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.command.execute().await
}
