import React, { useState, useEffect } from "react";

const API_BASE = "http://localhost:8080";

function App() {
  const [health, setHealth] = useState(null);
  const [papers, setPapers] = useState([]);
  const [form, setForm] = useState({ title: "", year: "", abstract: "" });
  const [searchText, setSearchText] = useState("");
  const [searchResults, setSearchResults] = useState([]);
  const [graphStart, setGraphStart] = useState("");
  const [graphPaths, setGraphPaths] = useState([]);
  const [message, setMessage] = useState("");

  useEffect(() => {
    checkHealth();
    loadPapers();
  }, []);

  async function checkHealth() {
    const r = await fetch(`${API_BASE}/health`);
    setHealth(await r.json());
  }

  async function loadPapers() {
    const r = await fetch(`${API_BASE}/v1/query`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ query: "SELECT * FROM papers" }),
    });
    const data = await r.json();
    setPapers(data.results || []);
  }

  async function createPaper(e) {
    e.preventDefault();
    const payload = {
      namespace: "default",
      table: "papers",
      data: {
        title: form.title,
        year: parseInt(form.year),
        abstract: form.abstract,
        embedding: {
          model_id: "all-MiniLM-L6-v2",
          values: Array.from({ length: 384 }, () => Math.random() * 2 - 1),
        },
      },
    };
    const r = await fetch(`${API_BASE}/v1/insert`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    const result = await r.json();
    setMessage(`Created: ${result.row_id}`);
    setForm({ title: "", year: "", abstract: "" });
    loadPapers();
  }

  async function updatePaper(rowId) {
    const newTitle = prompt("New title:");
    if (!newTitle) return;
    await fetch(`${API_BASE}/v1/update`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        namespace: "default",
        table: "papers",
        row_id: rowId,
        data: { title: newTitle },
      }),
    });
    setMessage(`Updated: ${rowId}`);
    loadPapers();
  }

  async function deletePaper(rowId) {
    if (!window.confirm(`Delete ${rowId}?`)) return;
    await fetch(`${API_BASE}/v1/rows/default/papers/${rowId}`, {
      method: "DELETE",
    });
    setMessage(`Deleted: ${rowId}`);
    loadPapers();
  }

  async function doSearch() {
    const r = await fetch(`${API_BASE}/v1/search_text`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        namespace: "default",
        table: "papers",
        text: searchText,
        top_k: 5,
      }),
    });
    const data = await r.json();
    setSearchResults(data.results || []);
  }

  async function traverseGraph() {
    const r = await fetch(
      `${API_BASE}/v1/graph/traverse?start_id=${graphStart}&depth=2&edge_types=cites`,
    );
    const data = await r.json();
    setGraphPaths(data.paths || []);
  }

  return (
    <div style={{ maxWidth: 900, margin: "0 auto", padding: 20 }}>
      <h1>CDF React Demo</h1>

      <section>
        <h2>Health</h2>
        <pre>{JSON.stringify(health, null, 2)}</pre>
      </section>

      <section>
        <h2>Create Paper (with Embedding)</h2>
        <form onSubmit={createPaper}>
          <input
            placeholder="Title"
            value={form.title}
            onChange={(e) => setForm({ ...form, title: e.target.value })}
          />
          <input
            placeholder="Year"
            value={form.year}
            onChange={(e) => setForm({ ...form, year: e.target.value })}
          />
          <input
            placeholder="Abstract"
            value={form.abstract}
            onChange={(e) => setForm({ ...form, abstract: e.target.value })}
          />
          <button type="submit">Insert</button>
        </form>
        {message && <p style={{ color: "green" }}>{message}</p>}
      </section>

      <section>
        <h2>All Papers (Read)</h2>
        <table border="1" cellPadding="8">
          <thead>
            <tr>
              <th>Row ID</th>
              <th>Title</th>
              <th>Year</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {papers.map((p) => (
              <tr key={p.row_id}>
                <td>{p.row_id}</td>
                <td>{p.title}</td>
                <td>{p.year}</td>
                <td>
                  <button onClick={() => updatePaper(p.row_id)}>Update</button>
                  <button onClick={() => deletePaper(p.row_id)}>Delete</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section>
        <h2>Text Search</h2>
        <input
          placeholder="Search text..."
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
        />
        <button onClick={doSearch}>Search</button>
        <ul>
          {searchResults.map((r, i) => (
            <li key={i}>
              {r.title} (score: {r.score?.toFixed(3)})
            </li>
          ))}
        </ul>
      </section>

      <section>
        <h2>Graph Traversal</h2>
        <input
          placeholder="Start node ID"
          value={graphStart}
          onChange={(e) => setGraphStart(e.target.value)}
        />
        <button onClick={traverseGraph}>Traverse</button>
        <pre>{JSON.stringify(graphPaths, null, 2)}</pre>
      </section>
    </div>
  );
}

export default App;
