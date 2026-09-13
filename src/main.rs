use clap::{Parser, Subcommand};
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
    Create(VmArgs),
    #[command(about = "Lists all VMs")]
    List,
    #[command(about = "Updates a VM by name")]
    Update(VmArgs),
    #[command(about = "Deletes a VM by name")]
    Delete { name: String },
    #[command(about = "Runs a stopped vm")]
    Run { name: String },
    #[command(about = "Gracefully shuts down a running VM")]
    Stop { name: String },
}

#[derive(Debug, Parser)]
struct VmArgs {
    name: String,
    #[arg(long)]
    cpu: Option<i32>,
    #[arg(long)]
    memory: Option<i32>,
    #[arg(long, help = "Disk size in GB (default 10)")]
    disk: Option<i32>,
}

impl Commands {
    pub fn execute(&self) -> anyhow::Result<()> {
        let mut conn = establish_connection()?;
        match self {
            Commands::Create(args) => {
                let vm = commands::create(
                    &mut conn,
                    CreateVm {
                        name: args.name.clone(),
                        cpu: args.cpu.unwrap_or(1),
                        memory: args.memory.unwrap_or(500),
                        disk: args.disk.unwrap_or(10),
                    },
                )?;
                println!("created {} ({})", vm.name, vm.ip_address);
            }
            Commands::Delete { name } => {
                commands::delete(&mut conn, name)?;
                println!("deleted {name}");
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
                    let status = format!("{:?}", vm.status).to_lowercase();
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
                let vm = commands::update(&mut conn, &args.name, changes)?;
                println!("updated {}", vm.name);
            }
            Commands::Run { name } => {
                commands::run(&mut conn, name)?;
                println!("started {name}");
            }
            Commands::Stop { name } => {
                commands::stop(&mut conn, name)?;
                println!("stopping {name}");
            }
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.command.execute()
}
