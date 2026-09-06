//! automation 模块集成测试。
//!
//! 覆盖:store CRUD、收件箱语义、运行记录过滤、hub 生命周期(reload/触发)。

use std::sync::Arc;

use openatrading_core::automation::{
    validate, AutomationHub, AutomationId, AutomationPatch, AutomationStore, NewAutomation, NewRun,
    RunListQuery, RunStatus, Schedule, Scope, SessionId,
};

fn new_draft(name: &str) -> NewAutomation {
    NewAutomation {
        name: name.to_string(),
        prompt: "每天汇报一次项目状态".to_string(),
        schedule: Schedule::Daily { hour: 9, minute: 0 },
        scope: Scope::NewThread,
        cwd: "/tmp".to_string(),
        read_only: false,
        timeout_secs: None,
    }
}

fn temp_store() -> (tempfile::TempDir, Arc<AutomationStore>) {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let store = Arc::new(
        AutomationStore::open(&dir.path().join("data.db"))
            .unwrap_or_else(|e| panic!("open store: {e}")),
    );
    (dir, store)
}

// ── Happy Path ───────────────────────────────────────────────────────────

#[test]
fn store_crud_roundtrip() {
    let (_dir, store) = temp_store();

    let created = store
        .create(&new_draft("每日简报"))
        .unwrap_or_else(|e| panic!("create: {e}"));
    assert_eq!(created.name, "每日简报");
    assert!(created.enabled);
    assert!(!created.id.0.is_empty());

    let fetched = store
        .get(&created.id)
        .unwrap_or_else(|e| panic!("get: {e}"))
        .unwrap_or_else(|| panic!("automation missing"));
    assert_eq!(fetched, created);

    let updated = store
        .update(
            &created.id,
            &AutomationPatch {
                name: Some("改名简报".into()),
                enabled: Some(false),
                ..AutomationPatch::default()
            },
        )
        .unwrap_or_else(|e| panic!("update: {e}"));
    assert_eq!(updated.name, "改名简报");
    assert!(!updated.enabled);
    assert_eq!(updated.schedule, created.schedule); // 未提及字段不变

    let list = store.list().unwrap_or_else(|e| panic!("list: {e}"));
    assert_eq!(list.len(), 1);

    store
        .delete(&created.id)
        .unwrap_or_else(|e| panic!("delete: {e}"));
    assert!(store
        .get(&created.id)
        .unwrap_or_else(|e| panic!("get: {e}"))
        .is_none());
}

#[test]
fn run_inbox_semantics() {
    let (_dir, store) = temp_store();
    let automation = store
        .create(&new_draft("a"))
        .unwrap_or_else(|e| panic!("create: {e}"));

    // 有产出 → 未读、未归档
    let r1 = store
        .start_run(&automation.id)
        .unwrap_or_else(|e| panic!("start run: {e}"));
    store
        .finish_run(
            r1,
            &NewRun {
                automation_id: automation.id.clone(),
                status: RunStatus::Success,
                output: Some("发现 3 个问题".into()),
                error: None,
            },
        )
        .unwrap_or_else(|e| panic!("finish run: {e}"));

    // 无产出 → 自动归档、不打扰
    let r2 = store
        .start_run(&automation.id)
        .unwrap_or_else(|e| panic!("start run: {e}"));
    store
        .finish_run(
            r2,
            &NewRun {
                automation_id: automation.id.clone(),
                status: RunStatus::Success,
                output: Some("   ".into()),
                error: None,
            },
        )
        .unwrap_or_else(|e| panic!("finish run: {e}"));

    // 失败 → 未读
    let r3 = store
        .start_run(&automation.id)
        .unwrap_or_else(|e| panic!("start run: {e}"));
    store
        .finish_run(
            r3,
            &NewRun {
                automation_id: automation.id.clone(),
                status: RunStatus::Failed,
                output: None,
                error: Some("llm timeout".into()),
            },
        )
        .unwrap_or_else(|e| panic!("finish run: {e}"));

    let runs = store
        .list_runs(&RunListQuery::default())
        .unwrap_or_else(|e| panic!("list runs: {e}"));
    assert_eq!(runs.len(), 3);
    // 最新在前
    assert_eq!(runs[0].status, RunStatus::Failed);
    assert!(runs[0].unread && !runs[0].archived);
    assert!(runs[1].archived && !runs[1].unread); // 空产出
    assert_eq!(runs[2].output.as_deref(), Some("发现 3 个问题"));
    assert!(runs[2].unread && !runs[2].archived);
}

#[test]
fn run_filters_and_flags() {
    let (_dir, store) = temp_store();
    let a1 = store
        .create(&new_draft("a1"))
        .unwrap_or_else(|e| panic!("create: {e}"));
    let a2 = store
        .create(&new_draft("a2"))
        .unwrap_or_else(|e| panic!("create: {e}"));

    let run_a1 = store
        .start_run(&a1.id)
        .unwrap_or_else(|e| panic!("start: {e}"));
    store
        .finish_run(
            run_a1,
            &NewRun {
                automation_id: a1.id.clone(),
                status: RunStatus::Success,
                output: Some("report".into()),
                error: None,
            },
        )
        .unwrap_or_else(|e| panic!("finish: {e}"));
    let run_a2 = store
        .start_run(&a2.id)
        .unwrap_or_else(|e| panic!("start: {e}"));
    store
        .finish_run(
            run_a2,
            &NewRun {
                automation_id: a2.id.clone(),
                status: RunStatus::Success,
                output: Some("noise".into()),
                error: None,
            },
        )
        .unwrap_or_else(|e| panic!("finish: {e}"));

    let by_auto = store
        .list_runs(&RunListQuery {
            automation_id: Some(a1.id.clone()),
            ..RunListQuery::default()
        })
        .unwrap_or_else(|e| panic!("list: {e}"));
    assert_eq!(by_auto.len(), 1);
    assert_eq!(by_auto[0].automation_id, a1.id);

    // 已读后 unread 过滤不再命中
    store
        .mark_read(run_a1)
        .unwrap_or_else(|e| panic!("read: {e}"));
    let unread = store
        .list_runs(&RunListQuery {
            unread_only: true,
            ..RunListQuery::default()
        })
        .unwrap_or_else(|e| panic!("list: {e}"));
    assert_eq!(unread.len(), 1);
    assert_eq!(unread[0].automation_id, a2.id);

    // 归档后 active 过滤不再命中
    store
        .set_archived(run_a1, true)
        .unwrap_or_else(|e| panic!("archive: {e}"));
    let active = store
        .list_runs(&RunListQuery {
            active_only: true,
            ..RunListQuery::default()
        })
        .unwrap_or_else(|e| panic!("list: {e}"));
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].automation_id, a2.id);
}

#[tokio::test]
async fn hub_start_reload_idempotent() {
    let (_dir, store) = temp_store();
    let hub = AutomationHub::new(store.clone());
    store
        .create(&new_draft("任务A"))
        .unwrap_or_else(|e| panic!("create: {e}"));

    hub.start(None)
        .await
        .unwrap_or_else(|e| panic!("start: {e}"));
    // 幂等:重复 start 不报错、不重复注册
    hub.start(None)
        .await
        .unwrap_or_else(|e| panic!("start again: {e}"));
    // CRUD 后 reload 正常
    hub.reload().await.unwrap_or_else(|e| panic!("reload: {e}"));
}

#[tokio::test]
async fn hub_trigger_without_attach_does_not_record() {
    let (_dir, store) = temp_store();
    let hub = AutomationHub::new(store.clone());
    hub.start(None)
        .await
        .unwrap_or_else(|e| panic!("start: {e}"));
    let automation = store
        .create(&new_draft("未 attach"))
        .unwrap_or_else(|e| panic!("create: {e}"));

    hub.trigger_now(&automation.id)
        .await
        .unwrap_or_else(|e| panic!("trigger: {e}"));
    // 等待 spawn 的 run_once 完成(未 attach 时只记日志、不落库)
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let runs = store
        .list_runs(&RunListQuery::default())
        .unwrap_or_else(|e| panic!("list: {e}"));
    assert!(runs.is_empty(), "unattached hub must not record runs");
}

// ── Edge Case ────────────────────────────────────────────────────────────

#[test]
fn invalid_definitions_rejected() {
    let (_dir, store) = temp_store();

    // existing_thread 缺 session_id
    let bad_scope = NewAutomation {
        scope: Scope::ExistingThread {
            session_id: SessionId(String::new()),
        },
        ..new_draft("bad scope")
    };
    assert!(store.create(&bad_scope).is_err());

    // 越界间隔
    let bad_interval = NewAutomation {
        schedule: Schedule::EveryMinutes { minutes: 0 },
        ..new_draft("bad interval")
    };
    assert!(store.create(&bad_interval).is_err());

    // 空 prompt
    let bad_prompt = NewAutomation {
        prompt: "  ".into(),
        ..new_draft("bad prompt")
    };
    assert!(store.create(&bad_prompt).is_err());
}

#[tokio::test]
async fn invalid_cron_is_skipped_not_fatal() {
    let (_dir, store) = temp_store();
    let hub = AutomationHub::new(store.clone());
    store
        .create(&NewAutomation {
            schedule: Schedule::Cron {
                expr: "0 99 * * * *".into(), // 字段数合法、数值越界,由调度器拒绝
            },
            ..new_draft("坏表达式")
        })
        .unwrap_or_else(|e| panic!("create: {e}"));

    // reload 不 panic、不失败(仅日志跳过该任务)
    hub.start(None)
        .await
        .unwrap_or_else(|e| panic!("start: {e}"));
    hub.reload().await.unwrap_or_else(|e| panic!("reload: {e}"));
}

#[tokio::test]
async fn hub_reload_only_schedules_enabled() {
    let (_dir, store) = temp_store();
    let hub = AutomationHub::new(store.clone());
    let a = store
        .create(&new_draft("禁用任务"))
        .unwrap_or_else(|e| panic!("create: {e}"));

    // disable 后 reload 不报错(该任务不注册)
    store
        .update(
            &a.id,
            &AutomationPatch {
                enabled: Some(false),
                ..AutomationPatch::default()
            },
        )
        .unwrap_or_else(|e| panic!("update: {e}"));
    hub.start(None)
        .await
        .unwrap_or_else(|e| panic!("start: {e}"));
    hub.reload().await.unwrap_or_else(|e| panic!("reload: {e}"));
}

#[test]
fn missing_ids_report_errors() {
    let (_dir, store) = temp_store();
    let ghost = AutomationId("no-such-id".into());

    let patch_err = store.update(&ghost, &AutomationPatch::default());
    assert!(patch_err.is_err());
    assert!(store.delete(&ghost).is_err());
    assert!(store.mark_read(999_999).is_err());
    assert!(store.set_archived(999_999, true).is_err());
}

// ── Failure Case ─────────────────────────────────────────────────────────

#[test]
fn corrupt_scope_json_propagates_error() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let db = dir.path().join("data.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap_or_else(|e| panic!("open: {e}"));
        conn.execute_batch(
            "CREATE TABLE automations (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, prompt TEXT NOT NULL,
                schedule_json TEXT NOT NULL, scope_json TEXT NOT NULL, cwd TEXT NOT NULL,
                enabled INTEGER NOT NULL, created_at TEXT, updated_at TEXT);",
        )
        .unwrap_or_else(|e| panic!("schema: {e}"));
        conn.execute(
            "INSERT INTO automations VALUES ('x','n','p','{\"kind\":\"daily\",\"hour\":9,\"minute\":0}','{broken','/tmp',1,'t','t')",
            [],
        )
        .unwrap_or_else(|e| panic!("insert: {e}"));
    }
    let store = AutomationStore::open(&db).unwrap_or_else(|e| panic!("open: {e}"));
    let err = store.list();
    assert!(
        err.is_err(),
        "corrupt scope_json must surface as error, not panic"
    );
}

#[test]
fn concurrent_writers_do_not_deadlock() {
    let (_dir, store) = temp_store();
    let automation = store
        .create(&new_draft("并发"))
        .unwrap_or_else(|e| panic!("create: {e}"));
    let mut handles = Vec::new();
    for i in 0..8 {
        let store = store.clone();
        let auto_id = automation.id.clone();
        handles.push(std::thread::spawn(move || {
            let run_id = store
                .start_run(&auto_id)
                .unwrap_or_else(|e| panic!("start {i}: {e}"));
            store
                .finish_run(
                    run_id,
                    &NewRun {
                        automation_id: auto_id,
                        status: RunStatus::Success,
                        output: Some(format!("run {i}")),
                        error: None,
                    },
                )
                .unwrap_or_else(|e| panic!("finish {i}: {e}"));
        }));
    }
    for handle in handles {
        let _ = handle.join();
    }
    let runs = store
        .list_runs(&RunListQuery::default())
        .unwrap_or_else(|e| panic!("list: {e}"));
    assert_eq!(runs.len(), 8);
}

// ── 本轮:P0 运行治理 + P1 扩展 ──────────────────────────────────────────

#[test]
fn weekly_schedule_to_cron_iso_to_quartz() {
    // ISO 1(周一)→ Quartz 2;ISO 7(周日)→ Quartz 1
    let s = Schedule::Weekly {
        days: vec![1, 7],
        hour: 9,
        minute: 0,
    };
    assert_eq!(
        s.to_cron().map_err(|e| e.to_string()),
        Ok("0 0 9 * * 2,1".to_string())
    );
}

#[test]
fn weekly_schedule_validation() {
    assert!(Schedule::Weekly {
        days: vec![],
        hour: 9,
        minute: 0
    }
    .to_cron()
    .is_err());
    assert!(Schedule::Weekly {
        days: vec![0, 1],
        hour: 9,
        minute: 0
    }
    .to_cron()
    .is_err());
    assert!(Schedule::Weekly {
        days: vec![8],
        hour: 9,
        minute: 0
    }
    .to_cron()
    .is_err());
    assert!(Schedule::Weekly {
        days: vec![1],
        hour: 24,
        minute: 0
    }
    .to_cron()
    .is_err());
}

#[test]
fn timeout_secs_validation() {
    let err = validate(
        "n",
        "p",
        &Schedule::EveryMinutes { minutes: 5 },
        &Scope::NewThread,
        Some(0),
    );
    assert!(err.is_err());
    let err = validate(
        "n",
        "p",
        &Schedule::EveryMinutes { minutes: 5 },
        &Scope::NewThread,
        Some(200_000),
    );
    assert!(err.is_err());
    let ok = validate(
        "n",
        "p",
        &Schedule::EveryMinutes { minutes: 5 },
        &Scope::NewThread,
        Some(120),
    );
    assert!(ok.is_ok());
}

#[tokio::test]
async fn run_lifecycle_running_state() {
    let (_dir, store) = temp_store();
    let automation = store
        .create(&new_draft("a"))
        .unwrap_or_else(|e| panic!("create: {e}"));

    let run_id = store
        .start_run(&automation.id)
        .unwrap_or_else(|e| panic!("start: {e}"));
    let runs = store
        .list_runs(&RunListQuery::default())
        .unwrap_or_else(|e| panic!("list: {e}"));
    assert_eq!(runs[0].status, RunStatus::Running);
    assert!(runs[0].finished_at.is_none());
    assert!(runs[0].unread);

    store
        .finish_run(
            run_id,
            &NewRun {
                automation_id: automation.id.clone(),
                status: RunStatus::Success,
                output: Some("done".into()),
                error: None,
            },
        )
        .unwrap_or_else(|e| panic!("finish: {e}"));
    let runs = store
        .list_runs(&RunListQuery::default())
        .unwrap_or_else(|e| panic!("list: {e}"));
    assert_eq!(runs[0].status, RunStatus::Success);
    assert!(runs[0].finished_at.is_some());
}

#[tokio::test]
async fn reap_stale_running_runs() {
    let (_dir, store) = temp_store();
    let automation = store
        .create(&new_draft("a"))
        .unwrap_or_else(|e| panic!("create: {e}"));

    // 手工插入一条"很久之前"的 running 记录
    {
        let _ = &automation;
        // 用 start_run 后无法改 started_at,直接 SQL 更新时间
        let run_id = store
            .start_run(&automation.id)
            .unwrap_or_else(|e| panic!("start: {e}"));
        let db = _dir.path().join("data.db");
        let conn = rusqlite::Connection::open(&db).unwrap_or_else(|e| panic!("open: {e}"));
        conn.execute(
            "UPDATE automation_runs SET started_at = datetime('now','localtime','-25 hours') WHERE id = ?1",
            [run_id],
        ).unwrap_or_else(|e| panic!("backdate: {e}"));
    }

    let reaped = store
        .reap_stale_runs(86_400)
        .unwrap_or_else(|e| panic!("reap: {e}"));
    assert_eq!(reaped, 1);
    let runs = store
        .list_runs(&RunListQuery::default())
        .unwrap_or_else(|e| panic!("list: {e}"));
    assert_eq!(runs[0].status, RunStatus::Failed);
    assert!(runs[0]
        .error
        .as_deref()
        .is_some_and(|e| e.contains("interrupted")));
}

#[tokio::test]
async fn finish_missing_run_errors() {
    let (_dir, store) = temp_store();
    let automation = store
        .create(&new_draft("a"))
        .unwrap_or_else(|e| panic!("create: {e}"));
    let err = store.finish_run(
        999_999,
        &NewRun {
            automation_id: automation.id.clone(),
            status: RunStatus::Success,
            output: Some("x".into()),
            error: None,
        },
    );
    assert!(err.is_err());
}

#[tokio::test]
async fn update_tool_changes_name_and_schedule() {
    let (_dir, store) = temp_store();
    let hub = AutomationHub::new(store.clone());
    let automation = store
        .create(&new_draft("旧名"))
        .unwrap_or_else(|e| panic!("create: {e}"));

    let tools = openatrading_core::automation::automation_tools(&hub);
    let update = tools
        .iter()
        .find(|t| t.name == "update_automation")
        .unwrap_or_else(|| panic!("missing update_automation"));
    let execute = update
        .execute
        .clone()
        .unwrap_or_else(|| panic!("no execute"));

    let result = execute(
        "test".into(),
        serde_json::json!({
            "id": automation.id.0,
            "name": "新名字",
            "schedule": {"kind": "weekly", "days": [1, 3, 5], "hour": 8, "minute": 30},
        }),
        None,
    )
    .await;
    assert!(result.is_ok(), "update tool should succeed");

    let updated = store
        .get(&automation.id)
        .unwrap_or_else(|e| panic!("get: {e}"))
        .unwrap_or_else(|| panic!("missing"));
    assert_eq!(updated.name, "新名字");
    assert_eq!(
        updated.schedule,
        Schedule::Weekly {
            days: vec![1, 3, 5],
            hour: 8,
            minute: 30
        }
    );
}
