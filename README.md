# hostv1

CLI to create and manage KVM virtual machines. Postgres (via diesel) is the
source of truth for VM specs; libvirt/QEMU does the actual virtualization.

```
hostv1 CLI + Postgres        ← this repo: create / list / update / run / stop / delete
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

### 2. Install virtualization packages (once)

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
sudo mkdir -p /var/lib/libvirt/images/hostv1
sudo chown $USER:$USER /var/lib/libvirt/images/hostv1
```

Download the Ubuntu cloud image every VM is cloned from:

```bash
mkdir -p /var/lib/libvirt/images/hostv1/images
curl -L -o /var/lib/libvirt/images/hostv1/images/noble.img \
  https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img
```

Layout:

```
/var/lib/libvirt/images/hostv1/
  images/noble.img        base Ubuntu 24.04 cloud image (read-only, shared)
  <vm-name>/
    disk.qcow2            VM's persistent disk, copy-on-write clone of noble.img
    user-data, meta-data  cloud-init config (user, password, ssh key, hostname)
    seed.iso              the two files above packed by cloud-localds
    domain.xml            libvirt definition (name, cpu, memory, disks, network)
```

### 4. Database

```bash
echo 'DATABASE_URL=postgres://user:pass@localhost/hostv1' > .env
cargo install diesel_cli --no-default-features --features postgres
diesel setup            # creates db + runs migrations/
```

### 5. Build and run

```bash
cargo run -- --help
cargo run -- create myvm --cpu 2 --memory 1024 --disk 20   # picks a free IP, builds files, defines domain
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

## Creating a VM by hand (what the CLI automates)

Useful to understand the pieces. `D=/var/lib/libvirt/images/hostv1`, VM name `test`.

```bash
mkdir -p $D/test && cd $D/test

# 1. identity for first boot
cat > user-data <<EOF
#cloud-config
hostname: test
users:
  - name: $USER
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: false
    plain_text_passwd: test
    ssh_authorized_keys:
      - $(cat ~/.ssh/id_ed25519.pub)
EOF
printf 'instance-id: test\nlocal-hostname: test\n' > meta-data
cloud-localds seed.iso user-data meta-data

# 2. persistent disk, 10G, backed by the shared image
qemu-img create -f qcow2 -F qcow2 -b $D/images/noble.img disk.qcow2 10G

# 3. libvirt definition
cat > domain.xml <<EOF
<domain type='kvm'>
  <name>test</name>
  <memory unit='MiB'>512</memory>
  <vcpu>1</vcpu>
  <os><type arch='x86_64' machine='q35'>hvm</type></os>
  <cpu mode='host-passthrough'/>
  <devices>
    <disk type='file' device='disk'>
      <driver name='qemu' type='qcow2'/>
      <source file='$D/test/disk.qcow2'/>
      <target dev='vda' bus='virtio'/>
    </disk>
    <disk type='file' device='cdrom'>
      <driver name='qemu' type='raw'/>
      <source file='$D/test/seed.iso'/>
      <target dev='sda' bus='sata'/>
      <readonly/>
    </disk>
    <interface type='network'>
      <source network='default'/>
      <model type='virtio'/>
    </interface>
    <serial type='pty'/>
    <console type='pty'><target type='serial'/></console>
  </devices>
</domain>
EOF

virsh -c qemu:///system define domain.xml   # = create
virsh -c qemu:///system start test          # = run
virsh -c qemu:///system domifaddr test      # IP on the default NAT network
ssh $USER@<ip>                              # password "test" on the console
virsh -c qemu:///system shutdown test       # = stop (graceful)
virsh -c qemu:///system undefine test       # = delete (then rm -rf $D/test)
```

Handy: `virsh -c qemu:///system console test` attaches to the serial console
(exit with `Ctrl+]`).
