import { Component, OnInit } from "@angular/core";
import { HttpClient } from "@angular/common/http";

const API = "http://localhost:8080";

@Component({
  selector: "app-root",
  template: `
    <div style="max-width:900px;margin:0 auto;padding:20px">
      <h1>CDF Angular Demo</h1>

      <section>
        <h2>Health</h2>
        <pre>{{ health | json }}</pre>
      </section>

      <section>
        <h2>Create Paper (Scalar + Text + Embedding)</h2>
        <form (ngSubmit)="createPaper()">
          <input [(ngModel)]="form.title" name="title" placeholder="Title" />
          <input
            [(ngModel)]="form.year"
            name="year"
            placeholder="Year"
            type="number"
          />
          <input
            [(ngModel)]="form.abstract"
            name="abstract"
            placeholder="Abstract"
          />
          <button type="submit">Insert</button>
        </form>
        <p *ngIf="message" style="color:green">{{ message }}</p>
      </section>

      <section>
        <h2>All Papers (Read)</h2>
        <table border="1" cellpadding="8">
          <tr>
            <th>Row ID</th>
            <th>Title</th>
            <th>Year</th>
            <th>Actions</th>
          </tr>
          <tr *ngFor="let p of papers">
            <td>{{ p.row_id }}</td>
            <td>{{ p.title }}</td>
            <td>{{ p.year }}</td>
            <td>
              <button (click)="updatePaper(p.row_id)">Update</button>
              <button (click)="deletePaper(p.row_id)">Delete</button>
            </td>
          </tr>
        </table>
      </section>

      <section>
        <h2>Text Search (Vector)</h2>
        <input [(ngModel)]="searchText" placeholder="Search text..." />
        <button (click)="doSearch()">Search</button>
        <ul>
          <li *ngFor="let r of searchResults">
            {{ r.title }} (score: {{ r.score | number: "1.3" }})
          </li>
        </ul>
      </section>

      <section>
        <h2>Graph Traversal</h2>
        <input [(ngModel)]="graphStart" placeholder="Start node ID" />
        <button (click)="traverseGraph()">Traverse</button>
        <pre>{{ graphPaths | json }}</pre>
      </section>

      <section>
        <h2>Admin: Create User</h2>
        <form (ngSubmit)="createUser()">
          <input
            [(ngModel)]="userForm.username"
            name="u"
            placeholder="Username"
          />
          <input [(ngModel)]="userForm.email" name="e" placeholder="Email" />
          <input
            [(ngModel)]="userForm.password"
            name="p"
            placeholder="Password"
            type="password"
          />
          <button type="submit">Create User</button>
        </form>
      </section>
    </div>
  `,
})
export class AppComponent implements OnInit {
  health: any = null;
  papers: any[] = [];
  form = { title: "", year: "", abstract: "" };
  searchText = "";
  searchResults: any[] = [];
  graphStart = "";
  graphPaths: any[] = [];
  userForm = { username: "", email: "", password: "" };
  message = "";

  constructor(private http: HttpClient) {}

  ngOnInit() {
    this.checkHealth();
    this.loadPapers();
  }

  checkHealth() {
    this.http.get(`${API}/health`).subscribe((r) => (this.health = r));
  }

  loadPapers() {
    this.http
      .post(`${API}/v1/query`, { query: "SELECT * FROM papers" })
      .subscribe((data: any) => (this.papers = data.results || []));
  }

  createPaper() {
    const payload = {
      namespace: "default",
      table: "papers",
      data: {
        title: this.form.title,
        year: parseInt(this.form.year),
        abstract: this.form.abstract,
        embedding: {
          model_id: "all-MiniLM-L6-v2",
          values: Array.from({ length: 384 }, () => Math.random() * 2 - 1),
        },
      },
    };
    this.http.post(`${API}/v1/insert`, payload).subscribe((result: any) => {
      this.message = `Created: ${result.row_id}`;
      this.form = { title: "", year: "", abstract: "" };
      this.loadPapers();
    });
  }

  updatePaper(rowId: string) {
    const newTitle = prompt("New title:");
    if (!newTitle) return;
    this.http
      .put(`${API}/v1/update`, {
        namespace: "default",
        table: "papers",
        row_id: rowId,
        data: { title: newTitle },
      })
      .subscribe(() => {
        this.message = `Updated: ${rowId}`;
        this.loadPapers();
      });
  }

  deletePaper(rowId: string) {
    if (!confirm(`Delete ${rowId}?`)) return;
    this.http.delete(`${API}/v1/rows/default/papers/${rowId}`).subscribe(() => {
      this.message = `Deleted: ${rowId}`;
      this.loadPapers();
    });
  }

  doSearch() {
    this.http
      .post(`${API}/v1/search_text`, {
        namespace: "default",
        table: "papers",
        text: this.searchText,
        top_k: 5,
      })
      .subscribe((data: any) => (this.searchResults = data.results || []));
  }

  traverseGraph() {
    this.http
      .get(
        `${API}/v1/graph/traverse?start_id=${this.graphStart}&depth=2&edge_types=cites`,
      )
      .subscribe((data: any) => (this.graphPaths = data.paths || []));
  }

  createUser() {
    this.http
      .post(`${API}/v1/admin/users`, {
        username: this.userForm.username,
        email: this.userForm.email,
        password: this.userForm.password,
        roles: ["developer"],
      })
      .subscribe(() => {
        this.message = `User created: ${this.userForm.username}`;
        this.userForm = { username: "", email: "", password: "" };
      });
  }
}
