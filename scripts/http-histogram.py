#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "seaborn",
#     "matplotlib",
#     "pandas",
# ]
# ///

import sys
import json
import seaborn as sns
import matplotlib.pyplot as plt
import pandas as pd

# Read JSON from stdin
data = json.load(sys.stdin)

# Replace "0" key with "dead"
if "0" in data:
    data["dead"] = data.pop("0")

# Create figure with two subplots
fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(16, 6))

# Plot 1: 200 OK vs Failed (all failures summed)
success_count = data.get("200", 0)
failed_count = sum(count for code, count in data.items() if code != "200")

plot1_data = pd.DataFrame({
    'Category': ['200 OK', 'Failed'],
    'Count': [success_count, failed_count]
})

sns.barplot(data=plot1_data, x='Category', y='Count', palette=['green', 'red'], ax=ax1)
ax1.set_title('Success vs Failures', fontsize=14, fontweight='bold')
ax1.set_xlabel('Category', fontsize=12)
ax1.set_ylabel('Count', fontsize=12)

# Add count labels on bars
for i, (category, count) in enumerate(zip(plot1_data['Category'], plot1_data['Count'])):
    ax1.text(i, count, str(count), ha='center', va='bottom', fontweight='bold')

# Plot 2: Individual failure cases (excluding 200)
failure_data = {code: count for code, count in data.items() if code != "200"}
plot2_df = pd.DataFrame(list(failure_data.items()), columns=['Status Code', 'Count'])

# Sort by status code (with "dead" at the end)
plot2_df['sort_key'] = plot2_df['Status Code'].apply(lambda x: 999 if x == 'dead' else int(x))
plot2_df = plot2_df.sort_values('sort_key').drop('sort_key', axis=1)

sns.barplot(data=plot2_df, x='Status Code', y='Count', palette='viridis', ax=ax2)
ax2.set_title('Individual Failure Cases', fontsize=14, fontweight='bold')
ax2.set_xlabel('Status Code', fontsize=12)
ax2.set_ylabel('Count', fontsize=12)
ax2.tick_params(axis='x', rotation=45)

plt.tight_layout()
plt.show()
