#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "sqlite-utils",
#     "seaborn",
#     "matplotlib",
# ]
# ///

"""
Tag Histogram Generator

This script reads tags from a SQLite database and produces a histogram
showing the frequency of each tag.
"""

import sys
from collections import Counter
from sqlite_utils import Database
import seaborn as sns
import matplotlib.pyplot as plt


def get_tags_from_database(db_path: str) -> list[str]:
    """
    Extract all tags from the database.
    
    Args:
        db_path: Path to the SQLite database
        
    Returns:
        List of individual tags
    """
    db = Database(db_path)
    
    # Get all rows from the item table
    items = db["items"].rows
    
    # Collect all tags
    all_tags = []
    for item in items:
        tags_string = item.get("tags", "")
        if tags_string:
            # Split by comma and strip whitespace
            tags = [tag.strip() for tag in tags_string.split(",") if tag.strip()]
            all_tags.extend(tags)
    
    return all_tags


def create_histogram(tags: list[str], output_file: str = "tag_histogram.png"):
    """
    Create a visual histogram of tag frequencies using seaborn.
    
    Args:
        tags: List of tags to analyze
        output_file: Path to save the histogram image
    """
    if not tags:
        print("No tags found in the database.")
        return
    
    # Count tag frequencies
    tag_counts = Counter(tags)
    
    # Sort by frequency (descending)
    sorted_tags = sorted(tag_counts.items(), key=lambda x: (-x[1], x[0]))
    
    # Limit to top 20 tags for readability
    top_n = 20
    if len(sorted_tags) > top_n:
        sorted_tags = sorted_tags[:top_n]
        title_suffix = f" (Top {top_n})"
    else:
        title_suffix = ""
    
    # Prepare data for plotting
    tag_names = [tag for tag, _ in sorted_tags]
    counts = [count for _, count in sorted_tags]
    
    # Set up the plot style
    sns.set_theme(style="whitegrid")
    
    # Create figure and axis
    fig, ax = plt.subplots(figsize=(12, max(6, len(tag_names) * 0.4)))
    
    # Create horizontal bar plot
    sns.barplot(x=counts, y=tag_names, palette="viridis", ax=ax)
    
    # Customize the plot
    ax.set_xlabel("Count", fontsize=12, fontweight='bold')
    ax.set_ylabel("Tag", fontsize=12, fontweight='bold')
    ax.set_title(f"Tag Frequency Histogram{title_suffix}\n"
                 f"Total tags: {len(tags)}, Unique tags: {len(tag_counts)}", 
                 fontsize=14, fontweight='bold', pad=20)
    
    # Add count labels on the bars
    for i, (count, tag) in enumerate(zip(counts, tag_names)):
        ax.text(count, i, f' {count}', va='center', fontsize=10)
    
    # Adjust layout to prevent label cutoff
    plt.tight_layout()
    
    # Save the figure
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"\nHistogram saved to: {output_file}")
    
    # Display summary statistics
    print(f"\nSummary Statistics:")
    print(f"  Total tags: {len(tags)}")
    print(f"  Unique tags: {len(tag_counts)}")
    print(f"  Most common tag: '{sorted_tags[0][0]}' ({sorted_tags[0][1]} occurrences)")
    print(f"  Least common tag: '{sorted_tags[-1][0]}' ({sorted_tags[-1][1]} occurrences)")
    
    # Optionally display the plot
    # plt.show()  # Uncomment to display interactively


def main():
    """Main function to run the script."""
    if len(sys.argv) < 2:
        print("Usage: uv run tag_histogram.py <database_path> [output_file]")
        print("\nExample: uv run tag_histogram.py mydata.db")
        print("         uv run tag_histogram.py mydata.db my_histogram.png")
        sys.exit(1)
    
    db_path = sys.argv[1]
    output_file = sys.argv[2] if len(sys.argv) > 2 else "tag_histogram.png"
    
    try:
        print(f"Reading tags from: {db_path}")
        tags = get_tags_from_database(db_path)
        create_histogram(tags, output_file)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
