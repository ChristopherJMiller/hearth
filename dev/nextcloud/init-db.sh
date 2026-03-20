#!/bin/bash
# dev/nextcloud/init-db.sh — Create Nextcloud database in shared PostgreSQL
#
# Mounted into the postgres container at /docker-entrypoint-initdb.d/20-nextcloud.sh
# Runs automatically on first container start.

set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    CREATE USER nextcloud WITH PASSWORD 'nextcloud';
    CREATE DATABASE nextcloud
        OWNER nextcloud
        ENCODING 'UTF8'
        LC_COLLATE='C'
        LC_CTYPE='C'
        TEMPLATE template0;
    GRANT ALL PRIVILEGES ON DATABASE nextcloud TO nextcloud;
EOSQL

echo "Nextcloud database created successfully"
