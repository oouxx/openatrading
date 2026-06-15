# 函数参数重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor 6 Rust functions with >5 parameters into domain-grouped struct parameters, satisfying the ≤3 param constraint.

**Architecture:** Introduce 4 new structs (`CronContext`, `PositionSnapshot`, `McpToolSpec`, `CommunityQuery`) and reuse 1 existing struct (`ChatArgs`). Each struct groups parameters by business semantics. No behavior changes — only signatures and call sites.

**Tech Stack:** Rust, existing codebase types

---

### Task 1: CronContext — Define struct and refactor `run_decision`

**Files:**
- Modify: `crates/core/src/cron/mod.rs`

- [ ] **Step 1: Add `CronContext` struct**

Add before `start_scheduler`:

```rust
/// Shared infrastructure for cron tasks (decision, collection, news).
pub struct CronContext {
    pub memory: Arc<dyn MemoryStore>,
    pub trade: Arc<dyn TradeManager>,
    pub social: Arc<dyn NotificationSender>,
    pub llm: LlmClient,
    pub api: ApiClient,
}
```

- [ ] **Step 2: Refactor `run_decision` signature**

Change from:
```rust
pub async fn run_decision(
    memory: Arc<dyn MemoryStore>,
    trade: Arc<dyn TradeManager>,
    social: &dyn NotificationSender,
    llm: &LlmClient,
    api: &ApiClient,
    roles: &[RoleEntry],
    tools: &[Arc<DynTool>],
) -> Result<()> {
```

To:
```rust
pub async fn run_decision(
    ctx: &CronContext,
    roles: &[RoleEntry],
    tools: &[Arc<DynTool>],
) -> Result<()> {
```

Then update the body to use `ctx.` prefix:
- `api.xuangubao.fetch_market_overview()` → `ctx.api.xuangubao.fetch_market_overview()`
- `update_all_position_prices(&*trade, api)` → `update_all_position_prices(&*ctx.trade, &ctx.api)`
- `run_decision_for_role(&*trade, social, llm, api, role, &market_context, &role_tools)` → `run_decision_for_role(ctx, role, &market_context, &role_tools)`
- `trade.clone()` → `ctx.trade.clone()`
- `memory.clone()` → `ctx.memory.clone()`

- [ ] **Step 3: Refactor `run_decision_for_role` signature**

Change from:
```rust
async fn run_decision_for_role(
    trade: &dyn TradeManager,
    social: &dyn NotificationSender,
    llm: &LlmClient,
    api: &ApiClient,
    role: &RoleEntry,
    market_context: &str,
    tools: &[Arc<DynTool>],
) -> Result<()> {
```

To:
```rust
async fn run_decision_for_role(
    ctx: &CronContext,
    role: &RoleEntry,
    market_context: &str,
    tools: &[Arc<DynTool>],
) -> Result<()> {
```

Then update the body to use `ctx.` prefix for all `trade`, `social`, `llm`, `api` references:
- `social.notify_decision(...)` → `ctx.social.notify_decision(...)`
- `social.notify_trade(...)` → `ctx.social.notify_trade(...)`
- `crate::llm::session::AgentSession::new(llm, &system)` → `crate::llm::session::AgentSession::new(&ctx.llm, &system)`
- `execute_buy(trade, api, &role.name, instr)` → `execute_buy(&*ctx.trade, &ctx.api, &role.name, instr)`
- `execute_sell(trade, api, &role.name, instr)` → `execute_sell(&*ctx.trade, &ctx.api, &role.name, instr)`

- [ ] **Step 4: Update `start_scheduler` decision job to construct `CronContext`**

In the Decision section of `start_scheduler`, change from cloning 6 variables separately to:
```rust
let cron_ctx = Arc::new(CronContext {
    memory: ctx.memory.clone(),
    trade: ctx.trade.clone(),
    social: ctx.social.clone(),
    llm: ctx.llm.clone(),
    api: ctx.api.clone(),
});

add_job!(
    scheduler,
    ctx.config.decision_schedule,
    "decision",
    move |_uuid, _lock| {
        let cron_ctx = cron_ctx.clone();
        let roles = roles.clone();
        let decision_tools = all_tools.clone();
        Box::pin(async move {
            if let Err(e) = run_decision(&cron_ctx, &roles, &decision_tools).await
            {
                tracing::error!("[cron] decision error: {e}");
            }
        })
    }
);
```

Remove the now-unnecessary individual clones (`mem`, `trd`, `soc`, `llm`, `api`).

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p openatrading-core`
Expected: Compile errors in `telegram_bot.rs` and `cli/mod.rs` (callers not yet updated — that's expected). The `cron/mod.rs` itself should compile.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/cron/mod.rs
git commit -m "refactor(cron): introduce CronContext for run_decision"
```

---

### Task 2: CronContext — Update callers (telegram_bot.rs, cli/mod.rs)

**Files:**
- Modify: `crates/core/src/social/telegram_bot.rs`
- Modify: `crates/cli/src/cli/mod.rs`

- [ ] **Step 1: Update `telegram_bot.rs`**

Change the import from:
```rust
use crate::cron::run_decision;
```
To:
```rust
use crate::cron::{run_decision, CronContext};
```

Change the call site (around line 455) from:
```rust
match run_decision(
    self.memory.clone(),
    self.trade.clone(),
    &notifier,
    &self.llm,
    &self.api,
    &filtered_roles,
    &tools,
)
.await
```
To:
```rust
let ctx = CronContext {
    memory: self.memory.clone(),
    trade: self.trade.clone(),
    social: Arc::new(notifier),
    llm: self.llm.clone(),
    api: self.api.clone(),
};
match run_decision(&ctx, &filtered_roles, &tools).await
```

Note: `DecisionNotifier` is wrapped in `Arc::new()` because `CronContext.social` is `Arc<dyn NotificationSender>`. `DecisionNotifier` implements `NotificationSender` (which requires `Send + Sync`), so this works.

- [ ] **Step 2: Update `cli/mod.rs`**

Change the import from:
```rust
use openatrading_core::cron::run_decision;
```
To:
```rust
use openatrading_core::cron::{run_decision, CronContext};
```

Change the call site (around line 279) from:
```rust
run_decision(
    gateway.memory.clone(),
    gateway.trade.clone(),
    &*social,
    &llm,
    &api,
    &filtered_roles,
    &tools,
)
.await?;
```
To:
```rust
let ctx = CronContext {
    memory: gateway.memory.clone(),
    trade: gateway.trade.clone(),
    social,
    llm,
    api,
};
run_decision(&ctx, &filtered_roles, &tools).await?;
```

Note: `social` is already `Arc<dyn NotificationSender>`, no conversion needed.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p openatrading-core -p openatrading-cli`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/social/telegram_bot.rs crates/cli/src/cli/mod.rs
git commit -m "refactor: update run_decision callers to use CronContext"
```

---

### Task 3: PositionSnapshot — Refactor `validate_buy`

**Files:**
- Modify: `crates/core/src/trade/paper.rs`
- Modify: `crates/core/src/trade/broker.rs`

- [ ] **Step 1: Add `PositionSnapshot` struct and update `validate_buy` signature**

In `paper.rs`, add before `impl PaperBroker`:

```rust
/// Current position state used for buy validation.
pub struct PositionSnapshot {
    pub same_symbol_value: f64,
    pub total_position_value: f64,
}
```

Change `validate_buy` from:
```rust
pub fn validate_buy(
    &self,
    shares: i64,
    exec_price: f64,
    same_symbol_value: f64,
    total_position_value: f64,
) -> Result<(), String> {
```

To:
```rust
pub fn validate_buy(
    &self,
    shares: i64,
    exec_price: f64,
    positions: &PositionSnapshot,
) -> Result<(), String> {
```

Update the body — replace `same_symbol_value` with `positions.same_symbol_value` and `total_position_value` with `positions.total_position_value`.

- [ ] **Step 2: Update tests in `paper.rs`**

Change:
```rust
assert!(broker.validate_buy(150, 10.0, 0.0, 0.0).is_err());
```
To:
```rust
assert!(broker.validate_buy(150, 10.0, &PositionSnapshot { same_symbol_value: 0.0, total_position_value: 0.0 }).is_err());
```

Change:
```rust
assert!(broker.validate_buy(100, 10.0, 0.0, 0.0).is_ok());
```
To:
```rust
assert!(broker.validate_buy(100, 10.0, &PositionSnapshot { same_symbol_value: 0.0, total_position_value: 0.0 }).is_ok());
```

Add the import in the test module:
```rust
use super::PositionSnapshot;
```

- [ ] **Step 3: Update `broker.rs`**

Change:
```rust
broker
    .validate_buy(shares, exec_price, same_symbol_value, total_position_value)
    .map_err(|e| anyhow::anyhow!("{e}"))?;
```

To:
```rust
broker
    .validate_buy(
        shares,
        exec_price,
        &crate::trade::paper::PositionSnapshot {
            same_symbol_value,
            total_position_value,
        },
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
```

- [ ] **Step 4: Verify compilation and tests**

Run: `cargo check -p openatrading-core && cargo test -p openatrading-core -- paper`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/trade/paper.rs crates/core/src/trade/broker.rs
git commit -m "refactor(trade): introduce PositionSnapshot for validate_buy"
```

---

### Task 4: McpToolSpec — Refactor `create_tool`

**Files:**
- Modify: `crates/core/src/llm/mcp_tools.rs`

- [ ] **Step 1: Add `McpToolSpec` struct and update `create_tool`**

Add before `impl McpClient`:

```rust
/// Specification for creating an MCP-backed DynTool.
pub struct McpToolSpec {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
    pub method: String,
}
```

Change `create_tool` from:
```rust
pub fn create_tool(
    &self,
    name: &str,
    description: &str,
    schema: serde_json::Value,
    method: &str,
) -> Arc<DynTool> {
    let peer = self.peer.clone();
    let method = method.to_string();
    let execute: super::tools::ExecuteFn = Arc::new(move |_id, args, _signal, _progress| {
```

To:
```rust
pub fn create_tool(&self, spec: McpToolSpec) -> Arc<DynTool> {
    let peer = self.peer.clone();
    let McpToolSpec { name, description, schema, method } = spec;
    let execute: super::tools::ExecuteFn = Arc::new(move |_id, args, _signal, _progress| {
```

Also update `super::tools::tool(name, description, schema, execute)` — note these are now owned `String` values, so change to `super::tools::tool(&name, &description, schema, execute)` if `tool()` takes `&str`, or keep as-is if it takes owned values. Check the signature of `super::tools::tool` and adjust accordingly.

- [ ] **Step 2: Update call site in `discover` method**

Change:
```rust
if schema.is_null() {
    all_tools.push(client.create_tool(
        name,
        desc,
        empty_schema(),
        name,
    ));
} else {
    all_tools.push(client.create_tool(name, desc, schema, name));
}
```

To:
```rust
all_tools.push(client.create_tool(McpToolSpec {
    name: name.to_string(),
    description: desc.to_string(),
    schema: if schema.is_null() { empty_schema() } else { schema },
    method: name.to_string(),
}));
```

This eliminates the branching — the null check is folded into the schema field.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p openatrading-core`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/llm/mcp_tools.rs
git commit -m "refactor(llm): introduce McpToolSpec for create_tool"
```

---

### Task 5: ChatArgs — Refactor `run_llm`

**Files:**
- Modify: `crates/tui/src/chat.rs`

- [ ] **Step 1: Refactor `run_llm` signature**

Change from:
```rust
async fn run_llm(
    msg_tx: tokio::sync::mpsc::Sender<Msg>,
    llm: LlmClient,
    tools: Vec<Arc<DynTool>>,
    system_prompt: String,
    llm_messages: Vec<serde_json::Value>,
) {
    use std::pin::Pin;

    let agent = Arc::new(create_agent(&llm, tools, &system_prompt));
```

To:
```rust
async fn run_llm(
    msg_tx: tokio::sync::mpsc::Sender<Msg>,
    args: ChatArgs,
    llm_messages: Vec<serde_json::Value>,
) {
    use std::pin::Pin;

    let agent = Arc::new(create_agent(&args.llm, args.tools, &args.system_prompt));
```

- [ ] **Step 2: Update call site**

Change:
```rust
let llm = state.llm.clone();
let tools = state.tools.clone();
let system_prompt = state.system_prompt.clone();
let llm_messages = collect_llm_messages(&state.conv);

let handle = state.conv.push_assistant("");
state.streaming_handle = Some(handle);

tokio::spawn(async move {
    run_llm(tx, llm, tools, system_prompt, llm_messages).await;
});
```

To:
```rust
let llm = state.llm.clone();
let tools = state.tools.clone();
let system_prompt = state.system_prompt.clone();
let llm_messages = collect_llm_messages(&state.conv);

let handle = state.conv.push_assistant("");
state.streaming_handle = Some(handle);

tokio::spawn(async move {
    run_llm(tx, ChatArgs { llm, tools, system_prompt }, llm_messages).await;
});
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p openatrading-tui`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/tui/src/chat.rs
git commit -m "refactor(tui): reuse ChatArgs for run_llm parameters"
```

---

### Task 6: CommunityQuery — Refactor `fetch_community`

**Files:**
- Modify: `crates/api/src/jiuyangongshe.rs`
- Modify: `crates/core/src/agents/sentiment.rs`

- [ ] **Step 1: Add `CommunityQuery` struct and update `fetch_community`**

In `jiuyangongshe.rs`, add before `impl JiuYanSession` (or in the same area as other request types):

```rust
/// Query parameters for the community article API.
pub struct CommunityQuery {
    pub category_id: String,
    pub article_type: i32,
    /// 1-based page number (passed directly as `start` to the API).
    pub page: i32,
    pub page_size: i32,
}
```

Change `fetch_community` from:
```rust
pub async fn fetch_community(
    &self,
    category_id: &str,
    article_type: i32,
    page_no: i32,
    page_size: i32,
) -> Result<CommunityData> {
    ...
    let body = CommunityReq {
        category_id: category_id.to_string(),
        r#type: article_type,
        limit: page_size,
        order: 1,
        start: page_no,
        back_garden: 0,
    };
```

To:
```rust
pub async fn fetch_community(&self, query: CommunityQuery) -> Result<CommunityData> {
    ...
    let body = CommunityReq {
        category_id: query.category_id,
        r#type: query.article_type,
        limit: query.page_size,
        order: 1,
        start: query.page,
        back_garden: 0,
    };
```

Note: `query.category_id` is already `String`, no `.to_string()` needed. `query.page` maps directly to `start` (the API uses the value as-is, not as a page index).

- [ ] **Step 2: Update call site in `sentiment.rs`**

Change:
```rust
match client.jiuyan.fetch_community("", 0, page, 30).await {
```

To:
```rust
match client.jiuyan.fetch_community(openatrading_api::jiuyangongshe::CommunityQuery {
    category_id: String::new(),
    article_type: 0,
    page,
    page_size: 30,
}).await
{
```

Note: Check if `CommunityQuery` needs to be re-exported or if the full path is needed. If `jiuyangongshe` is a private module, add a `pub use` in `crates/api/src/lib.rs`.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p openatrading-core -p openatrading-api`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/api/src/jiuyangongshe.rs crates/core/src/agents/sentiment.rs
git commit -m "refactor(api): introduce CommunityQuery for fetch_community"
```

---

### Task 7: Final verification

- [ ] **Step 1: Full workspace compilation check**

Run: `cargo check --workspace`
Expected: PASS

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 3: Verify no function has >5 parameters**

Run:
```bash
python3 -c "
import re, os
result = os.popen('find . -name \"*.rs\" -not -path \"*/target/*\" -print0').read()
files = [f for f in result.strip().split('\\0') if f]
fn_start = re.compile(r'^\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*(?:<[^>]*>)?\s*\(')
violations = []
for fpath in files:
    if not fpath or not os.path.isfile(fpath): continue
    with open(fpath) as f: lines = f.readlines()
    i = 0
    while i < len(lines):
        m = fn_start.match(lines[i])
        if m:
            fn_name = m.group(1)
            sig = lines[i]
            depth = sig.count('(') - sig.count(')')
            j = i + 1
            while depth > 0 and j < len(lines):
                sig += lines[j]; depth += lines[j].count('(') - lines[j].count(')'); j += 1
            ps = sig.index('(')
            d = 0; pe = ps
            for k in range(ps, len(sig)):
                if sig[k] == '(': d += 1
                elif sig[k] == ')':
                    d -= 1
                    if d == 0: pe = k; break
            params = sig[ps+1:pe].strip()
            if not params: cnt = 0
            else:
                d = 0; c = 0
                for ch in params:
                    if ch in '<(': d += 1
                    elif ch in '>)': d -= 1
                    elif ch == ',' and d == 0: c += 1
                cnt = c + 1
            if cnt > 5: violations.append((cnt, os.path.relpath(fpath), i+1, fn_name))
            i = j
        else: i += 1
if violations:
    for c, p, l, n in sorted(violations, key=lambda x: -x[0]):
        print(f'  {c} params | {p}:{l} | {n}')
else:
    print('All functions have <=5 parameters!')
"
```

Expected: "All functions have <=5 parameters!"

- [ ] **Step 4: Final commit (if any formatting fixes needed)**

```bash
cargo fmt
git add -A
git commit -m "chore: format after param refactor"  # only if changes
```
