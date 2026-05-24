"""
Drift Detection Service for Cognitive Data Fabric
Monitors embedding distributions over time to detect semantic drift.
"""

import time
from collections import deque
from typing import Dict, List, Optional

import numpy as np
import structlog
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
from scipy import stats
from sklearn.decomposition import PCA

logger = structlog.get_logger()


class DistributionSnapshot(BaseModel):
    concept_id: str = Field(description="Unique concept being tracked")
    embeddings: List[List[float]] = Field(description="Recent embeddings for this concept")
    timestamp: float = Field(default_factory=time.time)


class DriftReport(BaseModel):
    concept_id: str
    drift_detected: bool
    drift_score: float  # 0-1, higher = more drift
    reference_mean: List[float]
    current_mean: List[float]
    p_value: Optional[float]
    method: str


# In-memory store of reference distributions per concept
# Production: use CDF itself to store these
_reference_distributions: Dict[str, Dict] = {}
_current_windows: Dict[str, deque] = {}


def compute_wasserstein_distance(ref: np.ndarray, current: np.ndarray) -> float:
    """Compute sliced Wasserstein distance between distributions."""
    if ref.shape[1] != current.shape[1]:
        # Use PCA for dimensionality reduction if dimensions differ
        min_dim = min(ref.shape[1], current.shape[1])
        pca = PCA(n_components=min_dim)
        ref_reduced = pca.fit_transform(ref)
        cur_reduced = pca.transform(current)
    else:
        ref_reduced = ref
        cur_reduced = current

    # Random projection slices
    n_slices = 100
    distances = []
    dim = ref_reduced.shape[1]

    for _ in range(n_slices):
        direction = np.random.randn(dim)
        direction /= np.linalg.norm(direction)

        ref_proj = ref_reduced @ direction
        cur_proj = cur_reduced @ direction

        ref_sorted = np.sort(ref_proj)
        cur_sorted = np.sort(cur_proj)

        # Interpolate to same length
        n_points = min(len(ref_sorted), len(cur_sorted))
        ref_interp = np.interp(
            np.linspace(0, 1, n_points),
            np.linspace(0, 1, len(ref_sorted)),
            ref_sorted,
        )
        cur_interp = np.interp(
            np.linspace(0, 1, n_points),
            np.linspace(0, 1, len(cur_sorted)),
            cur_sorted,
        )

        distances.append(np.mean(np.abs(ref_interp - cur_interp)))

    return float(np.mean(distances))


def compute_kl_divergence(ref: np.ndarray, current: np.ndarray) -> float:
    """Approximate KL divergence using histogram binning."""
    n_bins = 50
    all_data = np.concatenate([ref, current])
    bins = np.linspace(all_data.min(), all_data.max(), n_bins)

    ref_hist, _ = np.histogram(ref.flatten(), bins=bins, density=True)
    cur_hist, _ = np.histogram(current.flatten(), bins=bins, density=True)

    # Add small epsilon to avoid log(0)
    epsilon = 1e-10
    ref_hist = ref_hist + epsilon
    cur_hist = cur_hist + epsilon

    ref_hist /= ref_hist.sum()
    cur_hist /= cur_hist.sum()

    kl = np.sum(ref_hist * np.log(ref_hist / cur_hist))
    return float(kl)


def detect_drift_mmd(ref: np.ndarray, current: np.ndarray) -> tuple[float, float]:
    """Maximum Mean Discrepancy for distribution comparison."""
    # Simplified MMD with linear kernel
    n_ref = len(ref)
    n_cur = len(current)

    # Mean embeddings
    ref_mean = ref.mean(axis=0)
    cur_mean = current.mean(axis=0)

    # MMD^2 = ||mu_x - mu_y||^2
    mmd_sq = float(np.sum((ref_mean - cur_mean) ** 2))

    # Approximate p-value using bootstrap
    combined = np.vstack([ref, current])
    n_bootstrap = 100
    count_extreme = 0

    for _ in range(n_bootstrap):
        np.random.shuffle(combined)
        boot_ref = combined[:n_ref]
        boot_cur = combined[n_ref:]
        boot_mmd = float(np.sum((boot_ref.mean(axis=0) - boot_cur.mean(axis=0)) ** 2))
        if boot_mmd >= mmd_sq:
            count_extreme += 1

    p_value = count_extreme / n_bootstrap
    return float(np.sqrt(mmd_sq)), p_value


app = FastAPI(
    title="CDF Drift Detector",
    description="Detect embedding distribution shift in the Cognitive Data Fabric",
    version="0.1.0",
)


@app.get("/health")
async def health():
    return {
        "status": "healthy",
        "tracked_concepts": len(_reference_distributions),
    }


@app.post("/register-reference")
async def register_reference(snapshot: DistributionSnapshot):
    """Register a baseline distribution for a concept."""
    embeddings = np.array(snapshot.embeddings)
    _reference_distributions[snapshot.concept_id] = {
        "embeddings": embeddings,
        "mean": embeddings.mean(axis=0).tolist(),
        "std": embeddings.std(axis=0).tolist(),
        "timestamp": snapshot.timestamp,
        "count": len(embeddings),
    }
    logger.info(
        "reference_registered",
        concept_id=snapshot.concept_id,
        count=len(embeddings),
    )
    return {"status": "registered", "concept_id": snapshot.concept_id}


@app.post("/check-drift")
async def check_drift(snapshot: DistributionSnapshot, threshold: float = 0.05):
    """
    Check if current embeddings have drifted from reference.
    Returns drift score and detection result.
    """
    if snapshot.concept_id not in _reference_distributions:
        raise HTTPException(
            status_code=404,
            detail=f"No reference distribution for concept '{snapshot.concept_id}'",
        )

    ref_data = _reference_distributions[snapshot.concept_id]
    ref_embeddings = ref_data["embeddings"]
    current_embeddings = np.array(snapshot.embeddings)

    # Compute drift metrics
    wasserstein = compute_wasserstein_distance(ref_embeddings, current_embeddings)
    mmd, p_value = detect_drift_mmd(ref_embeddings, current_embeddings)

    # Normalize scores to 0-1 range
    # Wasserstein is unbounded, use sigmoid-like scaling
    drift_score = min(1.0, wasserstein / (1.0 + wasserstein)) * 0.5 + min(
        1.0, mmd / (1.0 + mmd)
    ) * 0.5

    drift_detected = p_value < threshold or drift_score > 0.7

    report = DriftReport(
        concept_id=snapshot.concept_id,
        drift_detected=drift_detected,
        drift_score=drift_score,
        reference_mean=ref_data["mean"],
        current_mean=current_embeddings.mean(axis=0).tolist(),
        p_value=p_value,
        method="mmd+wasserstein",
    )

    logger.info(
        "drift_check_complete",
        concept_id=snapshot.concept_id,
        drift_detected=drift_detected,
        score=drift_score,
        p_value=p_value,
    )

    return report


@app.post("/update-reference")
async def update_reference(snapshot: DistributionSnapshot, weight: float = 0.3):
    """
    Update reference distribution with new data (exponential moving average).
    """
    if snapshot.concept_id not in _reference_distributions:
        return await register_reference(snapshot)

    old = _reference_distributions[snapshot.concept_id]
    new_embeddings = np.array(snapshot.embeddings)

    # Weighted combination
    old_mean = np.array(old["mean"])
    new_mean = new_embeddings.mean(axis=0)
    updated_mean = (1 - weight) * old_mean + weight * new_mean

    old["mean"] = updated_mean.tolist()
    old["timestamp"] = snapshot.timestamp
    old["count"] += len(new_embeddings)

    logger.info(
        "reference_updated",
        concept_id=snapshot.concept_id,
        weight=weight,
        new_count=old["count"],
    )

    return {"status": "updated", "concept_id": snapshot.concept_id}


@app.get("/concepts")
async def list_concepts():
    return {
        concept_id: {
            "count": data["count"],
            "timestamp": data["timestamp"],
        }
        for concept_id, data in _reference_distributions.items()
    }


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8002)
