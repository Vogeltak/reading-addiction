#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["sqlite-utils"]
# ///

import uuid
import sqlite_utils

db = sqlite_utils.Database("addiction.db")
table = db["items"]

# Add the pub_id column
table.add_column("pub_id", str)

# Generate UUIDs for existing rows
for row in table.rows:
    table.update(row["url"], {"pub_id": str(uuid.uuid4())})

# Transform: change primary key from url to rowid, make pub_id unique
table.transform(
    pk="rowid",
    not_null={"pub_id"},
)
table.create_index(["pub_id"], unique=True, if_not_exists=True)
table.create_index(["url"], unique=True, if_not_exists=True)

print(f"Migrated {table.count} rows")
print("Sample:", list(table.rows_where(limit=3)))
