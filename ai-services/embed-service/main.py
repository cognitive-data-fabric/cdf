"""
Embedding Service for Cognitive Data Fabric
Generates vector embeddings for text, images, and audio using sentence-transformers.
"""

import asyncio
import time
from contextlib import asynccontextmanager
from typing import List, Optional

import numpy as np
import structlog
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
from sentence_transformers import SentenceTransformer

logger = structlog.get_logger()

# Default model - lightweight and fast
DEFAULT_MODEL = "all-MiniLM-L6-v2"


class EmbedRequest(BaseModel):
    texts: List[str] = Field(default_factory=list, description="Text inputs to embed")
    model_id: Optional[str] = Field(default=DEFAULT_MODEL, description="Model to use")
    normalize: bool = Field(default=True, description="L2 normalize embeddings")
    batch_size: int = Field(default=32, ge=1, le=256)


class EmbedResponse(BaseModel):
    embeddings: List[List[float]]
    model_id: str
    dimensions: int
    processing_time_ms: float


class HealthResponse(BaseModel):
    status: str
    model_loaded: str
    queue_depth: int


# Global model cache
_model_cache: dict[str, SentenceTransformer] = {}
_request_queue: asyncio.Queue = asyncio.Queue()


def get_model(model_id: str) -> SentenceTransformer:
    """Load and cache models on-demand."""
    if model_id not in _model_cache:
        logger.info("loading_model", model_id=model_id)
        start = time.time()
        _model_cache[model_id] = SentenceTransformer(model_id)
        logger.info(
            "model_loaded",
            model_id=model_id,
            elapsed_ms=(time.time() - start) * 1000,
        )
    return _model_cache[model_id]


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Load default model at startup."""
    logger.info("embed_service_startup")
    get_model(DEFAULT_MODEL)
    yield
    logger.info("embed_service_shutdown")


app = FastAPI(
    title="CDF Embedding Service",
    description="Generate vector embeddings for the Cognitive Data Fabric",
    version="0.1.0",
    lifespan=lifespan,
)


@app.get("/health", response_model=HealthResponse)
async def health():
    return HealthResponse(
        status="healthy",
        model_loaded=DEFAULT_MODEL,
        queue_depth=_request_queue.qsize(),
    )


@app.post("/embed", response_model=EmbedResponse)
async def embed_text(request: EmbedRequest):
    if not request.texts:
        raise HTTPException(status_code=400, detail="No texts provided")

    await _request_queue.put(time.time())

    try:
        model = get_model(request.model_id)
        start = time.time()

        # Generate embeddings
        embeddings = model.encode(
            request.texts,
            batch_size=request.batch_size,
            normalize_embeddings=request.normalize,
            convert_to_numpy=True,
            show_progress_bar=False,
        )

        processing_time = (time.time() - start) * 1000
        logger.info(
            "embeddings_generated",
            count=len(request.texts),
            model=request.model_id,
            dimensions=embeddings.shape[1],
            time_ms=processing_time,
        )

        return EmbedResponse(
            embeddings=embeddings.tolist(),
            model_id=request.model_id,
            dimensions=embeddings.shape[1],
            processing_time_ms=processing_time,
        )
    except Exception as e:
        logger.error("embed_failed", error=str(e))
        raise HTTPException(status_code=500, detail=str(e))
    finally:
        _request_queue.get_nowait()


@app.post("/embed/single")
async def embed_single(text: str, model_id: Optional[str] = DEFAULT_MODEL):
    """Convenience endpoint for single text embedding."""
    response = await embed_text(EmbedRequest(texts=[text], model_id=model_id))
    return {"embedding": response.embeddings[0], "model_id": response.model_id}


@app.get("/models")
async def list_models():
    """List available models."""
    return {
        "loaded": list(_model_cache.keys()),
        "available": [
            "all-MiniLM-L6-v2",
            "all-MiniLM-L12-v2",
            "all-mpnet-base-v2",
            "sentence-t5-base",
        ],
    }


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8001)
