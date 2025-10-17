# Rustypipe

Rustypipe is a lightweight, dependency-aware pipeline executor written in Rust.  
It allows you to define complex task DAGs (Directed Acyclic Graphs) in YAML and run them locally or with backend support, with concurrency, retries, caching, and graceful shutdown.

---

## Features

- **Dependency-aware DAG execution**: Tasks run only when dependencies are satisfied.
- **Concurrency control**: Limit number of tasks running in parallel.
- **Retries & fail-fast**: Automatic retries and configurable stop-on-failure.
- **Task output interpolation**: Pass outputs from one task to another.
- **Artifacts & logging**: Capture logs and metadata for every task.
- **Extensible backends**: Currently supports local execution; extendable for SSH, Docker, etc.
- **Cross-platform**: Works on Windows and Linux.

---

## Why Rustypipe?

Compared to other pipeline tools:

| Feature | Rustypipe | Airflow | Jenkins |
|---------|-----------|---------|---------|
| Lightweight | ✅ | ❌ (requires full Airflow setup) | ❌ (heavy) |
| Easy local execution | ✅ | ❌ | ❌ |
| Written in Rust (fast, safe, single binary) | ✅ | ❌ | ❌ |
| Concurrency control + retries built-in | ✅ | ✅ | Partial |
| YAML-based DAGs | ✅ | ✅ | ❌ |

Rustypipe focuses on simplicity, performance, and developer control.

---

## Installation

Clone the repository and build with Cargo:

```bash
git clone https://github.com/Arekkusul/Rustypipe.git
cd Rustypipe
cargo build --release
