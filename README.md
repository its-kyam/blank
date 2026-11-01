# Blank, the web framework

![CI](https://img.shields.io/github/actions/workflow/status/its-kyam/blank/ci.yml?branch=master&style=plastic&logo=github&label=CI)
![Blazing Fast](https://img.shields.io/badge/Blazing-Fast-orange?style=plastic&logo=rocket)
![Super Simple](https://img.shields.io/badge/Super-Simple-blue?style=plastic&logo=sparkles)
![Crazily Small](https://img.shields.io/badge/Crazily-Small-brightgreen?style=plastic&logo=feather)

Blank is a lightweight, cross-platform web framework written in Rust.

```rust
use blank::Blank;

fn main() {
    let mut server = Blank::new();

    server.route_func("/", |_rq, out| {
        out.text("Hello, world!");
    });

    server.serve().unwrap();
}
```

### Contribute

```bash
git clone https://github.com/its-kyam/blank

cd blank
cargo test
```

### Install

```bash
cargo add blank-web@latest
```

### About

This project implements its own coroutines, it doesn't use **Tokio** at all!

DX is **delightful**, and amazing, try it out and see!

### Platform support

- Linux
- macOS
- Windows

### License

MIT OR Apache-2.0
