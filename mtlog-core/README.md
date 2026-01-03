# mtlog-core

Core utilities for mtlog - shared logging infrastructure.

**⚠️ This is an internal crate** - you probably want one of these instead:

- **[`mtlog`](https://crates.io/crates/mtlog)** - For standard multi-threaded logging
- **[`mtlog-tokio`](https://crates.io/crates/mtlog-tokio)** - For async applications with tokio
- **[`mtlog-progress`](https://crates.io/crates/mtlog-progress)** - For progress bars

## Overview

This crate provides the internal infrastructure used by the `mtlog` family of crates. It is not intended to be used directly by end users.

The mtlog family is designed around a simple principle: **logging and progress bars should work seamlessly together in concurrent applications**. This crate provides the shared logging thread and message-passing infrastructure that makes this possible.

## What's Included

This crate contains:
- Core logging message types and channels
- Log writer implementations for stdout and file output
- Shared utilities for log formatting and thread management

## Documentation

For detailed API documentation, visit [docs.rs/mtlog-core](https://docs.rs/mtlog-core).

## License

MIT
