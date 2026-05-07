#!/usr/bin/env python3
"""
load_to_chromadb.py — Загрузка доктрин WATERS в ChromaDB

Использует sentence-transformers с моделью all-MiniLM-L6-v2
для генерации эмбеддингов и загрузки в ChromaDB.

Usage:
    python scripts/load_to_chromadb.py [--host HOST] [--port PORT] [--collection NAME]
"""

import argparse
import hashlib
import os
import re
import sys
from pathlib import Path

import chromadb
from chromadb.config import Settings
from sentence_transformers import SentenceTransformer

EMBEDDING_MODEL = "all-MiniLM-L6-v2"
CHUNK_SIZE = 500
CHUNK_OVERLAP = 100
DOCTRINE_DIR = Path(__file__).parent.parent / "doctrine"


def chunk_text(text: str, chunk_size: int = CHUNK_SIZE, overlap: int = CHUNK_OVERLAP) -> list[str]:
    words = text.split()
    chunks = []
    for i in range(0, len(words), chunk_size - overlap):
        chunk = " ".join(words[i:i + chunk_size])
        if chunk.strip():
            chunks.append(chunk.strip())
    return chunks


def extract_metadata(filepath: str, chunk_index: int, chunk: str) -> dict:
    filename = os.path.basename(filepath)
    title_match = re.search(r"^#\s+(.+)$", chunk, re.MULTILINE)
    title = title_match.group(1) if title_match else filename.replace(".md", "")

    return {
        "source": "doctrine",
        "filename": filename,
        "filepath": filepath,
        "chunk_index": chunk_index,
        "title": title,
        "language": "ru",
    }


def doc_id(filepath: str, chunk_index: int) -> str:
    raw = f"{filepath}:{chunk_index}"
    return hashlib.sha256(raw.encode()).hexdigest()[:16]


def load_documents(directory: Path) -> list[tuple[str, list[str], list[dict]]]:
    documents = []
    for md_file in sorted(directory.glob("*.md")):
        text = md_file.read_text(encoding="utf-8")
        chunks = chunk_text(text)
        metadata_list = [extract_metadata(str(md_file), i, c) for i, c in enumerate(chunks)]
        ids = [doc_id(str(md_file), i) for i in range(len(chunks))]
        documents.append((ids, chunks, metadata_list))
    return documents


def main():
    parser = argparse.ArgumentParser(description="Загрузка доктрин WATERS в ChromaDB")
    parser.add_argument("--host", default="localhost", help="ChromaDB host")
    parser.add_argument("--port", type=int, default=8000, help="ChromaDB port")
    parser.add_argument("--collection", default="doctrine", help="ChromaDB collection name")
    args = parser.parse_args()

    print(f"=== Загрузка доктрин WATERS в ChromaDB ===")
    print(f"Model: {EMBEDDING_MODEL}")
    print(f"ChromaDB: {args.host}:{args.port}")
    print(f"Collection: {args.collection}")
    print(f"Doctrine dir: {DOCTRINE_DIR}")
    print()

    if not DOCTRINE_DIR.exists():
        print(f"ERROR: Doctrine directory not found: {DOCTRINE_DIR}")
        sys.exit(1)

    print("Загрузка эмбеддинг-модели...")
    model = SentenceTransformer(EMBEDDING_MODEL)

    print("Подключение к ChromaDB...")
    client = chromadb.HttpClient(host=args.host, port=args.port, settings=Settings(anonymized_telemetry=False))

    collection = client.get_or_create_collection(
        name=args.collection,
        metadata={"hnsw:space": "cosine"},
    )
    print(f"Collection '{args.collection}' готова. Документов сейчас: {collection.count()}")
    print()

    all_docs = load_documents(DOCTRINE_DIR)

    total_chunks = sum(len(chunks) for _, chunks, _ in all_docs)
    print(f"Найдено {len(all_docs)} файлов, {total_chunks} чанков для загрузки.")
    print()

    for ids, chunks, metadatas in all_docs:
        if not chunks:
            continue
        filename = metadatas[0]["filename"]
        print(f"  [{filename}] {len(chunks)} чанков...", end=" ", flush=True)

        embeddings = model.encode(chunks, show_progress_bar=False).tolist()

        collection.upsert(
            ids=ids,
            embeddings=embeddings,
            documents=chunks,
            metadatas=metadatas,
        )
        print("OK")

    print()
    print(f"=== Загрузка завершена. Всего документов: {collection.count()} ===")


if __name__ == "__main__":
    main()
