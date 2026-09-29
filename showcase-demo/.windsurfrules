

# SpacetimeDB Core Concepts

SpacetimeDB is a relational database that is also a server. It lets you upload application logic directly into the database as modules, eliminating the traditional web/game server layer entirely. Rust, C#, and C++ modules compile to WebAssembly, while TypeScript modules run on V8.

---

## Critical Rules

1. **Reducers are transactional.** They do not return data to callers. Use subscriptions to read data.
2. **Reducers must be deterministic.** Do not use filesystem, network, external clocks, or external random sources in reducers. Use the reducer context (`ctx`) for SpacetimeDB-provided timestamp and deterministic random values.
3. **Read data via tables/subscriptions**, not reducer return values. Clients get data through subscribed queries.
4. **Auto-increment IDs are not sequential.** Gaps are normal, do not use for ordering. Use timestamps or explicit sequence columns.
5. **`ctx.sender` is the authenticated principal.** Never trust identity passed as arguments.

---

## Feature Implementation Checklist

1. **Backend:** Define table(s) to store the data
2. **Backend:** Define reducer(s) to mutate the data
3. **Client:** Subscribe to the table(s)
4. **Client:** Call the reducer(s) from UI
5. **Client:** Render the data from the table(s)

---

## Debugging Checklist

1. Is SpacetimeDB server running? (`spacetime start`)
2. Is the module published? (`spacetime publish`)
3. Are client bindings generated? (`spacetime generate`)
4. Check server logs for errors (`spacetime logs <db-name>`)
5. Is the reducer actually being called from the client?

---

## Tables

- **Private tables** (default): Only accessible by reducers and the database owner.
- **Public tables**: Exposed for client read access through subscriptions. Writes still require reducers.

Organize data by access pattern, not by entity:

```
Player          PlayerState         PlayerStats
id         <--  player_id           player_id
name            position_x          total_kills
                position_y          total_deaths
                velocity_x          play_time
```

## Reducers

Reducers are transactional functions that modify database state. They run atomically, cannot interact with the outside world, and do not return data to callers. See the language-specific server skills for syntax.

## Event Tables

Event tables broadcast reducer-specific data to clients. Rows are never stored in the client cache (`count()` returns 0, `iter()` yields nothing); only `onInsert` callbacks fire.

## Subscriptions

Subscriptions replicate database rows to clients in real-time.

1. **Subscribe**: Register SQL queries describing needed data
2. **Receive initial data**: All matching rows are sent immediately
3. **Receive updates**: Real-time updates when subscribed rows change
4. **React to changes**: Use callbacks (`onInsert`, `onDelete`, `onUpdate`)

Best practices:
- Group subscriptions by lifetime
- Subscribe before unsubscribing when updating subscriptions
- Avoid overlapping queries
- Use indexes for efficient queries

## Modules

Modules contain application logic that runs inside the database.

- **Tables**: Define the data schema
- **Reducers**: Define callable functions that modify state
- **Event Tables**: Broadcast reducer-specific data to clients
- **Views**: Read-only functions that expose computed subsets of data to clients
- **Procedures**: (Unstable) Functions that can have side effects (HTTP requests, `ctx.withTx`)

Server-side modules can be written in: Rust, C#, TypeScript, C++

Lifecycle: Write → Compile → Publish (`spacetime publish`) → Hot-swap (republish without disconnecting clients)

## Identity

- **Identity**: A long-lived, globally unique identifier for a user.
- **ConnectionId**: Identifies a specific client connection.
- Always use `ctx.sender` / `ctx.Sender` / `ctx.sender()` for authorization.

SpacetimeDB works with many OIDC providers, including SpacetimeAuth (built-in), Auth0, Clerk, Keycloak, Google, and GitHub.




# SpacetimeDB CLI

Use this skill when the user needs help with the `spacetime` CLI tool - initializing projects, building modules, publishing databases, querying data, managing servers, or troubleshooting CLI issues.

## Quick Reference

### Project Initialization & Development

```bash
# Initialize new project
spacetime init my-project --lang rust|csharp|typescript|cpp
spacetime init my-project --template <template-id>

# Build module
spacetime build                    # release build
spacetime build --debug            # faster iteration, slower runtime

# Dev mode (auto-rebuild, auto-publish, generates bindings)
spacetime dev
spacetime dev --client-lang typescript --module-bindings-path ./client/src/module_bindings

# Generate client bindings
spacetime generate --lang typescript|csharp|rust --out-dir ./bindings --module-path ./server
spacetime generate --lang unrealcpp --uproject-dir ./MyGame --module-path ./server --unreal-module-name MyGame
```

### Publishing & Deployment

```bash
# Publish to Maincloud (default)
spacetime publish my-database --yes

# Publish to local server
spacetime publish my-database --server local --yes

# Clear database and republish
spacetime publish my-database --delete-data=always --yes
```

### Database Interaction

```bash
# SQL queries
spacetime sql my-database "SELECT * FROM users"
spacetime sql my-database --interactive   # REPL mode

# Call reducers (each argument is a separate positional arg)
spacetime call my-database my_reducer '"value"' '123'

# Subscribe to changes
spacetime subscribe my-database "SELECT * FROM users" --num-updates 10

# View logs
spacetime logs my-database -f              # follow logs
spacetime logs my-database -n 100          # up to 100 log lines

# Describe schema
spacetime describe my-database --json
spacetime describe my-database table users --json
spacetime describe my-database reducer my_reducer --json
```

### Database Management

```bash
# List databases
spacetime list

# Delete database
spacetime delete my-database

# Rename database
spacetime rename <database-identity> --to new-name
```

### Server Management

```bash
# List configured servers
spacetime server list

# Add server
spacetime server add local --url http://localhost:3000 --default
spacetime server add myserver --url https://my-spacetime.example.com

# Set default server
spacetime server set-default local

# Test connectivity
spacetime server ping local

# Start local instance
spacetime start

# Clear local data
spacetime server clear
```

### Authentication

```bash
# Login (opens browser)
spacetime login

# Login with token
spacetime login --token <token>

# Show login status
spacetime login show

# Logout
spacetime logout
```

## Default Servers

| Name | URL | Description |
|------|-----|-------------|
| `maincloud` | `https://maincloud.spacetimedb.com` | Production cloud (default) |
| `local` | `http://127.0.0.1:3000` | Local development server |

## Common Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--server` | `-s` | Target server (nickname, hostname, or URL) |
| `--yes` | `-y` | Non-interactive mode (skip confirmations) |
| `--anonymous` | | Use anonymous identity |
| `--module-path` | `-p` | Path to module project |

## Troubleshooting

### "Not logged in"
```bash
spacetime login
# Or use --anonymous for public operations
```

### "Server not responding"
```bash
spacetime server ping <server>
# For local: ensure spacetime start is running
```

### "Schema conflict"
```bash
# Clear data and republish
spacetime publish my-db --delete-data=always --yes
```

### "Build failed"
```bash
# Check Rust/C# toolchain
rustup show
# For Rust modules, ensure wasm32-unknown-unknown target
rustup target add wasm32-unknown-unknown
```

## Module Languages

**Server-side (modules):** Rust, C#, TypeScript, C++
**Client SDKs:** TypeScript, C#, Rust, Unreal Engine
**CLI `generate` targets:** TypeScript, C#, Rust, Unreal C++




# SpacetimeDB Rust SDK Reference

## Imports

```rust
use spacetimedb::{
    procedure, reducer, table, Filter, Identity, ProcedureContext, Query,
    ReducerContext, SpacetimeType, Table, ConnectionId, ScheduleAt,
    TimeDuration, Timestamp, Uuid,
};
```

**`Table` is required.** Without it, `ctx.db.*.insert()`, `.iter()`, `.find()` etc. won't compile (`no method named 'insert' found`).

## Tables

`#[spacetimedb::table(...)]` on a `pub struct`. `accessor` must be snake_case:

```rust
#[spacetimedb::table(accessor = entity, public)]
pub struct Entity {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub owner: Identity,
    pub name: String,
    #[index(btree)]
    pub active: bool,
}
```

Options: `accessor = snake_case` (required), `public`, `scheduled(reducer_fn)`, `index(...)`

`ctx.db` accessors use the `accessor` name (snake_case).

## Column Types

| Rust type | Notes |
|-----------|-------|
| `u8` / `u16` / `u32` / `u64` / `u128` | unsigned integers |
| `i8` / `i16` / `i32` / `i64` / `i128` | signed integers |
| `spacetimedb::sats::u256` / `spacetimedb::sats::i256` | 256-bit integers |
| `f32` / `f64` | floats |
| `bool` | boolean |
| `String` | text |
| `Vec<T>` | list/array |
| `Identity` | user identity |
| `ConnectionId` | connection handle |
| `Timestamp` | server timestamp (microseconds since epoch) |
| `TimeDuration` | duration in microseconds |
| `Uuid` | UUID |
| `Option<T>` | nullable column |

## Column Attributes

```rust
#[primary_key]          // primary key
#[auto_inc]             // auto-increment (use 0 as placeholder on insert)
#[unique]               // unique constraint
#[index(btree)]         // btree index (enables .filter() on this column)
#[default(true)]        // migration-safe default for a newly appended column
```

Defaults support compatible addition of a newly appended field. Do not place `#[default(...)]` on primary-key, unique, or auto-increment columns.

## Indexes

Prefer `#[index(btree)]` inline for single-column. Multi-column uses table-level:

```rust
// Inline (preferred for single-column):
#[index(btree)]
pub author_id: u64,
// Access: ctx.db.post().author_id().filter(author_id)

// Multi-column (table-level):
#[spacetimedb::table(accessor = membership, public,
    index(accessor = by_group_user, btree(columns = [group_id, user_id]))
)]
pub struct Membership { pub group_id: u64, pub user_id: Identity, ... }
// Access: ctx.db.membership().by_group_user().filter((group_id, &user_id))
```

When you frequently look up rows by multiple columns, prefer a multi-column index over filtering by one column and looping over the results.

## Reducers

```rust
#[spacetimedb::reducer]
pub fn create_entity(ctx: &ReducerContext, name: String) {
    ctx.db.entity().insert(Entity { id: 0, owner: ctx.sender(), name, active: true });
}

// Reducers can return Result<(), String> or Result<(), E> where E: Display
#[spacetimedb::reducer]
pub fn validate_entity(ctx: &ReducerContext, name: String) -> Result<(), String> {
    if name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    ctx.db.entity().try_insert(Entity { id: 0, owner: ctx.sender(), name, active: true })?;
    Ok(())
}
```

Note: `insert()` panics on constraint violations. Use `try_insert()` with `?` when returning `Result`.

## DB Operations

```rust
ctx.db.entity().insert(Entity { id: 0, name: "Sample".into() });  // Insert (0 for autoInc)
ctx.db.entity().id().find(entity_id);                              // Find by PK → Option<Entity>
ctx.db.entity().identity().find(ctx.sender());                     // Find by unique column → Option<Entity>
ctx.db.item().author_id().filter(author_id);                       // Filter by index → iterator
ctx.db.entity().iter();                                            // All rows → iterator
ctx.db.entity().count();                                           // Count rows
ctx.db.entity().id().update(Entity { name: new_name, ..existing }); // Update (override + spread)
ctx.db.entity().id().delete(entity_id);                            // Delete by PK
ctx.db.entity().name().delete("Alice".to_string());                // Delete by indexed String column
```

Note: `iter()` and `filter()` return iterators. Collect to Vec if you need `.sort()`, `.filter()`, `.map()`.

Range queries on btree indexes: `filter(18..=65)`, `filter(18..)`, `filter(..18)`.

String column accessors operate on the column's owned `String` type, not `&str`. Pass a `String` or `&String` to `find` and `delete`; index filters borrow the key, as in `ctx.db.product().category().filter(&"hardware".to_string())`.

## Lifecycle Hooks

```rust
#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) { ... }

#[spacetimedb::reducer(client_connected)]
pub fn on_connect(ctx: &ReducerContext) { ... }

#[spacetimedb::reducer(client_disconnected)]
pub fn on_disconnect(ctx: &ReducerContext) { ... }
```

The current connection ID is available through `ctx.connection_id()` (not a public field) and may be absent outside connection-scoped calls.

## Views

```rust
// Anonymous view (same result for all clients):
use spacetimedb::{view, AnonymousViewContext};

#[view(accessor = active_users, public)]
fn active_users(ctx: &AnonymousViewContext) -> Vec<Entity> {
    ctx.db.entity().active().filter(true).collect()
}

// Per-user view (result varies by sender):
use spacetimedb::{view, ViewContext};

#[view(accessor = my_profile, public)]
fn my_profile(ctx: &ViewContext) -> Option<Entity> {
    ctx.db.entity().identity().find(ctx.sender())
}
```

Procedural-view table handles support indexed `find` and `filter` access, but not full-table `iter()`. Start a procedural view from an appropriate table index.

Declare a procedural view primary key in the view attribute:

```rust
#[view(accessor = catalog_entry, public, primary_key = sku)]
fn catalog_entry(ctx: &AnonymousViewContext) -> Vec<CatalogEntry> { ... }
```

Procedural-view primary keys are explicit schema metadata. Add one only when the view itself is required to expose a primary key; a source table's primary key is not inherited by the view.

Query-builder views use `ViewContext`, `ctx.from`, and return `impl Query<Row>`. Use `filter` for predicates and `right_semijoin` when the result should contain right-side rows that have a matching left-side row:

```rust
ctx.from.article().filter(|article| article.published.eq(true))
ctx.from.subscription().right_semijoin(
    ctx.from.account(),
    |subscription, account| subscription.account_id.eq(account.id),
)
```

Query-builder comparisons against a `String` column take an owned `String`, for example `row.label.eq("active".to_string())`.

## Client Visibility Filters

```rust
use spacetimedb::Filter;

#[spacetimedb::client_visibility_filter]
const PRIVATE_NOTE_FILTER: Filter =
    Filter::Sql("SELECT * FROM owned_row WHERE owner = :sender");
```

## Reducer Context API

`ReducerContext` (`ctx`) is the only source of sender identity, time, and randomness; stdlib clocks and RNG are unavailable in modules.

```rust
// Auth: ctx.sender() is the caller's Identity
if row.owner != ctx.sender() {
    panic!("unauthorized");
    // or: return Err(anyhow::anyhow!("unauthorized"));
}

// Server timestamp (deterministic per reducer call)
ctx.db.item().insert(Item { id: 0, owner: ctx.sender(), created_at: ctx.timestamp, .. });

// Timestamp arithmetic
let expiry = ctx.timestamp + TimeDuration::from_micros(delay_micros);

// Deterministic RNG: ctx.random() for a single value, ctx.rng() for the rand::Rng trait
use spacetimedb::rand::Rng;
let n: u32 = ctx.random();                    // random u32
let roll: u32 = ctx.rng().gen_range(1..=6);   // any rand::Rng method

// Client: Timestamp → milliseconds since epoch
timestamp.to_micros_since_unix_epoch() / 1000
```

## Scheduled Tables

```rust
#[spacetimedb::table(accessor = tick_timer, scheduled(tick), public)]
pub struct TickTimer {
    #[primary_key]
    #[auto_inc]
    scheduled_id: u64,
    scheduled_at: spacetimedb::ScheduleAt,
}

#[spacetimedb::reducer]
pub fn tick(ctx: &ReducerContext, timer: TickTimer) {
    // timer row is auto-deleted after this reducer runs
}

// One-time: fires once at a specific time
let at = ScheduleAt::Time(ctx.timestamp + std::time::Duration::from_secs(10));
// Repeating: fires on an interval
let at = ScheduleAt::Interval(std::time::Duration::from_secs(5).into());

ctx.db.tick_timer().insert(TickTimer { scheduled_id: 0, scheduled_at: at });
```

Construct a connection ID from a numeric representation with `ConnectionId::from_u128(value)`.

## Procedures and HTTP

Procedures use `&mut ProcedureContext` and may return typed values:

```rust
use spacetimedb::{procedure, ProcedureContext, SpacetimeType};

#[derive(SpacetimeType)]
pub struct ResultValue { pub value: String }

#[procedure]
pub fn inspect(_ctx: &mut ProcedureContext, input: String) -> ResultValue {
    ResultValue { value: input }
}
```

Outbound HTTP is available through `ctx.http`. Convenience methods such as `get` return a response, and other methods use `Request::builder()` with `ctx.http.send(request)`. `response.status()` returns a `StatusCode`; use `is_success()` to test it or `as_u16()` when a numeric status is needed. Responses are not cloneable, so inspect status and headers before consuming the body with `response.into_body().into_string_lossy()`. Header values use the fallible `to_str()` conversion; they do not provide `as_str()`.

Open short database transactions with `ctx.with_tx(|tx| ...)`. Access tables inside the callback through `tx.db`, not directly on `tx`. It returns the callback's value directly, so do not call `unwrap` or `expect` on the result unless the callback itself returns a `Result`. The callback implements `Fn`, so clone captured owned values when storing them rather than moving them out of the closure. Perform network I/O before opening the transaction.

For an outbound request without a convenience method:

```rust
let request = Request::builder()
    .method("POST")
    .uri(url)
    .body(Body::from_bytes(data))
    .unwrap();
let response = ctx.http.send(request).expect("request failed");
```

Scheduled procedures use the ordinary scheduled-table shape, but the scheduled function is marked `#[procedure]` and receives `&mut ProcedureContext` plus the scheduled row.

Inbound HTTP uses handler functions and one router:

```rust
use spacetimedb::http::{handler, router, Body, HandlerContext, Request, Response, Router};

#[handler]
fn health(_ctx: &mut HandlerContext, _request: Request) -> Response {
    Response::builder().status(200).body(Body::from_bytes("ok")).unwrap()
}

#[router]
fn routes() -> Router { Router::new().get("/health", health) }
```

`HandlerContext` does not expose `db`; open database access with `ctx.with_tx(|tx| ...)`. HTTP response bodies must own their data or borrow `'static` data, so convert dynamic borrowed text to an owned `String` before passing it to `Body::from_bytes`.

## Logging

```rust
log::info!("Player connected: {:?}", ctx.sender());
log::warn!("Low health: {}", hp);
log::error!("Failed to find entity");
```

## Custom Types

```rust
#[derive(SpacetimeType)]
pub enum Status { Online, Away, Offline }

#[derive(SpacetimeType)]
pub struct Point { x: f32, y: f32 }
```

## Complete Example

```rust
// src/lib.rs
use spacetimedb::{Identity, ReducerContext, SpacetimeType, Table, Timestamp};

#[spacetimedb::table(accessor = entity, public)]
pub struct Entity {
    #[primary_key]
    pub identity: Identity,
    pub name: String,
    pub active: bool,
}

#[spacetimedb::table(accessor = record, public)]
pub struct Record {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub owner: Identity,
    pub value: u32,
    pub created_at: Timestamp,
}

#[spacetimedb::reducer(client_connected)]
pub fn on_connect(ctx: &ReducerContext) {
    if let Some(existing) = ctx.db.entity().identity().find(ctx.sender()) {
        ctx.db.entity().identity().update(Entity { active: true, ..existing });
    }
}

#[spacetimedb::reducer(client_disconnected)]
pub fn on_disconnect(ctx: &ReducerContext) {
    if let Some(existing) = ctx.db.entity().identity().find(ctx.sender()) {
        ctx.db.entity().identity().update(Entity { active: false, ..existing });
    }
}

#[spacetimedb::reducer]
pub fn create_entity(ctx: &ReducerContext, name: String) {
    if ctx.db.entity().identity().find(ctx.sender()).is_some() {
        panic!("already exists");
    }
    ctx.db.entity().insert(Entity { identity: ctx.sender(), name, active: true });
}

#[spacetimedb::reducer]
pub fn add_record(ctx: &ReducerContext, value: u32) {
    if ctx.db.entity().identity().find(ctx.sender()).is_none() {
        panic!("not found");
    }
    ctx.db.record().insert(Record {
        id: 0,
        owner: ctx.sender(),
        value,
        created_at: ctx.timestamp,
    });
}
```
