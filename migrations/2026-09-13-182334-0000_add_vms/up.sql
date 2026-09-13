-- Your SQL goes here
CREATE TYPE vm_status AS ENUM ('running', 'stopped', 'suspended');
CREATE TABLE vms (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    cpu INT NOT NULL,
    memory INT NOT NULL,
    status vm_status NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);