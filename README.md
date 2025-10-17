# RustyPipe — MVP

Run:

```bash
cargo run -- run examples/sample_pipeline.yaml
```

This MVP parses a simple YAML pipeline, constructs a DAG by `depends_on`, and executes tasks concurrently when ready.
