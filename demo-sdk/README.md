# CDF Frontend Demo Apps

5 demo applications showing how to connect to CDF from different frontend frameworks.

## Prerequisites

CDF must be running locally:

```bash
docker-compose up -d
```

## Demo Apps

| Framework      | Folder             | How to Run                                  | Features                           |
| -------------- | ------------------ | ------------------------------------------- | ---------------------------------- |
| **React**      | `react-demo/`      | `cd react-demo && npm install && npm start` | Health, CRUD, Search, Graph, Admin |
| **Vue**        | `vue-demo/`        | Open `index.html` in browser                | Health, CRUD, Search, Graph, Admin |
| **Angular**    | `angular-demo/`    | `ng serve` (requires Angular CLI)           | Health, CRUD, Search, Graph, Admin |
| **Vanilla JS** | `vanilla-js-demo/` | Open `index.html` in browser                | Health, CRUD, Search, Graph, Admin |
| **Svelte**     | `svelte-demo/`     | Open `index.html` in browser                | Health, CRUD, Search, Graph, Admin |

## Common Features (All Apps)

All 5 demos demonstrate:

| Feature             | CDF API Used             | Data Types                                                        |
| ------------------- | ------------------------ | ----------------------------------------------------------------- |
| **Create**          | `POST /v1/insert`        | Scalar (title, year), Text (abstract), Embedding (384-dim vector) |
| **Read**            | `POST /v1/query`         | All fields returned                                               |
| **Update**          | `PUT /v1/update`         | Scalar update                                                     |
| **Delete**          | `DELETE /v1/rows/{id}`   | Row removal                                                       |
| **Vector Search**   | `POST /v1/search_text`   | Text → embedding → ANN search                                     |
| **Graph Traversal** | `GET /v1/graph/traverse` | Follow citation edges                                             |
| **Add Edge**        | `POST /v1/graph/edges`   | GraphEdge (citations)                                             |
| **Admin**           | `POST /v1/admin/users`   | User creation with RBAC                                           |

## API Endpoint

All demos connect to: `http://localhost:8080`

## CORS Note

If you get CORS errors when opening HTML files directly, serve them via a local server:

```bash
# Python
python -m http.server 3000

# Node
npx serve .

# Or use VS Code Live Server extension
```

Then open `http://localhost:3000` instead of `file://`.
