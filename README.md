# h0

CLI to create and manage VMs with ease. abstraction over libvirt,QEMU and KVM.

```
h0 CLI + Postgres        ← this repo: create / list / update / run / stop / delete
        │
     libvirtd                 ← daemon that owns the VM processes (virsh talks to it)
        │
   QEMU + KVM (/dev/kvm)      ← hardware virtualization
```

## Getting started

### 1. Host requirements

- Linux with KVM (`ls /dev/kvm` must exist; `lscpu | grep Virtualization` shows VT-x/AMD-V)
- Postgres
- Rust toolchain

### 2. Install virtualization packages 

###  Or check out the [installation guide](installation.md)

```bash
sudo apt install -y qemu-kvm libvirt-daemon-system libvirt-dev cloud-image-utils
sudo usermod -aG libvirt $USER   # then log out/in (or use: sg libvirt -c '<cmd>')
```

- `libvirt-daemon-system` — `libvirtd` + `virsh`
- `libvirt-dev` — C headers needed to build the `virt` crate
- `cloud-image-utils` — `cloud-localds`, builds the cloud-init seed ISO (sets user/password/ssh key on first boot)

Check it works:

```bash
virsh -c qemu:///system list --all       # empty table, no permission error
virsh -c qemu:///system net-list         # "default" network should be active
```

### 3. VM storage directory (once)

VM files must live under `/var/lib/libvirt/images` — Ubuntu's AppArmor profile
blocks libvirt from reading disks in `/home`.

```bash
sudo mkdir -p /var/lib/libvirt/images/h0
sudo chown $USER:$USER /var/lib/libvirt/images/h0
```

Download the Ubuntu cloud image every VM is cloned from:

```bash
mkdir -p /var/lib/libvirt/images/h0/images
curl -L -o /var/lib/libvirt/images/h0/images/noble.img \
  https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img
```

Layout:

```
/var/lib/libvirt/images/h0/
  images/noble.img        base Ubuntu 24.04 cloud image (read-only, shared)
  <vm-name>/
    disk.qcow2            VM's persistent disk, copy-on-write clone of noble.img
    user-data, meta-data  cloud-init config (user, password, ssh key, hostname)
    seed.iso              the two files above packed by cloud-localds
    domain.xml            libvirt definition (name, cpu, memory, disks, network)
```

### 4. Database

```bash
echo 'DATABASE_URL=postgres://user:pass@localhost/h0' > .env
cargo install diesel_cli --no-default-features --features postgres
diesel setup            # creates db + runs migrations/
```

### 5. Build and run

```bash
cargo run -- --help
cargo run -- create myvm --cpu 2 --memory 1024 --disk 20 --user test_user   # picks a free IP, builds files, defines domain
cargo run -- run myvm                                       # boot; ssh <user>@<ip> once up (~30s)
cargo run -- update myvm --cpu 4                            # applies on next boot; --disk can only grow
cargo run -- stop myvm                                      # graceful shutdown
cargo run -- delete myvm                                    # power off, undefine, remove files + IP
cargo run -- list
```

### IP addresses

`create` assigns the lowest free IP in `192.168.122.100–254` and reserves it
in libvirt's `default` network (DHCP host entry keyed by a MAC derived from the
IP), so the VM always boots with the same address. To keep libvirt's dynamic
leases out of that range, optionally shrink it once:

```bash
virsh -c qemu:///system net-edit default
#   <range start='192.168.122.2' end='192.168.122.99'/>
virsh -c qemu:///system net-destroy default && virsh -c qemu:///system net-start default
```

## Useful `virsh` Commands

```bash
virsh -c qemu:///system define domain.xml   # = create
virsh -c qemu:///system start test          # = run
virsh -c qemu:///system domifaddr test      # IP on the default NAT network
ssh $USER@<ip>                              # password "test" on the console
virsh -c qemu:///system shutdown test       # = stop (graceful)
virsh -c qemu:///system undefine test       # = delete (then rm -rf $D/test)
virsh -c qemu:///system console test        # attach to serial console exit with Ctrl+]
```
