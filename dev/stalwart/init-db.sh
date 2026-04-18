#!/bin/bash
# dev/stalwart/init-db.sh — Create Stalwart database in shared PostgreSQL
#
# Mounted into the postgres container at /docker-entrypoint-initdb.d/30-stalwart.sh
# Runs automatically on first container start.

set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    CREATE USER stalwart WITH PASSWORD 'stalwart';
    CREATE DATABASE stalwart
        OWNER stalwart
        ENCODING 'UTF8'
        LC_COLLATE='C'
        LC_CTYPE='C'
        TEMPLATE template0;
    GRANT ALL PRIVILEGES ON DATABASE stalwart TO stalwart;
EOSQL

echo "Stalwart database created successfully"
