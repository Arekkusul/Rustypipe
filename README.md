# Rustypipe

Rustypipe is a lightweight, dependency-aware pipeline executor written in Rust.  
It allows you to define complex task DAGs (Directed Acyclic Graphs) in YAML and run them locally or with backend support, with concurrency, retries, caching, and graceful shutdown.

---

## Features

- **Dependency-aware DAG execution**: Tasks run only when dependencies are satisfied.
- **Concurrency control**: Limit number of tasks running in parallel.
- **Retries & fail-fast**: Automatic retries and configurable stop-on-failure.
- **Task output interpolation**: Pass outputs from one task to another.
- **Artifacts & logging**: Capture logs and metadata for every task. [Logs and task metadata saved in `.rustypipe` for reproducibility.]
- **Extensible backends**: Currently supports local execution; extendable for SSH, Docker, etc.
- **Cross-platform**: Works on Windows and Linux.

---

## Why Rustypipe?

Compared to other pipeline tools:

## Comparison with Jenkins & Airflow

| Feature / Platform          | Rustypipe               | Jenkins                     | Airflow                       |
|-----------------------------|-----------------------|-----------------------------|--------------------------------|
| Language                    | Rust                  | Java                        | Python                        |
| Pipeline definition          | YAML                  | GUI / Declarative pipeline  | Python DAGs                   |
| Scheduler                   | CLI only             | Cron, SCM triggers, Webhooks | DAG scheduler with cron/external triggers |
| Concurrency & Distribution   | Local concurrency      | Distributed with agents     | Distributed executors (Celery/K8s) |
| Logging & Monitoring         | Local logs & JSON      | Web UI, build history       | Web UI, DAG visualization     |
| Retry & Error Handling       | Task-level retries     | Build retries, aborts       | Retry policies, exponential backoff |
| Ecosystem & Integrations     | Minimal (custom backends) | Thousands of plugins       | Hooks for AWS/GCP/DB/Kafka   |
| UI                           | CLI only              | Web UI                      | Web UI                        |
| Memory Safety & Speed        | ✅ Rust native        | ❌ JVM overhead             | ❌ Python runtime             |
| Reproducible artifact tracking | ✅                   | Partial (needs plugins)     | Partial                        |

**Key Advantages of Rustypipe**:
- **Speed & Safety**: Rust-native execution with memory safety guarantees.  
- **Full Control**: Customizable pipelines and backends.  
- **Lightweight**: No heavy web UI, databases, or plugins required.  
- **Portable**: Works easily on local machines without external dependencies.

Rustypipe focuses on simplicity, performance, and developer control.

---

## Installation

Clone the repository and build with Cargo:

```bash
git clone https://github.com/Arekkusul/Rustypipe.git
cd Rustypipe
cargo build --release
