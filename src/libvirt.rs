use crate::models::Vm;
use crate::provision;
use anyhow::Context;
use virt::connect::Connect;
use virt::domain::Domain;
use virt::network::Network;
use virt::sys::{self};

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
        self.domain(name)?.create()?;
        Ok(())
    }

    pub fn shutdown(&self, name: &str) -> anyhow::Result<()> {
        self.domain(name)?.shutdown()?;
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

    /// Powers off if running, then unregisters the domain.
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

    fn domain(&self, name: &str) -> anyhow::Result<Domain> {
        Domain::lookup_by_name(&self.conn, name)
            .with_context(|| format!("VM '{name}' not found in libvirt"))
    }
}
