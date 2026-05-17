# Doctor 诊断命令 + Gateway 启动诊断

## `openatrading doctor`

新增 CLI 子命令，诊断当前服务状态。

### 诊断项

| 检查项 | 验证方式 | 通过输出 | 失败输出 |
|--------|----------|----------|----------|
| 配置文件 | `AppConfig::load()` | `✅ config.toml` | `❌ <错误>` |
| SQLite | `Gateway::init()` | `✅ SQLite` | `❌ <错误>` |
| Cron 表达式 | 解析 schedule 字段 | `✅ cron` | `❌ schedule 无效` |
| Telegram | 发送 test message | `✅ Telegram` | `❌ 未配置 / <API错误>` |
| 角色配置 | 统计 roles 数量 | `✅ N 个角色` | — |

### 用法

```bash
openatrading doctor           # 默认 human-readable
openatrading doctor --json    # JSON 输出，适合脚本/监控
```

### 实现

在 `crates/cli/src/cli/` 新增 `doctor.rs`，注册为顶层子命令。

```rust
// app.rs — 新增顶层命令
#[derive(Subcommand)]
pub enum Commands {
    Init,
    Doctor,         // ← 新增
    Gateway { ... },
    ...
}
```

```rust
// doctor.rs
pub async fn cmd_doctor(project_root: &PathBuf, json: bool) -> Result<()>;
```

---

## 删除 `gateway test-telegram`

从 `GatewayAction` 移除 `TestTelegram` 变体，从 `gateway_cmd.rs` 删除 `cmd_gateway_test_telegram` 函数。Telegram 测试逻辑迁入 `doctor.rs`。

---

## Gateway 启动诊断

`gateway run` 启动时，在 cron/web 启动之前打印：

```
[gateway] cron: enabled, collection=<schedule>, decision=<schedule>
[gateway] telegram: ✅ connected
```

或：

```
[gateway] cron: disabled
[gateway] telegram: ⚠ not configured
```

Telegram 检查复用 `doctor.rs` 中的测试逻辑。
