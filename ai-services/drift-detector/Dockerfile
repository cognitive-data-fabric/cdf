FROM python:3.10-slim
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
  gcc g++ libgomp1 && \
  rm -rf /var/lib/apt/lists/*

RUN pip install --no-cache-dir fastapi uvicorn numpy scipy scikit-learn structlog

COPY main.py .
EXPOSE 8002
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8002"]
