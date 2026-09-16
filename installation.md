# Installation

## Check KVM
```bash
ls /dev/kvm; lscpu | grep -i virtualization
```
No `/dev/kvm` → enable VT-x/AMD-V in BIOS.

## Packages
```bash
sudo apt install -y qemu-kvm qemu-utils libvirt-daemon-system libvirt-dev cloud-image-utils zfsutils-linux zstd jq
```

## Daemon
```bash
sudo systemctl enable --now libvirtd
sudo virsh net-autostart default; sudo virsh net-start default
```

## Permissions
```bash
sudo usermod -aG libvirt,kvm $USER
# log out/in, then:
virsh -c qemu:///system list --all
```

## Storage
VM disks are ZFS volumes on the pool `tank` (`src/storage/zfs.rs`); `h0` runs `zfs` through
`sudo -n`. Cloud-init seeds and backups stay under `/var/lib/libvirt/images/h0`.

```bash
sudo mkdir -p /var/lib/libvirt/images/h0
sudo chown -R $USER:$USER /var/lib/libvirt/images/h0

# a real disk is better; a file-backed pool works for dev
sudo truncate -s 100G /var/lib/libvirt/images/h0/tank.img
sudo zpool create tank /var/lib/libvirt/images/h0/tank.img
sudo zfs create tank/images
sudo zfs create tank/vms

echo "$USER ALL=(root) NOPASSWD: /usr/sbin/zfs" | sudo tee /etc/sudoers.d/h0-zfs
```

## Images
Base images are volumes `tank/images/<name>` with a `@base` snapshot that every VM is cloned
from, where `<name>` is the `--image` value (`ubuntu24`, `ubuntu22`, `centos10`, `kali`).
Ubuntu defaults to `ubuntu24`.

```bash
# writes a qcow2 to tank/images/<name> and snapshots it
import() {
  local size=$(qemu-img info --output=json "$2" | jq '."virtual-size"')
  sudo zfs create -V "$size" "tank/images/$1"
  sudo udevadm settle
  sudo qemu-img convert -O raw "$2" "/dev/zvol/tank/images/$1"
  sudo zfs snapshot "tank/images/$1@base"
}

# Ubuntu 24 LTS
curl -L -o /tmp/ubuntu24.img \
  https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img
import ubuntu24 /tmp/ubuntu24.img

# Ubuntu 22 LTS
curl -L -o /tmp/ubuntu22.img \
  https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img
import ubuntu22 /tmp/ubuntu22.img

# CentOS 10
curl -L -o /tmp/centos10.img \
  https://cloud.centos.org/centos/10-stream/x86_64/images/CentOS-Stream-GenericCloud-x86_64-10-latest.x86_64.qcow2
import centos10 /tmp/centos10.img

# kali ships a raw disk inside a tarball; qemu-img handles raw input too
KALI=$(curl -s https://kali.download/cloud-images/current/ | grep -o 'kali-linux-[0-9.]*-cloud-genericcloud-amd64.tar.xz' | head -1)
curl -L -o /tmp/kali.tar.xz https://kali.download/cloud-images/current/$KALI
tar -xf /tmp/kali.tar.xz -C /tmp
import kali /tmp/disk.raw

rm /tmp/*.img /tmp/kali.tar.xz /tmp/disk.raw
```
