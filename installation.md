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

## Storage + image
```bash
sudo mkdir -p /var/lib/libvirt/images/h0/images
sudo chown -R $USER:$USER /var/lib/libvirt/images/h0
curl -L -o /var/lib/libvirt/images/h0/images/noble.img \
  https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img
```
