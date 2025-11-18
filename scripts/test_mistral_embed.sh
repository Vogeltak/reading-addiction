#!/usr/bin/env sh
curl -X POST "https://api.mistral.ai/v1/embeddings" \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${API_KEY}" \
     -d '{"model": "mistral-embed", "input": ["The people who create software generally refer to themselves as software *engineers*, and yet if they graduate from university, it is typically with a degree in computer *science*. That has always felt a little strange to me, because science and engineering are two pretty different disciplines – yet we for the most part seem to take such an obvious contradiction for granted. However, I think there is something uniquely magical about software, and part of that magic might stem from this tension in how we define it."]}' \
     -o embedding.json
