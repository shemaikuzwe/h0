# Installation

## Check KVM
```bash
ls /dev/kvm; lscpu | grep -i virtualization
```
No `/dev/kvm` → enable VT-x/AMD-V in BIOS.

## Packages
```bash
sudo apt install -y qemu-kvm qemu-utils libvirt-daemon-system libvirt-dev cloud-image-utils
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
```bash
sudo mkdir -p /var/lib/libvirt/images/h0/images
sudo chown -R $USER:$USER /var/lib/libvirt/images/h0
```

## Images
Base images live in `/var/lib/libvirt/images/h0/images/<name>.qcow2`, where `<name>` is the `--image` value (`ubuntu24`, `ubuntu22`, `centos10`, `kali`). Ubuntu defaults to `ubuntu24`.

```bash
IMG=/var/lib/libvirt/images/h0/images

# Ubuntu 24 LTS
curl -L -o $IMG/ubuntu24.qcow2 \
  https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img

# Ubuntu 22 LTS
curl -L -o $IMG/ubuntu22.qcow2 \
  https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img

# CentOS 10
curl -L -o $IMG/centos10.qcow2 \
  https://cloud.centos.org/centos/10-stream/x86_64/images/CentOS-Stream-GenericCloud-x86_64-10-latest.x86_64.qcow2

# kali ships a raw disk inside a tarball; convert it to qcow2
KALI=$(curl -s https://kali.download/cloud-images/current/ | grep -o 'kali-linux-[0-9.]*-cloud-genericcloud-amd64.tar.xz' | head -1)
curl -L -o /tmp/kali.tar.xz https://kali.download/cloud-images/current/$KALI
tar -xf /tmp/kali.tar.xz -C /tmp
qemu-img convert -O qcow2 /tmp/disk.raw $IMG/kali.qcow2
rm /tmp/kali.tar.xz /tmp/disk.raw
```
