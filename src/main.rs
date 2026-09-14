use clap::{Parser, Subcommand};
use colored::Colorize;
use hostv1::{
    commands, establish_connection,
    models::{CreateVm, VmUpdate},
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
    #[command(about = "Lists all VMs")]
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
}

impl Commands {
    pub fn execute(&self) -> anyhow::Result<()> {
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
                    },
                )?;
                println!(
                    "created {}: ssh {}@{}",
                    vm.name.green(),
                    args.user.blue(),
                    vm.ip_address.blue()
                );
            }
            Commands::Delete { name } => {
                commands::delete(&mut conn, name)?;
                println!("deleted {}", name.red());
            }
            Commands::List => {
                let all = commands::list(&mut conn)?;
                if all.is_empty() {
                    println!("no VMs");
                    return Ok(());
                }
                println!(
                    "{:<10}{:<8}{:<10}{:<8}{:<16}{}",
                    "NAME", "CPUS", "MEMORY", "DISK", "IP", "STATUS"
                );
                for vm in all {
                    let memory = format!("{} MB", vm.memory);
                    let disk = format!("{} GB", vm.disk);
                    let status = format!("{}", vm.status.to_string());
                    println!(
                        "{:<10}{:<8}{:<10}{:<8}{:<16}{}",
                        vm.name, vm.cpu, memory, disk, vm.ip_address, status
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
                    commands::update(&mut conn, &args.name, changes, args.reboot.unwrap_or(true))?;
                println!("updated {}", vm.name.green());
            }
            Commands::Run { name } => {
                commands::run(&mut conn, name)?;
                println!("started {}", name.green());
            }
            Commands::Stop { name } => {
                commands::stop(&mut conn, name)?;
                println!("stopping {}", name.red());
            }
            Commands::Reboot { name } => {
                commands::reboot(name)?;
                println!("Rebooted {}", name.green())
            }
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.command.execute()
}
