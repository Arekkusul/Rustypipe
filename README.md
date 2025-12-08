# Rustypipe

Rustypipe is a lightweight, dependency-aware pipeline executor written in Rust.
It allows you to define complex task DAGs (Directed Acyclic Graphs) in YAML and run them locally or with backend support, with concurrency, retries, caching, and graceful shutdown.

---

## Features

- **Dependency-aware DAG execution**: Tasks run only when dependencies are satisfied.
- **Concurrency control**: Limit number of tasks running in parallel.
- **Retries & fail-fast**: Automatic retries with exponential backoff and configurable stop-on-failure.
- **Task output interpolation**: Pass outputs from one task to another using `{{task.output}}` syntax.
- **JSON path access**: Extract nested fields from JSON task outputs using dot notation (`{{task.output.field.subfield}}`).
- **Environment variable integration**: Reference environment variables (`${ENV:VAR_NAME}`) and file contents (`${FILE:/path}`).
- **Intelligent caching**: Cache task results by key to avoid redundant executions.
- **Secrets management**: Secure handling of sensitive data through dedicated secrets configuration.
- **Conditional execution**: Run tasks based on conditions (`when: "{{vars.ENV}} == 'production'"`).
- **Task hooks**: Execute commands before, after, or on failure of tasks.
- **Dry run mode**: Preview pipeline execution without running tasks (`--dry-run` flag).
- **Artifacts & logging**: Capture logs and metadata for every task. Logs and task metadata are saved in `.rustypipe` for reproducibility.
- **Progress indicators**: Real-time visual feedback with progress bars and task status.
- **Run history**: Track and inspect past pipeline executions.
- **Extensible backends**: Supports multiple execution backends:
  - **Local**: Run tasks on the host machine.
  - **SSH**: Run tasks on remote machines over SSH.
  - **Docker**: Run tasks inside Docker containers for reproducible environments.
  - **Kubernetes**: Run tasks in ephemeral Kubernetes pods for cloud-native workflows.
- **Cross-platform**: Works on Windows, Linux, and macOS.

---

## Installation

Clone the repository and build with Cargo:

\`\`\`bash
git clone https://github.com/Arekkusul/Rustypipe.git
cd Rustypipe
cargo build --release
\`\`\`

The binary will be in \`target/release/rustypipe\`.

---

## Quick Start

### Basic Example (Linux/macOS)

Create a pipeline file \`example.yaml\`:

\`\`\`yaml
name: hello_world
concurrency: 2
stop_on_fail: true

tasks:
  - name: greet
    run: "echo 'Hello, World!'"
    backend: local

  - name: timestamp
    depends_on: [greet]
    run: "date"
    backend: local

  - name: combine
    depends_on: [greet, timestamp]
    run: "echo 'Greeted at {{timestamp.output}}'"
    backend: local
\`\`\`

Run it:

\`\`\`bash
rustypipe run example.yaml
\`\`\`

### Basic Example (Windows)

\`\`\`yaml
name: hello_world
concurrency: 2
stop_on_fail: true

tasks:
  - name: greet
    run: "Write-Output 'Hello, World!'"
    backend: local

  - name: timestamp
    depends_on: [greet]
    run: "Get-Date"
    backend: local

  - name: combine
    depends_on: [greet, timestamp]
    run: "Write-Output 'Greeted at {{timestamp.output}}'"
    backend: local
\`\`\`

---

## Advanced Features

### Backend Configuration

Define multiple backends in your pipeline:

\`\`\`yaml
name: multi_backend_pipeline

backends:
  local_machine:
    type: local

  remote_server:
    type: ssh
    host: 192.168.1.100
    user: deploy
    key_path: ~/.ssh/id_rsa
    port: 22

  docker_env:
    type: docker
    image: ubuntu:22.04
    extra_args: ["--network", "host"]

  k8s_cluster:
    type: kubernetes
    image: python:3.11
    namespace: default

tasks:
  - name: local_task
    run: "echo 'Running locally'"
    backend: local_machine

  - name: remote_task
    run: "uname -a"
    backend: remote_server

  - name: container_task
    run: "cat /etc/os-release"
    backend: docker_env

  - name: k8s_task
    run: "python --version"
    backend: k8s_cluster
\`\`\`

### Caching

Cache expensive task results:

\`\`\`yaml
tasks:
  - name: expensive_computation
    run: "sleep 5 && echo 'Computed result: 42'"
    cache_key: "computation-v1"  # Runs once, then cached

  - name: use_cached_result
    depends_on: [expensive_computation]
    run: "echo 'Got: {{expensive_computation.output}}'"
\`\`\`

Clear cache:

\`\`\`bash
rustypipe clear-cache
\`\`\`

### Secrets Management

Store sensitive data separately:

\`\`\`yaml
name: secure_pipeline

secrets:
  API_KEY: "your-secret-key"
  DB_PASSWORD: "your-password"

tasks:
  - name: api_call
    run: "curl -H 'Authorization: Bearer {{API_KEY}}' https://api.example.com"
    backend: local
\`\`\`

**Important**: Never commit secrets to version control. Use environment variables or external secret managers in production.

### Continue on Failure

Allow specific tasks to fail without stopping the pipeline:

\`\`\`yaml
tasks:
  - name: critical_task
    run: "important_script.sh"
    backend: local

  - name: optional_notification
    depends_on: [critical_task]
    run: "send_notification.sh"
    continue_on_fail: true  # Won't stop pipeline if this fails

  - name: cleanup
    depends_on: [critical_task, optional_notification]
    run: "cleanup.sh"
    backend: local
\`\`\`

### Retries with Exponential Backoff

Automatically retry failed tasks:

\`\`\`yaml
tasks:
  - name: flaky_network_call
    run: "curl https://unreliable-api.example.com"
    retries: 3  # Will try up to 4 times total (initial + 3 retries)
    timeout: 30  # Timeout after 30 seconds
    backend: local
\`\`\`

### JSON Output Parsing

Extract nested fields from JSON task outputs using dot notation:

\`\`\`yaml
tasks:
  - name: get_config
    run: "cat config.json"  # Outputs: {"database": {"host": "localhost", "port": 5432}}
    backend: local

  - name: connect_db
    depends_on: [get_config]
    run: "psql -h {{get_config.output.database.host}} -p {{get_config.output.database.port}}"
    backend: local
\`\`\`

### Environment Variables and File Secrets

Reference environment variables and file contents for sensitive data:

\`\`\`yaml
tasks:
  - name: deploy
    run: "kubectl apply -f deployment.yaml --token ${ENV:KUBE_TOKEN}"
    backend: local

  - name: api_call
    run: "curl -H 'Authorization: Bearer ${FILE:/secrets/api_token.txt}' https://api.example.com"
    backend: local
\`\`\`

**Important**: Use environment variables and files for production secrets instead of hardcoding them in YAML.

### Conditional Task Execution

Run tasks only when specific conditions are met:

\`\`\`yaml
secrets:
  ENV: "production"

tasks:
  - name: run_tests
    run: "pytest tests/"
    backend: local

  - name: deploy_staging
    depends_on: [run_tests]
    run: "deploy.sh staging"
    backend: local
    when: "{{vars.ENV}} == 'staging'"

  - name: deploy_production
    depends_on: [run_tests]
    run: "deploy.sh production"
    backend: local
    when: "{{vars.ENV}} == 'production'"
\`\`\`

Supports comparison operators: `==`, `!=`, `>`, `<`, `>=`, `<=`

### Task Hooks

Execute commands before, after, or on failure of tasks:

\`\`\`yaml
tasks:
  - name: database_migration
    before: "echo 'Taking database backup...'"
    run: "python manage.py migrate"
    after: "echo 'Migration completed successfully'"
    on_failure: "python rollback.py && echo 'Migration rolled back'"
    backend: local
\`\`\`

Hooks are useful for:
- Setup and teardown operations
- Notifications
- Cleanup on failure
- Logging and monitoring

### Dry Run Mode

Preview what would be executed without actually running tasks:

\`\`\`bash
rustypipe run pipeline.yaml --dry-run
\`\`\`

Dry run shows:
- Task execution order
- Interpolated commands
- Conditional task skips
- Hooks that would execute
- All task settings

---

## CLI Commands

### Run a Pipeline

\`\`\`bash
rustypipe run pipeline.yaml

# With options
rustypipe run pipeline.yaml --concurrency 8 --stop-on-fail
rustypipe run pipeline.yaml --verbose
\`\`\`

### Validate a Pipeline

Check syntax and DAG validity without executing:

\`\`\`bash
rustypipe validate pipeline.yaml
\`\`\`

### View Run History

List recent pipeline executions:

\`\`\`bash
rustypipe history
rustypipe history --limit 20
\`\`\`

### Show Run Details

Inspect a specific run:

\`\`\`bash
rustypipe show <run-id>
\`\`\`

### Clear Cache

Remove all cached task results:

\`\`\`bash
rustypipe clear-cache
\`\`\`

### Clean Old Runs

Remove old pipeline run artifacts:

\`\`\`bash
rustypipe clean --keep 10  # Keep only 10 most recent runs
\`\`\`

---

## Distributed & Remote Execution

Rustypipe now supports running tasks on remote machines or isolated environments, enabling distributed pipelines:

- **SSH backend**: Execute tasks on remote servers securely.
- **Docker backend**: Run tasks in containerized environments for consistency and isolation.
- **Kubernetes backend**: Run tasks in ephemeral pods within a cluster for cloud-native scalability.

This allows Rustypipe pipelines to leverage multiple machines or cloud resources while keeping configuration simple.

---

## Why Rustypipe?

### Comparison with Jenkins & Airflow

| Feature / Platform           | Rustypipe                        | Jenkins                     | Airflow                       |
|-------------------------------|---------------------------------|-----------------------------|--------------------------------|
| Language                     | Rust                             | Java                        | Python                        |
| Pipeline definition           | YAML                             | GUI / Declarative pipeline  | Python DAGs                   |
| Scheduler                     | CLI only                         | Cron, SCM triggers, Webhooks | DAG scheduler with cron/external triggers |
| Concurrency & Distribution    | Local concurrency, remote backends | Distributed with agents     | Distributed executors (Celery/K8s) |
| Logging & Monitoring          | CLI + file logs + JSON           | Web UI, build history       | Web UI, DAG visualization     |
| Retry & Error Handling        | Task-level retries w/ exponential backoff | Build retries, aborts   | Retry policies, exponential backoff |
| Caching                       | ✅ Built-in content-based caching | Partial (plugins required)  | Partial                        |
| Secrets Management            | ✅ Built-in                      | Credentials plugin         | Connections & Variables       |
| Ecosystem & Integrations      | Minimal (custom backends)        | Thousands of plugins       | Hooks for AWS/GCP/DB/Kafka   |
| UI                            | CLI only                         | Web UI                      | Web UI                        |
| Memory Safety & Speed         | ✅ Rust native                  | ❌ JVM overhead             | ❌ Python runtime             |
| Reproducible artifact tracking| ✅ Full run history             | Partial (needs plugins)     | Partial                        |
| Distributed / Remote Execution| SSH, Docker, Kubernetes         | Agents, nodes               | Celery, Kubernetes, Dask       |

**Key Advantages of Rustypipe**:
- **Speed & Safety**: Rust-native execution with memory safety guarantees and zero-cost abstractions.
- **Full Control**: Customizable pipelines and backends without heavy dependencies.
- **Lightweight**: No heavy web UI, databases, or external dependencies required.
- **Portable**: Works easily on local machines without complex setup.
- **Built-in Intelligence**: Caching, progress tracking, and run history out of the box.

Rustypipe focuses on simplicity, performance, and developer control.

---

## Pipeline YAML Reference

### Top-level Fields

\`\`\`yaml
name: string                  # Pipeline name (optional)
concurrency: integer          # Max concurrent tasks (default: 4)
stop_on_fail: boolean         # Stop on first failure (default: false)

backends:                     # Backend configurations (optional)
  backend_name:
    type: local | ssh | docker | kubernetes
    # ... backend-specific options

secrets:                      # Secrets (optional)
  SECRET_NAME: "value"

tasks:                        # Task definitions (required)
  - name: string              # Unique task name
    run: string               # Command to execute
    depends_on: [string]      # Task dependencies (optional)
    backend: string           # Backend to use (default: local)
    retries: integer          # Retry count (default: 0)
    timeout: integer          # Timeout in seconds (optional)
    cache_key: string         # Cache key template (optional)
    continue_on_fail: boolean # Continue pipeline on failure (default: false)
    when: string              # Conditional expression (optional)
    before: string            # Command to run before task (optional)
    after: string             # Command to run after successful task (optional)
    on_failure: string        # Command to run on task failure (optional)
\`\`\`

### Backend Types

**Local**:
\`\`\`yaml
local_backend:
  type: local
\`\`\`

**SSH**:
\`\`\`yaml
remote:
  type: ssh
  host: hostname or IP
  user: username (optional)
  port: port number (optional, default: 22)
  key_path: path to SSH key (optional)
  extra_args: [additional SSH args] (optional)
\`\`\`

**Docker**:
\`\`\`yaml
container:
  type: docker
  image: docker_image:tag
  extra_args: [additional docker run args] (optional)
\`\`\`

**Kubernetes**:
\`\`\`yaml
k8s:
  type: kubernetes
  image: docker_image:tag
  namespace: k8s_namespace (optional)
  extra_args: [additional kubectl args] (optional)
\`\`\`

---

## Output Interpolation

Reference task outputs in subsequent tasks:

\`\`\`yaml
tasks:
  - name: get_version
    run: "cat version.txt"
    backend: local

  - name: use_version
    depends_on: [get_version]
    run: "echo 'Version is: {{get_version.output}}'"
    backend: local
\`\`\`

Access secrets:

\`\`\`yaml
secrets:
  TOKEN: "abc123"

tasks:
  - name: authenticated_request
    run: "curl -H 'Token: {{TOKEN}}' https://api.example.com"
\`\`\`

---

## Examples

See the \`examples/\` directory:
- \`sample_pipeline_linux.yaml\` - Linux/macOS basic example
- \`sample_pipeline.yaml\` - Windows PowerShell example
- \`advanced_pipeline.yaml\` - Showcases backends, caching, and secrets
- \`conditional_pipeline.yaml\` - Demonstrates conditional task execution
- \`hooks_pipeline.yaml\` - Shows task hooks (before/after/on_failure)

---

## Development

### Running Tests

\`\`\`bash
cargo test
\`\`\`

### Building from Source

\`\`\`bash
cargo build --release
\`\`\`

### Contributing

Contributions are welcome! Please open an issue or pull request on GitHub.

---

## Future Roadmap

Planned improvements (see \`FUTURE.md\`):

1. Web-based Monitoring UI
2. Persistent Scheduler with DAG backfills
3. Centralized logging and artifact tracking
4. Plugin system for custom extensions
5. Advanced error handling strategies
6. Backend manager UI

✅ **Done**: Additional Backends (SSH, Docker, Kubernetes), Caching, Secrets Management, Progress Indicators, Run History

---

## License

MIT License - see [LICENSE](LICENSE) file for details.

Copyright (c) 2025 Arekkusul

---

## Support

For issues, questions, or feature requests, please open an issue on [GitHub](https://github.com/Arekkusul/Rustypipe/issues).
