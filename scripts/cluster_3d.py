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
import time
import numpy as np
import pandas as pd
import plotly.express as px
from sklearn.cluster import KMeans
from sklearn.decomposition import PCA

# Set default renderer to browser
import plotly.io as pio
pio.renderers.default = "browser"

def main():
    # --- 1. Data Ingestion ---
    try:
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

    try:
        vectors = np.array([r["vector"] for r in records])
        urls = [r["url"] for r in records]
    except KeyError:
        print("Error: Input objects must contain 'url' and 'vector' keys.", file=sys.stderr)
        sys.exit(1)

    n_samples, n_features = vectors.shape
    print(f"Processing {n_samples} items with {n_features} dimensions...", file=sys.stderr)

    # --- 2. Clustering (K-Means) ---
    # Cap clusters at 8 or n_samples
    n_clusters = min(8, n_samples)
    
    if n_samples > 1:
        kmeans = KMeans(n_clusters=n_clusters, random_state=42, n_init="auto")
        labels = kmeans.fit_predict(vectors)
    else:
        labels = np.array([0])

    # --- 3. Dimensionality Reduction (3D PCA) ---
    print("Performing 3D dimensionality reduction...", file=sys.stderr)
    
    # We need 3 components for X, Y, Z
    target_dims = 3
    
    if n_features >= target_dims:
        pca = PCA(n_components=target_dims)
        coords = pca.fit_transform(vectors)
    else:
        # If data has < 3 dimensions, pad with zeros to make it 3D
        padding = np.zeros((n_samples, target_dims - n_features))
        coords = np.column_stack((vectors, padding))

    # --- 4. visualization (3D Plot) ---
    print("Generating 3D interactive plot...", file=sys.stderr)

    df = pd.DataFrame({
        'x': coords[:, 0],
        'y': coords[:, 1],
        'z': coords[:, 2],
        'Cluster': labels.astype(str),
        'URL': urls
    })
    
    # Sort for tidy legend
    df = df.sort_values('Cluster')

    fig = px.scatter_3d(
        df,
        x='x', 
        y='y', 
        z='z',
        color='Cluster',
        hover_name='URL',
        hover_data={'x': False, 'y': False, 'z': False, 'Cluster': True},
        title=f"3D Cluster Analysis ({n_samples} items, {n_clusters} clusters)",
        color_discrete_sequence=px.colors.qualitative.G10,
        template="plotly_white",
        height=900
    )

    fig.update_traces(marker=dict(size=5, line=dict(width=0)))
    
    # --- 5. Save Clusters to JSON ---
    timestamp = int(time.time())
    filename = f"clusters-{n_clusters}-{timestamp}.json"
    
    # Group URLs by Cluster ID for the output file
    # Format: { "0": ["url1", "url2"], "1": ["url3"] ... }
    cluster_map = {}
    for label, url in zip(labels, urls):
        label_str = str(label)
        if label_str not in cluster_map:
            cluster_map[label_str] = []
        cluster_map[label_str].append(url)

    # Sort keys for cleaner file
    sorted_cluster_map = dict(sorted(cluster_map.items()))

    with open(filename, 'w') as f:
        json.dump(sorted_cluster_map, f, indent=2)

    print(f"Cluster groups saved to: {filename}", file=sys.stderr)
    print("Opening browser...", file=sys.stderr)
    fig.show()

if __name__ == "__main__":
    main()
