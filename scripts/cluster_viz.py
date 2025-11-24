# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "numpy",
#     "pandas",
#     "plotly",
#     "scikit-learn",
# ]
# ///

import sys
import json
import numpy as np
import pandas as pd
import plotly.express as px
from sklearn.cluster import KMeans
from sklearn.decomposition import PCA

# Set default renderer so it works in various environments (terminal, jupyter, etc)
import plotly.io as pio
pio.renderers.default = "browser"

def main():
    # --- 1. Data Ingestion ---
    try:
        # Read everything from stdin
        input_data = sys.stdin.read()
        if not input_data:
            print("Error: No input received from stdin.", file=sys.stderr)
            sys.exit(1)
        
        records = json.loads(input_data)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON format - {e}", file=sys.stderr)
        sys.exit(1)

    if not records:
        print("Error: JSON array is empty.", file=sys.stderr)
        sys.exit(1)

    # Extract vectors and URLs
    try:
        vectors = np.array([r["vector"] for r in records])
        urls = [r["url"] for r in records]
    except KeyError:
        print("Error: Input objects must contain 'url' and 'vector' keys.", file=sys.stderr)
        sys.exit(1)

    n_samples, n_features = vectors.shape
    print(f"Processing {n_samples} items with {n_features} dimensions...", file=sys.stderr)

    # --- 2. Clustering (K-Means) ---
    # Dynamically choose cluster count based on data size (cap at 8 for demo)
    n_clusters = min(8, n_samples)
    if n_samples > 1:
        # Using k-means++ initialization for better results
        kmeans = KMeans(n_clusters=n_clusters, random_state=42, n_init="auto")
        labels = kmeans.fit_predict(vectors)
    else:
        # Handle edge case of a single data point
        labels = np.array([0])

    # --- 3. Dimensionality Reduction (PCA) ---
    # Project high-dimensional vectors down to 2D for plotting
    print("Performing dimensionality reduction (PCA)...", file=sys.stderr)
    if n_features > 2:
        pca = PCA(n_components=2)
        coords = pca.fit_transform(vectors)
    elif n_features == 2:
        # Already 2D
        coords = vectors
    else:
        # Fallback for 1D data: add a dummy 0 y-axis
        coords = np.column_stack((vectors, np.zeros_like(vectors)))

    # --- 4. Data Preparation for Plotly ---
    # Combine results into a Pandas DataFrame. This is the easiest way
    # to map data to visual elements in Plotly Express.
    df = pd.DataFrame({
        'x_coord': coords[:, 0],
        'y_coord': coords[:, 1],
        # Convert labels to string so Plotly treats them as distinct categories
        # instead of a continuous numeric scale.
        'Cluster ID': labels.astype(str),
        'URL': urls
    })
    
    # Sort by cluster ID so the legend looks tidy
    df = df.sort_values('Cluster ID')

    # --- 5. Interactive Visualization ---
    print("Generating interactive plot...", file=sys.stderr)

    fig = px.scatter(
        df,
        x='x_coord',
        y='y_coord',
        color='Cluster ID',
        # Use the URL as the bold header in the hover tooltip
        hover_name='URL',
        # Customize what details show up below the header
        # We hide the raw PCA coordinates as they aren't usually interpretable
        hover_data={
            'x_coord': False,
            'y_coord': False,
            'Cluster ID': True
        },
        title=f"Interactive Cluster Analysis ({n_samples} items, {n_clusters} clusters)",
        # Use a distinct color palette suited for categorical data
        color_discrete_sequence=px.colors.qualitative.G10,
        template="plotly_white",
        height=800
    )

    # Make the markers slightly larger and give them a border for visibility
    fig.update_traces(
        marker=dict(size=10, line=dict(width=1, color='DarkSlateGrey')),
        selector=dict(mode='markers')
    )
    
    # Clean up axes labels as PCA components don't have inherent units
    fig.update_xaxes(title_text="PCA Component 1")
    fig.update_yaxes(title_text="PCA Component 2")

    print("Opening browser...", file=sys.stderr)
    # This will open your default web browser with the plot
    fig.show()

if __name__ == "__main__":
    main()
