#!/usr/bin/env python3
"""CDF Demo Data Seeder — Populates database with sample data for live demo."""

import requests
import time

BASE_URL = "http://gateway:8080"
EMBED_URL = "http://embed-service:8001"


def wait_for_service(url, timeout=120):
    """Wait for service to become healthy."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            r = requests.get(f"{url}/health", timeout=2)
            if r.status_code == 200:
                print(f"✅ {url} is ready")
                return True
        except Exception:
            pass
        time.sleep(2)
    print(f"❌ {url} not ready after {timeout}s")
    return False


def get_embedding(text):
    """Generate embedding via embed service."""
    r = requests.post(
        f"{EMBED_URL}/embed",
        json={"texts": [text], "normalize": True},
        timeout=30,
    )
    return r.json()["embeddings"][0]


def seed():
    print("=" * 50)
    print("CDF Demo Data Seeder")
    print("=" * 50)

    # Wait for services
    if not wait_for_service(BASE_URL):
        return
    if not wait_for_service(EMBED_URL):
        return

    # Create a demo table
    print("\n📦 Creating 'papers' table...")
    requests.post(
        f"{BASE_URL}/v1/tables",
        json={
            "name": "papers",
            "namespace": "demo",
            "schema": {
                "title": {"type": "string", "nullable": False},
                "abstract": {"type": "string"},
                "year": {"type": "integer"},
                "embedding": {
                    "type": "vector",
                    "dimensions": 384,
                    "metric": "cosine",
                },
            },
        },
    )

    # Seed papers with real embeddings
    papers = [
        {
            "title": "Attention Is All You Need",
            "abstract": "We propose a new simple network architecture, the Transformer, based solely on attention mechanisms.",
            "year": 2017,
            "text": "transformer attention mechanism deep learning",
        },
        {
            "title": "BERT: Pre-training of Deep Bidirectional Transformers",
            "abstract": "We introduce a new language representation model called BERT.",
            "year": 2018,
            "text": "bert language model pre-training nlp",
        },
        {
            "title": "GPT-3: Language Models are Few-Shot Learners",
            "abstract": "We demonstrate that scaling up language models greatly improves task-agnostic few-shot performance.",
            "year": 2020,
            "text": "gpt-3 large language model few-shot learning",
        },
        {
            "title": "ImageNet Classification with Deep Convolutional Neural Networks",
            "abstract": "We trained a large, deep convolutional neural network to classify the 1.2 million images.",
            "year": 2012,
            "text": "alexnet convolutional neural network image classification",
        },
        {
            "title": "Deep Residual Learning for Image Recognition",
            "abstract": "We present a residual learning framework to ease the training of networks that are substantially deeper.",
            "year": 2015,
            "text": "resnet residual network deep learning computer vision",
        },
        {
            "title": "Generative Adversarial Networks",
            "abstract": "We propose a new framework for estimating generative models via an adversarial process.",
            "year": 2014,
            "text": "gan generative adversarial network deep learning",
        },
        {
            "title": "Graph Attention Networks",
            "abstract": "We present graph attention networks, which operate on graph-structured data.",
            "year": 2017,
            "text": "graph neural network attention gat",
        },
        {
            "title": "Neural Machine Translation by Jointly Learning to Align and Translate",
            "abstract": "Neural machine translation by jointly learning to align and translate.",
            "year": 2014,
            "text": "seq2seq attention machine translation neural",
        },
    ]

    print(f"\n📝 Inserting {len(papers)} papers with embeddings...")

    for paper in papers:
        text = paper.pop("text")
        embedding = get_embedding(text)
        paper["embedding"] = {
            "model_id": "all-MiniLM-L6-v2",
            "values": embedding,
        }

        r = requests.post(
            f"{BASE_URL}/v1/insert",
            json={"table": "papers", "namespace": "demo", "data": paper},
        )
        if r.status_code in (200, 201):
            print(f"  ✅ {paper['title'][:40]}...")
        else:
            print(f"  ❌ Failed: {r.text[:100]}")

    # Add graph edges (citation relationships)
    print("\n🔗 Adding citation edges...")
    edges = [
        ("BERT: Pre-training", "Attention Is All You Need"),
        ("GPT-3: Language Models", "Attention Is All You Need"),
        ("Deep Residual Learning", "ImageNet Classification"),
        ("Graph Attention Networks", "Attention Is All You Need"),
        ("Neural Machine Translation", "Attention Is All You Need"),
    ]

    for from_title, to_title in edges:
        r = requests.post(
            f"{BASE_URL}/v1/graph/edges",
            json={
                "namespace": "demo",
                "from_id": from_title,
                "to_id": to_title,
                "edge_type": "cites",
                "properties": {"certainty": 1.0},
            },
        )
        if r.status_code in (200, 201):
            print(f"  ✅ {from_title[:25]}... → cites → {to_title[:25]}...")
        else:
            print(f"  ❌ Edge failed")

    print("\n" + "=" * 50)
    print("✅ Demo data seeded successfully!")
    print(f"   Table: papers ({len(papers)} rows)")
    print(f"   Graph edges: {len(edges)} citations")
    print(f"\n   Try these queries:")
    print(f"   - GET {BASE_URL}/health")
    print(f"   - POST {BASE_URL}/v1/search (vector search)")
    print(f"   - POST {BASE_URL}/v1/graph/traverse (citations)")
    print("=" * 50)


if __name__ == "__main__":
    seed()
