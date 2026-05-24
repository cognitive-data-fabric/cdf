FROM python:3.10-slim
WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
  gcc g++ libgomp1 && \
  rm -rf /var/lib/apt/lists/*

# Install Python dependencies
COPY requirements.txt* ./
RUN pip install --no-cache-dir fastapi uvicorn sentence-transformers torch numpy scipy scikit-learn structlog

# Pre-download model
RUN python -c "from sentence_transformers import SentenceTransformer; SentenceTransformer('all-MiniLM-L6-v2')"

COPY main.py .
EXPOSE 8001
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8001"]
