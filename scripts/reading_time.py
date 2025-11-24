# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "sqlite-utils",
# ]
# ///

import sqlite_utils
import argparse
import sys
import math

def calculate_reading_time(word_count, wpm=200):
    """
    Calculates reading time assuming 200 words per minute.
    Returns a human-readable string.
    """
    minutes_total = word_count / wpm
    
    if minutes_total < 1:
        return "Less than a minute"
    
    hours = math.floor(minutes_total / 60)
    minutes = math.ceil(minutes_total % 60) # Round up for the last minute
    
    parts = []
    if hours > 0:
        parts.append(f"{hours} hour{'s' if hours != 1 else ''}")
    if minutes > 0:
        parts.append(f"{minutes} minute{'s' if minutes != 1 else ''}")
        
    return " ".join(parts)

def main():
    parser = argparse.ArgumentParser(description="Calculate total reading time from markdown column in sqlite.")
    parser.add_argument("db_path", help="Path to the SQLite database file")
    
    args = parser.parse_args()
    
    # Check if file exists to give a better error message
    try:
        db = sqlite_utils.Database(args.db_path)
    except Exception as e:
        print(f"Error opening database: {e}")
        sys.exit(1)

    table_name = "items"
    column_name = "markdown"

    if table_name not in db.table_names():
        print(f"Error: Table '{table_name}' not found in database.")
        sys.exit(1)

    total_words = 0
    try:
        # iterate over rows where status is 'unread'
        for row in db[table_name].rows_where("status = ?", ["unread"]):
            content = row.get(column_name)
            if content and isinstance(content, str):
                # Split by whitespace to count words
                words = content.split()
                total_words += len(words)
    except Exception as e:
        print(f"Error processing rows: {e}")
        sys.exit(1)

    print(f"Total Words: {total_words:,}")
    print(f"Estimated Reading Time: {calculate_reading_time(total_words)}")

if __name__ == "__main__":
    main()
