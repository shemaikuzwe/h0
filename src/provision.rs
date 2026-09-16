use crate::apps::CloudInit;
use crate::models::Vm;
use crate::storage::Disk;
use anyhow::Context;
use diesel::PgConnection;
use diesel::prelude::*;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::schema::vms::dsl as vm;

pub const BASE: &str = "/var/lib/libvirt/images/h0";

// 192.168.122.100-254 on libvirt's default network
const IP_POOL: std::ops::RangeInclusive<u8> = 100..=254;

pub fn next_free_ip(conn: &mut PgConnection) -> anyhow::Result<String> {
    let used: Vec<String> = vm::vms.select(vm::ip_address).load(conn)?;
    IP_POOL
        .map(|n| format!("192.168.122.{n}"))
        .find(|ip| !used.contains(ip))
        .context("no free IP addresses left")
}

fn dir(name: &str) -> PathBuf {
    Path::new(BASE).join(name)
}

/// Creates the VM directory with its cloud-init seed.
pub fn create_files(vm: &Vm, user: &str, password: &str, apps: &CloudInit) -> anyhow::Result<()> {
    let d = dir(&vm.name);
    fs::create_dir_all(&d)?;

    // cloud init
    let tmp = tempdir(&vm.name)?;
    let user_data = tmp.join("user-data");
    let meta_data = tmp.join("meta-data");
    fs::write(&user_data, init_user_data(vm, user, password, apps)?)?;
    fs::write(
        &meta_data,
        format!("instance-id: {0}\nlocal-hostname: {0}\n", vm.name),
    )?;
    // explictly needed by kali
    let net_config = tmp.join("network-config");
    fs::write(
        &net_config,
        format!(
            "version: 2\nethernets:\n  net0:\n    match:\n      macaddress: '{}'\n    dhcp4: true\n",
            mac(&vm.ip_address)?
        ),
    )?;
    run(
        "cloud-localds",
        [
            "-N".as_ref(),
            net_config.as_os_str(),
            d.join("seed.iso").as_os_str(),
            user_data.as_os_str(),
            meta_data.as_os_str(),
        ],
    )?;
    fs::remove_dir_all(tmp)?;
    Ok(())
}

fn backup_dir(name: &str) -> PathBuf {
    Path::new(BASE).join("backups").join(name)
}

pub fn backup_path(vm: &str) -> anyhow::Result<String> {
    let d = backup_dir(vm);
    fs::create_dir_all(&d)?;
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    Ok(d.join(format!("{ts}.zfs.zst"))
        .to_string_lossy()
        .into_owned())
}

pub fn remove_backups(name: &str) -> anyhow::Result<()> {
    let d = backup_dir(name);
    if d.exists() {
        fs::remove_dir_all(d)?;
    }
    Ok(())
}

pub fn remove_files(name: &str) -> anyhow::Result<()> {
    let d = dir(name);
    if d.exists() {
        fs::remove_dir_all(d)?;
    }
    Ok(())
}

pub fn mac(ip: &str) -> anyhow::Result<String> {
    let o: Vec<u8> = ip.split('.').map(|s| s.parse()).collect::<Result<_, _>>()?;
    anyhow::ensure!(o.len() == 4, "invalid ip {ip}");
    //default mac address prefix for QEMU/KVM
    Ok(format!("52:54:00:{:02x}:{:02x}:{:02x}", o[1], o[2], o[3]))
}

pub fn domain_xml(vm: &Vm, disk: &Disk) -> anyhow::Result<String> {
    let d = dir(&vm.name);
    let disk_xml = match disk {
        Disk::Block(p) => format!(
            "<disk type='block' device='disk'>
      <driver name='qemu' type='raw' cache='none' io='native' discard='unmap'/>
      <source dev='{}'/>
      <target dev='vda' bus='virtio'/>
    </disk>",
            p.display()
        ),
        Disk::File { path, format } => format!(
            "<disk type='file' device='disk'>
      <driver name='qemu' type='{format}'/>
      <source file='{}'/>
      <target dev='vda' bus='virtio'/>
    </disk>",
            path.display()
        ),
    };
    Ok(format!(
        r#"<domain type='kvm'>
  <name>{name}</name>
  <memory unit='MiB'>{memory}</memory>
  <vcpu>{cpu}</vcpu>
  <os><type arch='x86_64' machine='q35'>hvm</type></os>
  <cpu mode='host-passthrough'/>
  <devices>
    {disk_xml}
    <disk type='file' device='cdrom'>
      <driver name='qemu' type='raw'/>
      <source file='{dir}/seed.iso'/>
      <target dev='sda' bus='sata'/>
      <readonly/>
    </disk>
    <interface type='network'>
      <mac address='{mac}'/>
      <source network='default'/>
      <model type='virtio'/>
    </interface>
    <serial type='pty'/>
    <console type='pty'><target type='serial'/></console>
    <channel type='unix'><target type='virtio' name='org.qemu.guest_agent.0'/></channel>
    <!-- kali's kernel resets in a loop without a display device -->
    <video><model type='vga'/></video>
  </devices>
</domain>
"#,
        name = vm.name,
        memory = vm.memory,
        cpu = vm.cpu,
        dir = d.display(),
        mac = mac(&vm.ip_address)?,
    ))
}

fn init_user_data(vm: &Vm, user: &str, password: &str, apps: &CloudInit) -> anyhow::Result<String> {
    let home = std::env::var("HOME")?;
    let key = fs::read_to_string(format!("{home}/.ssh/id_ed25519.pub"))
        .context("reading ~/.ssh/id_ed25519.pub")?;
    Ok(format!(
        "#cloud-config
hostname: {hostname}
ssh_pwauth: true
users:
  - name: {user}
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: false
    plain_text_passwd: {password}
    ssh_authorized_keys:
      - {key}
{apps}",
        hostname = vm.name,
        key = key.trim(),
        apps = apps.render(),
    ))
}

fn tempdir(name: &str) -> anyhow::Result<PathBuf> {
    let p = std::env::temp_dir().join(format!("h0-{name}"));
    fs::create_dir_all(&p)?;
    Ok(p)
}

fn run<I, S>(cmd: &str, args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("running {cmd}"))?;
    anyhow::ensure!(
        out.status.success(),
        "{cmd} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(())
}
