# /// script
# dependencies = [
#   "matplotlib",
# ]
# ///

import json
import sys
import matplotlib.pyplot as plt

def main():
    # Load the cluster data from stdin
    try:
        # Check if input is being piped
        if sys.stdin.isatty():
            print("Please pipe JSON data to this script.\nExample: cat clusters.json | uv run cluster_labels_hist.py")
            return
        data = json.load(sys.stdin)
    except json.JSONDecodeError:
        print("Error: Failed to decode JSON from stdin.")
        return

    # Define the labels we generated previously
    cluster_labels = {
        "0": "Philosophy, Politics, Sociology & History",
        "1": "Personal Development, Career Advice & Productivity",
        "2": "Sci-Fi, Fantasy Lit & Creative Writing",
        "3": "Distributed Systems, Networking & Security",
        "4": "AI, ML & Tech Industry Analysis",
        "5": "Systems Programming, OS, Rust & Low-Level",
        "6": "Dutch Journalism, Politics & Society",
        "7": "Cooking, Recipes & Global Cuisine"
    }

    # Prepare data for plotting
    # Sort by cluster ID (converted to int) to keep order 0-7
    sorted_keys = sorted(data.keys(), key=lambda x: int(x))

    counts = [len(data[k]) for k in sorted_keys]
    print(counts)
    print(f"total is {sum(counts)}")
    # Create labels that include the Cluster ID
    labels = [f"Cluster {k}: {cluster_labels.get(k, 'Unknown')}" for k in sorted_keys]

    # Create the horizontal bar chart
    plt.figure(figsize=(12, 8))
    bars = plt.barh(labels, counts, color='skyblue', edgecolor='navy')

    # Add count labels to the end of each bar
    for bar in bars:
        width = bar.get_width()
        plt.text(width + 1, bar.get_y() + bar.get_height()/2, 
                 f'{int(width)}', 
                 ha='left', va='center', fontweight='bold')

    # Styling
    plt.xlabel('Number of URLs')
    plt.title('Distribution of URLs per Content Cluster')
    plt.grid(axis='x', linestyle='--', alpha=0.7)
    
    # Invert y-axis so Cluster 0 is at the top
    plt.gca().invert_yaxis()
    
    plt.tight_layout()
    plt.show()

if __name__ == "__main__":
    main()
