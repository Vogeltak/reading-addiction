# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "sqlite-utils",
# ]
# ///

import re
import sys
import sqlite_utils

# --- CONFIGURATION ---
# You can change these defaults or pass arguments via CLI
DB_NAME = "addiction.db"
TABLE_NAME = "items"
COLUMN_NAME = "markdown"

def unescape_punctuation(value):
    """
    Removes backslashes appearing before punctuation.
    Example: '04\.13\.2006' -> '04.13.2006'
    Example: '\(\!\)' -> '(!)'
    """
    if value is None or not isinstance(value, str):
        return value
    
    # Regex explanation:
    # \\        : Matches a literal backslash
    # ([^\w\s]) : Captures any character that is NOT a word char (a-z, 0-9) 
    #             and NOT whitespace. This targets punctuation like . , ! ? )
    return re.sub(r'\\([^\w\s])', r'\1', value)

def main():
    # Allow overriding DB name via command line: uv run clean_markdown.py my_data.db
    db_path = sys.argv[1] if len(sys.argv) > 1 else DB_NAME
    
    print(f"Opening database: {db_path}")
    db = sqlite_utils.Database(db_path)

    if TABLE_NAME not in db.table_names():
        print(f"Error: Table '{TABLE_NAME}' not found in database.")
        return

    print(f"Cleaning column '{COLUMN_NAME}' in table '{TABLE_NAME}'...")
    
    # This runs the transformation on every row in the database
    db[TABLE_NAME].convert(COLUMN_NAME, unescape_punctuation)
    
    print("Done! Punctuation has been unescaped.")

if __name__ == "__main__":
    main()
