use crate::models::Vm;
use crate::provision;
use anyhow::Context;
use std::ffi::CString;
use std::time::Duration;
use virt::connect::Connect;
use virt::domain::Domain;
use virt::domain_snapshot::DomainSnapshot;
use virt::network::Network;
use virt::sys::{self};

pub struct Snapshot {
    pub name: String,
    pub created_at: chrono::NaiveDateTime,
}

pub struct Libvirt {
    conn: Connect,
}

pub fn connect() -> anyhow::Result<Libvirt> {
    // errors are returned as Results; stop libvirt also printing them to stderr
    virt::error::clear_error_callback();
    let conn = Connect::open(Some("qemu:///system")).context("connecting to libvirt")?;
    Ok(Libvirt { conn })
}

impl Libvirt {
    pub fn define(&self, vm: &Vm) -> anyhow::Result<()> {
        let mut xml = provision::domain_xml(vm)?;
        if let Ok(existing) = Domain::lookup_by_name(&self.conn, &vm.name) {
            let uuid = existing.get_uuid_string()?;
            xml = xml.replacen("</name>", &format!("</name>\n  <uuid>{uuid}</uuid>"), 1);
        }
        Domain::define_xml(&self.conn, &xml)?;
        Ok(())
    }

    pub fn start(&self, name: &str) -> anyhow::Result<()> {
        let dom = self.domain(name)?;
        dom.create()?;
        dom.set_autostart(true)?;
        Ok(())
    }

    pub fn shutdown(&self, name: &str) -> anyhow::Result<()> {
        let dom = self.domain(name)?;
        dom.set_autostart(false)?;
        dom.shutdown()?;
        Ok(())
    }
    pub fn reboot(&self, name: &str) -> anyhow::Result<()> {
        let domain = self.domain(name)?;
        if domain.is_active()? {
            self.domain(name)?.reboot(sys::VIR_DOMAIN_REBOOT_DEFAULT)?;
        }
        Ok(())
    }
    pub fn resize_disk(&self, vm: &Vm) -> anyhow::Result<()> {
        let dom = self.domain(&vm.name)?;
        if !dom.is_active()? {
            return provision::resize_disk(vm);
        }
        let kib = u64::try_from(vm.disk)? * 1024 * 1024;
        dom.block_resize("vda", kib, 0)?;
        Ok(())
    }

    pub fn is_active(&self, name: &str) -> anyhow::Result<bool> {
        Ok(self.domain(name)?.is_active()?)
    }

    pub fn snapshot_create(&self, name: &str, snap: &str) -> anyhow::Result<()> {
        let xml = format!("<domainsnapshot><name>{snap}</name></domainsnapshot>");
        DomainSnapshot::create_xml(&self.domain(name)?, &xml, 0)?;
        Ok(())
    }

    pub fn snapshot_list(&self, name: &str) -> anyhow::Result<Vec<Snapshot>> {
        let mut all = Vec::new();
        for s in self.domain(name)?.list_all_snapshots(0)? {
            let xml = s.get_xml_desc(0)?;
            let secs: i64 = xml_text(&xml, "creationTime").unwrap_or("0").parse()?;
            all.push(Snapshot {
                name: s.get_name()?,
                created_at: chrono::DateTime::from_timestamp(secs, 0)
                    .context("bad creationTime")?
                    .naive_local(),
            });
        }
        all.sort_by_key(|s| s.created_at);
        Ok(all)
    }

    pub fn snapshot_revert(&self, name: &str, snap: &str) -> anyhow::Result<()> {
        self.snapshot(name, snap)?.revert(0)?;
        Ok(())
    }

    pub fn snapshot_delete(&self, name: &str, snap: &str) -> anyhow::Result<()> {
        self.snapshot(name, snap)?.delete(0)?;
        Ok(())
    }

    pub fn backup(&self, name: &str, target: &str) -> anyhow::Result<()> {
        let dom = self.domain(name)?;
        let xml = CString::new(format!(
            "<domainbackup mode='push'><disks>\
             <disk name='vda' type='file'><driver type='qcow2'/><target file='{target}'/></disk>\
             </disks></domainbackup>"
        ))?;
        // not wrapped by the virt crate yet
        let ret =
            unsafe { sys::virDomainBackupBegin(dom.as_ptr(), xml.as_ptr(), std::ptr::null(), 0) };
        anyhow::ensure!(
            ret == 0,
            "backup failed: {}",
            virt::error::Error::last_error()
        );
        while dom.get_job_info()?.r#type != sys::VIR_DOMAIN_JOB_NONE as i32 {
            std::thread::sleep(Duration::from_millis(500));
        }
        let done = dom.get_job_stats(sys::VIR_DOMAIN_JOB_STATS_COMPLETED)?;
        anyhow::ensure!(
            done.r#type == sys::VIR_DOMAIN_JOB_COMPLETED as i32,
            "backup job failed"
        );
        Ok(())
    }

    pub fn remove(&self, name: &str) -> anyhow::Result<()> {
        let dom = self.domain(name)?;
        if dom.is_active()? {
            dom.destroy()?;
        }
        dom.undefine()?;
        Ok(())
    }

    pub fn reserve_ip(&self, vm: &Vm) -> anyhow::Result<()> {
        self.dhcp_host(vm, sys::VIR_NETWORK_UPDATE_COMMAND_ADD_LAST)
    }

    pub fn release_ip(&self, vm: &Vm) -> anyhow::Result<()> {
        self.dhcp_host(vm, sys::VIR_NETWORK_UPDATE_COMMAND_DELETE)
    }

    fn dhcp_host(&self, vm: &Vm, cmd: sys::virNetworkUpdateCommand) -> anyhow::Result<()> {
        let xml = format!(
            "<host mac='{}' ip='{}'/>",
            provision::mac(&vm.ip_address)?,
            vm.ip_address
        );
        Network::lookup_by_name(&self.conn, "default")?.update(
            cmd,
            sys::VIR_NETWORK_SECTION_IP_DHCP_HOST,
            -1,
            &xml,
            sys::VIR_NETWORK_UPDATE_AFFECT_LIVE | sys::VIR_NETWORK_UPDATE_AFFECT_CONFIG,
        )?;
        Ok(())
    }

    fn snapshot(&self, name: &str, snap: &str) -> anyhow::Result<DomainSnapshot> {
        DomainSnapshot::lookup_by_name(&self.domain(name)?, snap, 0)
            .with_context(|| format!("snapshot '{snap}' not found on '{name}'"))
    }

    fn domain(&self, name: &str) -> anyhow::Result<Domain> {
        Domain::lookup_by_name(&self.conn, name)
            .with_context(|| format!("VM '{name}' not found in libvirt"))
    }
}

fn xml_text<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let start = xml.find(&format!("<{tag}>"))? + tag.len() + 2;
    let end = xml[start..].find(&format!("</{tag}>"))?;
    Some(&xml[start..start + end])
}
