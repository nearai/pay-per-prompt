#!/usr/bin/env bash

set -e

echo "Running migrations"

DB_URL=$(yq '.db_url' configs/test_config.yaml)
sqlx migrate run --database-url $DB_URL
