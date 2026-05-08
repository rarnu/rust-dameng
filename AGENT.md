# AGENT.md - Rust Dameng Database Driver

## Project Overview

Build a complete async Rust driver for Dameng (达梦) database implementing sqlx compatibility, using tokio for async I/O. Wire protocol is implemented from scratch based on reverse-engineered protocol captures.

## Working Directory

- Project root: `/Users/rarnu/Code/github/rust-dameng-ex/`
- Read-only reference: `./dm_go/` (NEVER modify)
- Scripts go in `./scripts/`

## DM Database Instance

- Host: 127.0.0.1
- Port: 5236
- User: SYSDBA
- Password: SYSDBA
- Database: SYSDBA
- Test table: SAMPLE (ID INT, NAME VARCHAR)

## Development Rules

1. Follow DESIGN.md strictly for architecture and implementation order
2. Every function MUST have doc comments and at least 3 test cases
3. Use sqlx-standard patterns for tests and API design
4. When stuck on protocol details, reference dm_go source or protocol captures in /tmp/dm_dumps/ and /tmp/proxy_bind.log
5. Strict separation: tests/ for unit tests, examples/ for usage examples
6. All helper scripts go in scripts/ directory — no临时 scripts
7. Git commit after every meaningful change — each commit must compile and pass tests
8. macOS + Linux targets (Apple Silicon + x86_64)

## Protocol Knowledge

The DM wire protocol uses TCP with a 64-byte frame header + variable payload:
- Header: version(4B) + msg_type(2B) + handle(6B) + reserved(10B) + payload_len(2B) + reserved(16B) + checksum(4B) + reserved(20B)
- Key message types: STARTUP(200/228), LOGIN(1/163), READY(3/187), PREPARE/EXEC(5/0), BIND(13/187), COMMIT(8/187), CLOSE(20/187), FETCH(21/0)
- Full protocol details in DESIGN.md

## Workflow

1. Generate DESIGN.md first (already done)
2. Implement phase by phase per DESIGN.md
3. Run `cargo build` and `cargo test` after each phase
4. Commit to git after each successful phase
5. Use delegate_task for parallel subagent work when appropriate
