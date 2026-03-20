-- Headscale mesh VPN fields on machines
ALTER TABLE machines ADD COLUMN headscale_ip TEXT;
ALTER TABLE machines ADD COLUMN headscale_node_id TEXT;
