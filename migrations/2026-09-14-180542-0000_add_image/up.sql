CREATE TYPE vm_image AS ENUM ('ubuntu24', 'ubuntu22', 'centos10', 'kali');
ALTER TABLE vms ADD COLUMN image vm_image NOT NULL DEFAULT 'ubuntu24';
