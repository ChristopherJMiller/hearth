#!/bin/bash
# dev/synapse/init-db.sh — Create Synapse database in shared PostgreSQL
#
# Mounted into the postgres container at /docker-entrypoint-initdb.d/10-synapse.sh
# Runs automatically on first container start.

set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    CREATE USER synapse WITH PASSWORD 'synapse';
    CREATE DATABASE synapse
        OWNER synapse
        ENCODING 'UTF8'
        LC_COLLATE='C'
        LC_CTYPE='C'
        TEMPLATE template0;
    GRANT ALL PRIVILEGES ON DATABASE synapse TO synapse;
EOSQL

echo "Synapse database created successfully"
