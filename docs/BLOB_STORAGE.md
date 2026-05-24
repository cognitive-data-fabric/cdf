# CDF Blob Storage Guide — Images, Audio, Video, PDFs

CDF stores large binary files (images, audio, video, PDFs) using **BlobRef** — a content-addressable reference to MinIO (S3-compatible object storage).

## Architecture

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│  Your App   │─────▶│   Gateway   │─────▶│    MinIO    │
│             │      │   :8080     │      │  (internal) │
│  Upload     │      │  Proxies    │      │  Blob Store │
│  Download   │      │  blobs      │      │             │
└─────────────┘      └─────────────┘      └─────────────┘
```

**Key point:** Only port `8080` (Gateway) is exposed. MinIO is internal. All blob operations go through the Gateway API.

---

## BlobRef Data Type

| Field       | Type     | Description                                              |
| ----------- | -------- | -------------------------------------------------------- |
| `hash`      | `string` | Content hash (SHA-256) — the unique identifier           |
| `size`      | `int`    | File size in bytes                                       |
| `mime_type` | `string` | `image/png`, `audio/mp3`, `video/mp4`, `application/pdf` |
| `metadata`  | `object` | Optional: width, height, duration, etc.                  |

---

## REST API Endpoints

### Upload Blob

| Method | Endpoint           | Auth          | Content-Type          |
| ------ | ------------------ | ------------- | --------------------- |
| `POST` | `/v1/blobs/upload` | API Key / JWT | `multipart/form-data` |

**Request:**

```http
POST /v1/blobs/upload
Content-Type: multipart/form-data
X-API-Key: dev-key-abc123

--boundary
Content-Disposition: form-data; name="file"; filename="photo.png"
Content-Type: image/png

<binary data>
--boundary--
```

**Response:**

```json
{
  "blob_ref": "blob://a1b2c3d4e5f6...",
  "hash": "a1b2c3d4e5f6...",
  "size": 245760,
  "mime_type": "image/png"
}
```

### Download Blob

| Method | Endpoint           | Auth          |
| ------ | ------------------ | ------------- |
| `GET`  | `/v1/blobs/{hash}` | API Key / JWT |

**Response:** Raw binary with `Content-Type` header.

### Get Blob Metadata

| Method | Endpoint                | Auth          |
| ------ | ----------------------- | ------------- |
| `GET`  | `/v1/blobs/{hash}/meta` | API Key / JWT |

**Response:**

```json
{
  "hash": "a1b2c3d4e5f6...",
  "size": 245760,
  "mime_type": "image/png",
  "metadata": {
    "width": 1024,
    "height": 768
  }
}
```

---

## Python Example: Upload Image + Store in CDF

```python
import requests

API = "http://localhost:8080"
API_KEY = "dev-key-abc123"
HEADERS = {"X-API-Key": API_KEY}

# 1. Upload image file
with open("photo.png", "rb") as f:
    files = {"file": ("photo.png", f, "image/png")}
    r = requests.post(f"{API}/v1/blobs/upload", files=files, headers=HEADERS)
    blob = r.json()

print(f"Uploaded: {blob['blob_ref']}")

# 2. Store BlobRef in CDF table
record = {
    "namespace": "default",
    "table": "media",
    "data": {
        "title": "Conference Photo",
        "photo": {
            "type": "blob_ref",
            "hash": blob["hash"],
            "mime_type": blob["mime_type"],
            "size": blob["size"]
        },
        "tags": ["conference", "2024"],
        "uploaded_at": "2024-05-24T10:00:00Z"
    }
}

r = requests.post(f"{API}/v1/insert", json=record, headers=HEADERS)
print(f"Stored: {r.json()['row_id']}")
```

---

## Python Example: Upload Audio + Store in CDF

```python
import requests

API = "http://localhost:8080"
HEADERS = {"X-API-Key": "dev-key-abc123"}

# 1. Upload audio file
with open("podcast.mp3", "rb") as f:
    files = {"file": ("podcast.mp3", f, "audio/mpeg")}
    r = requests.post(f"{API}/v1/blobs/upload", files=files, headers=HEADERS)
    blob = r.json()

# 2. Store with embedding (if you have audio embedding model)
record = {
    "namespace": "default",
    "table": "audio_clips",
    "data": {
        "title": "AI Podcast Episode 1",
        "audio": {
            "type": "blob_ref",
            "hash": blob["hash"],
            "mime_type": "audio/mpeg",
            "size": blob["size"],
            "duration_seconds": 1800
        },
        "transcript": "Full transcript text here...",
        "embedding": {
            "model_id": "audio-embedding-v1",
            "values": [0.1, 0.2, ...]  # From audio embedding model
        }
    }
}

r = requests.post(f"{API}/v1/insert", json=record, headers=HEADERS)
print(f"Stored: {r.json()['row_id']}")
```

---

## JavaScript Example: Upload + Display Image

```javascript
const API = "http://localhost:8080";
const API_KEY = "dev-key-abc123";

// Upload image from file input
async function uploadImage(fileInput) {
  const file = fileInput.files[0];
  const form = new FormData();
  form.append("file", file);

  const r = await fetch(`${API}/v1/blobs/upload`, {
    method: "POST",
    headers: { "X-API-Key": API_KEY },
    body: form,
  });
  return await r.json(); // { blob_ref, hash, size, mime_type }
}

// Store in CDF
async function saveImageRecord(blob) {
  const record = {
    namespace: "default",
    table: "gallery",
    data: {
      title: "My Photo",
      image: {
        type: "blob_ref",
        hash: blob.hash,
        mime_type: blob.mime_type,
        size: blob.size,
      },
    },
  };

  const r = await fetch(`${API}/v1/insert`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-API-Key": API_KEY,
    },
    body: JSON.stringify(record),
  });
  return await r.json();
}

// Display image by hash
function getImageUrl(hash) {
  return `${API}/v1/blobs/${hash}?api_key=${API_KEY}`;
}

// Usage in HTML:
// <img src="getImageUrl(record.image.hash)" />
```

---

## JavaScript Example: Upload + Play Audio

```javascript
async function uploadAudio(file) {
  const form = new FormData();
  form.append("file", file);

  const r = await fetch(`${API}/v1/blobs/upload`, {
    method: "POST",
    headers: { "X-API-Key": API_KEY },
    body: form,
  });
  const blob = await r.json();

  // Store in CDF
  await fetch(`${API}/v1/insert`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-API-Key": API_KEY,
    },
    body: JSON.stringify({
      namespace: "default",
      table: "audio_clips",
      data: {
        title: file.name,
        audio: {
          type: "blob_ref",
          hash: blob.hash,
          mime_type: blob.mime_type,
          size: blob.size,
        },
      },
    }),
  });

  return blob.hash;
}

// Play audio
function playAudio(hash) {
  const audio = new Audio(`${API}/v1/blobs/${hash}?api_key=${API_KEY}`);
  audio.play();
}
```

---

## Querying Blobs

### Find all images in a gallery

```python
r = requests.post(f"{API}/v1/query", json={
    "query": "SELECT title, image.hash FROM gallery WHERE image.mime_type LIKE 'image/%'"
}, headers=HEADERS)
```

### Search audio by transcript (semantic search)

```python
r = requests.post(f"{API}/v1/search_text", json={
    "namespace": "default",
    "table": "audio_clips",
    "text": "machine learning discussion",
    "top_k": 5
}, headers=HEADERS)
```

---

## Supported MIME Types

| Category  | MIME Types                                                            |
| --------- | --------------------------------------------------------------------- |
| Images    | `image/png`, `image/jpeg`, `image/gif`, `image/webp`, `image/svg+xml` |
| Audio     | `audio/mpeg`, `audio/wav`, `audio/ogg`, `audio/flac`, `audio/aac`     |
| Video     | `video/mp4`, `video/webm`, `video/ogg`, `video/quicktime`             |
| Documents | `application/pdf`, `application/msword`, `text/plain`                 |
| Archives  | `application/zip`, `application/gzip`                                 |

---

## Size Limits

| Limit         | Value                        |
| ------------- | ---------------------------- |
| Single upload | 100 MB                       |
| Batch upload  | 500 MB total                 |
| Storage       | Limited by MinIO volume size |

---

## Security Notes

1. **Always upload via Gateway** (`:8080`) — never expose MinIO directly
2. **BlobRef uses content hash** — deduplication is automatic
3. **Access control** — use CDF ACL to restrict who can read/write blobs
4. **Encryption** — MinIO supports SSE-S3 encryption at rest
