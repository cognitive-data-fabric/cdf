/* ============================================
   Cognitive Data Fabric — Landing Page JS
   ============================================ */

// Search index data
const SEARCH_INDEX = [
  {
    title: "Vector Search",
    desc: "Semantic similarity search with HNSW index. <10ms p99 latency.",
    type: "feature",
    url: "#learning",
  },
  {
    title: "Graph Traversal",
    desc: "BFS/DFS with edge-type filtering on CSR graph index.",
    type: "feature",
    url: "#learning",
  },
  {
    title: "Bitemporal Queries",
    desc: "Query data by valid-time and transaction-time dimensions.",
    type: "feature",
    url: "#learning",
  },
  {
    title: "Native Uncertainty",
    desc: "Probabilistic values with confidence scoring built into types.",
    type: "feature",
    url: "#learning",
  },
  {
    title: "RBAC & ACL",
    desc: "5 roles with hierarchical permissions and fine-grained resource ACL.",
    type: "feature",
    url: "#learning",
  },
  {
    title: "JWT Authentication",
    desc: "HMAC-SHA256 tokens with auto-refresh and role claims.",
    type: "feature",
    url: "#learning",
  },
  {
    title: "Python SDK",
    desc: "Full client with CRUD, search, graph, and admin operations.",
    type: "feature",
    url: "#learning",
  },
  {
    title: "CQL Reference",
    desc: "Cognitive Query Language specification with examples.",
    type: "doc",
    url: "https://github.com/cognitive-data-fabric/cdf/blob/main/docs/CQL.md",
  },
  {
    title: "API Documentation",
    desc: "Complete REST and gRPC endpoint reference.",
    type: "doc",
    url: "https://github.com/cognitive-data-fabric/cdf/blob/main/docs/API.md",
  },
  {
    title: "Architecture Guide",
    desc: "System design, data flow, and component details.",
    type: "doc",
    url: "https://github.com/cognitive-data-fabric/cdf/blob/main/docs/ARCHITECTURE.md",
  },
  {
    title: "Contributing Guide",
    desc: "Development setup, code style, and pull request workflow.",
    type: "doc",
    url: "https://github.com/cognitive-data-fabric/cdf/blob/main/CONTRIBUTING.md",
  },
  {
    title: "Quick Start",
    desc: "Get CDF running locally in under 10 minutes.",
    type: "doc",
    url: "https://github.com/cognitive-data-fabric/cdf#quick-start",
  },
  {
    title: "What is CDF?",
    desc: "Unified database replacing vector, graph, relational, and temporal stores.",
    type: "faq",
    url: "#about",
  },
  {
    title: "Why Python 3.10?",
    desc: "sentence-transformers and torch require binary compatibility.",
    type: "faq",
    url: "https://github.com/cognitive-data-fabric/cdf/blob/main/CONTRIBUTING.md",
  },
  {
    title: "Supported Distances",
    desc: "Cosine, Euclidean, Dot Product, and Manhattan metrics.",
    type: "faq",
    url: "#learning",
  },
  {
    title: "Kubernetes Deploy",
    desc: "Helm charts, StatefulSets, Prometheus monitoring.",
    type: "faq",
    url: "#learning",
  },
  {
    title: "First Query Example",
    desc: "Python SDK code for insert and search.",
    type: "code",
    url: "#home",
  },
  {
    title: "CQL Query",
    desc: "SELECT title FROM papers WHERE embedding SIMILAR TO :q",
    type: "code",
    url: "https://github.com/cognitive-data-fabric/cdf/blob/main/docs/CQL.md",
  },
  {
    title: "Grant ACL",
    desc: 'client.admin.grant_acl(\"user-001\", \"table\", \"docs\", [\"read\", \"write\"])',
    type: "code",
    url: "https://github.com/cognitive-data-fabric/cdf/blob/main/docs/API.md",
  },
  {
    title: "Graph Edge",
    desc: 'client.add_edge(\"doc-a\", \"doc-b\", \"cites\", properties={})',
    type: "code",
    url: "https://github.com/cognitive-data-fabric/cdf/blob/main/docs/API.md",
  },
  {
    title: "Create User",
    desc: 'client.admin.create_user(\"alice\", \"a@x.com\", \"pass\", roles=[\"developer\"])',
    type: "code",
    url: "https://github.com/cognitive-data-fabric/cdf/blob/main/docs/API.md",
  },
];

// ============================================
// DOM Ready
// ============================================
document.addEventListener("DOMContentLoaded", () => {
  initNavigation();
  initSearch();
  initDemo();
  initContactForm();
  initScrollAnimations();
});

// ============================================
// Navigation
// ============================================
function initNavigation() {
  const navbar = document.getElementById("navbar");
  const navToggle = document.getElementById("navToggle");
  const navMenu = document.getElementById("navMenu");
  const navLinks = document.querySelectorAll(".nav-link");

  // Mobile toggle
  navToggle?.addEventListener("click", () => {
    navMenu.classList.toggle("active");
    const spans = navToggle.querySelectorAll("span");
    if (navMenu.classList.contains("active")) {
      spans[0].style.transform = "rotate(45deg) translate(5px, 5px)";
      spans[1].style.opacity = "0";
      spans[2].style.transform = "rotate(-45deg) translate(5px, -5px)";
    } else {
      spans[0].style.transform = "";
      spans[1].style.opacity = "";
      spans[2].style.transform = "";
    }
  });

  // Scroll spy + navbar shrink
  let lastScroll = 0;
  window.addEventListener("scroll", () => {
    const currentScroll = window.scrollY;

    // Shrink navbar
    if (currentScroll > 50) {
      navbar.style.backdropFilter = "blur(30px)";
      navbar.style.background = "rgba(10, 10, 15, 0.9)";
    } else {
      navbar.style.backdropFilter = "blur(20px)";
      navbar.style.background = "rgba(10, 10, 15, 0.7)";
    }

    // Active section highlighting
    const sections = document.querySelectorAll("section[id]");
    sections.forEach((section) => {
      const rect = section.getBoundingClientRect();
      if (rect.top <= 100 && rect.bottom > 100) {
        navLinks.forEach((link) => {
          link.classList.remove("active");
          if (link.getAttribute("href") === `#${section.id}`) {
            link.classList.add("active");
          }
        });
      }
    });

    lastScroll = currentScroll;
  });

  // Close mobile menu on link click
  navLinks.forEach((link) => {
    link.addEventListener("click", () => {
      navMenu.classList.remove("active");
      const spans = navToggle.querySelectorAll("span");
      spans[0].style.transform = "";
      spans[1].style.opacity = "";
      spans[2].style.transform = "";
    });
  });
}

// ============================================
// Search
// ============================================
function initSearch() {
  const searchInput = document.getElementById("globalSearch");
  const searchResults = document.getElementById("searchResults");
  const searchTags = document.querySelectorAll(".search-tag");
  let currentFilter = "all";

  if (!searchInput || !searchResults) return;

  // Filter tags
  searchTags.forEach((tag) => {
    tag.addEventListener("click", () => {
      searchTags.forEach((t) => t.classList.remove("active"));
      tag.classList.add("active");
      currentFilter = tag.dataset.filter;
      performSearch(searchInput.value, currentFilter);
    });
  });
  searchTags[0]?.classList.add("active");

  // Search input
  let debounceTimer;
  searchInput.addEventListener("input", () => {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      performSearch(searchInput.value, currentFilter);
    }, 150);
  });

  // Keyboard shortcut
  document.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "k") {
      e.preventDefault();
      searchInput.focus();
    }
  });

  function performSearch(query, filter) {
    if (!query.trim()) {
      searchResults.innerHTML =
        '<div class="demo-placeholder">Start typing to search features, docs, FAQs, and code...</div>';
      return;
    }

    const q = query.toLowerCase();
    let results = SEARCH_INDEX.filter((item) => {
      const match =
        item.title.toLowerCase().includes(q) ||
        item.desc.toLowerCase().includes(q);
      const filterMatch = filter === "all" || item.type === filter;
      return match && filterMatch;
    });

    if (results.length === 0) {
      searchResults.innerHTML =
        '<div class="demo-placeholder">No results found. Try different keywords.</div>';
      return;
    }

    searchResults.innerHTML = results
      .map(
        (item) => `
            <div class="search-result-item" data-url="${item.url}">
                <h4>${highlightMatch(item.title, q)}</h4>
                <p>${highlightMatch(item.desc, q)}</p>
                <span class="result-tag ${item.type}">${item.type}</span>
            </div>
        `,
      )
      .join("");

    // Click handlers
    searchResults.querySelectorAll(".search-result-item").forEach((el) => {
      el.addEventListener("click", () => {
        const url = el.dataset.url;
        if (url.startsWith("http")) {
          window.open(url, "_blank");
        } else {
          window.location.href = url;
        }
      });
    });
  }

  function highlightMatch(text, query) {
    const regex = new RegExp(`(${escapeRegex(query)})`, "gi");
    return text.replace(
      regex,
      '<mark style="background: rgba(168,85,247,0.3); color: #c084fc; border-radius: 3px; padding: 0 2px;">$1</mark>',
    );
  }

  function escapeRegex(str) {
    return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }

  // Initial state
  searchResults.innerHTML =
    '<div class="demo-placeholder">Start typing to search features, docs, FAQs, and code...</div>';
}

// ============================================
// Interactive Demo
// ============================================
function initDemo() {
  const demoRun = document.getElementById("demoRun");
  const demoQuery = document.getElementById("demoQuery");
  const demoOutput = document.getElementById("demoOutput");

  if (!demoRun || !demoQuery || !demoOutput) return;

  demoRun.addEventListener("click", () => {
    const query = demoQuery.value.trim();
    if (!query) return;

    demoOutput.innerHTML =
      '<div class="demo-placeholder">⏳ Parsing query...</div>';

    setTimeout(() => {
      const plan = generateExecutionPlan(query);
      demoOutput.innerHTML = plan;
    }, 600);
  });

  demoQuery.addEventListener("keydown", (e) => {
    if (e.key === "Enter") demoRun.click();
  });

  function generateExecutionPlan(query) {
    const steps = [];
    const lower = query.toLowerCase();

    // Parse detection
    steps.push("🔍 <strong>Parse</strong> — ANTLR grammar → AST");

    // Detect clauses
    if (lower.includes("select"))
      steps.push(
        "📋 <strong>Project</strong> — Extract columns from PolyValue",
      );
    if (lower.includes("from"))
      steps.push("📁 <strong>Source</strong> — Resolve table + namespace");
    if (lower.includes("where")) {
      steps.push("🎯 <strong>Filter</strong> — Apply predicates");
      if (lower.includes("similar")) {
        steps.push(
          "🧬 <strong>Vector Search</strong> — HNSW ANN index scan (ef=200, M=16)",
        );
        steps.push(
          "📊 <strong>Rerank</strong> — Cosine similarity + scalar filters",
        );
      }
      if (lower.includes("confidence")) {
        steps.push(
          "🎲 <strong>Probabilistic</strong> — Distribution confidence check",
        );
      }
    }
    if (lower.includes("path") || lower.includes("traverse")) {
      steps.push(
        "🔗 <strong>Graph</strong> — CSR BFS traversal with edge filtering",
      );
    }
    if (lower.includes("at time")) {
      steps.push(
        "⏰ <strong>Temporal</strong> — BTree valid-time index lookup",
      );
    }
    if (lower.includes("order by"))
      steps.push("📈 <strong>Sort</strong> — Merge sort on result set");
    if (lower.includes("limit"))
      steps.push("✂️ <strong>Limit</strong> — Top-K truncation");

    // Shard routing
    steps.push("🌐 <strong>Route</strong> — Consistent hash → Storage Node 3");
    steps.push(
      "⚡ <strong>Execute</strong> — Fan-out to 3 shards, collect results",
    );
    steps.push("✅ <strong>Done</strong> — Estimated: ~15ms");

    return (
      steps
        .map((s, i) => {
          const indent = s.includes("strong") ? "" : "&nbsp;&nbsp;";
          return `<div style="opacity: 0; animation: fadeIn 0.3s ${i * 0.1}s forwards;">${indent}${s}</div>`;
        })
        .join("") +
      `
            <style>
                @keyframes fadeIn { to { opacity: 1; } }
            </style>
        `
    );
  }
}

// ============================================
// Contact Form
// ============================================
function initContactForm() {
  // Inline discussion starter for GitHub Discussions
  const discBtn = document.getElementById("discBtn");
  if (discBtn) {
    discBtn.addEventListener("click", () => {
      const title = document.getElementById("discTitle").value.trim();
      const body = document.getElementById("discBody").value.trim();

      if (!title || !body) {
        showToast("Please fill in both title and message.");
        return;
      }

      const encTitle = encodeURIComponent(title);
      const encBody = encodeURIComponent(
        `${body}\n\n---\n_Posted via [Cognitive Data Fabric](https://cognitive-data-fabric.github.io/) website._`,
      );
      const url = `https://github.com/cognitive-data-fabric/cdf/discussions/new?category=general&title=${encTitle}&body=${encBody}`;
      window.open(url, "_blank");

      document.getElementById("discTitle").value = "";
      document.getElementById("discBody").value = "";
      showToast("Opening GitHub Discussions...");
    });
  }
}

function showToast(message) {
  const toast = document.getElementById("toast");
  if (!toast) return;
  toast.textContent = message;
  toast.classList.add("show");
  setTimeout(() => toast.classList.remove("show"), 3000);
}

// ============================================
// Scroll Animations (Intersection Observer)
// ============================================
function initScrollAnimations() {
  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.style.opacity = "1";
          entry.target.style.transform = "translateY(0)";
        }
      });
    },
    { threshold: 0.1 },
  );

  // Animate cards on scroll
  document
    .querySelectorAll(".learn-card, .mission-card, .feature, .channel")
    .forEach((el) => {
      el.style.opacity = "0";
      el.style.transform = "translateY(20px)";
      el.style.transition = "opacity 0.5s ease, transform 0.5s ease";
      observer.observe(el);
    });
}
