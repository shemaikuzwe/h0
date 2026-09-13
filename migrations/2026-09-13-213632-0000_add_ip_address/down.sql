ALTER TABLE vms
    DROP CONSTRAINT vms_name_unique,
    DROP COLUMN disk,
    DROP COLUMN ip_address;
