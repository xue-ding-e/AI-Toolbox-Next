use super::*;

fn insert_websocket_proxy(db: &SqliteDbState, kind: &str, outcome: &str) {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO proxy_request_logs (request_id,provider_id,app_type,model,request_model,input_tokens,output_tokens,cache_read_tokens,created_at,status_code,stream_outcome,transport,request_kind,usage_request_count,duration_ms)
             VALUES ('SESSION:codex:provider:response-ws','provider','codex','test-model','test-model',10,4,10,?1,0,?2,'websocket',?3,?4,2000)",
            params![THEN, outcome, kind, i64::from(kind == "request")],
        ).map_err(|error| error.to_string())?;
        Ok(())
    }).unwrap();
}

fn write_native_session(root: &Path) {
    write_jsonl(
        &root.join(format!("rollout-{PARENT}.jsonl")),
        &[
            json!({"type":"session_meta","timestamp":THEN - 10,"payload":{"id":PARENT}}),
            json!({"type":"turn_context","payload":{"model":"test-model"}}),
            token_event(100, 20, 4, THEN),
        ],
    );
}

#[test]
fn websocket_usage_deduplicates_native_codex_in_both_arrival_orders() {
    for native_first in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let db = SqliteDbState::in_memory_for_test().unwrap();
        write_native_session(root.path());
        if native_first {
            assert_eq!(
                run_sync(&db, GatewayCliKey::Codex, root.path()).inserted_records,
                1
            );
        }
        insert_websocket_proxy(&db, "request", "completed");
        let imported = run_sync(&db, GatewayCliKey::Codex, root.path());
        assert_eq!(imported.failed_files, 0);
        if !native_first {
            assert_eq!(imported.parsed_records, 1);
        }
        assert_eq!(
            run_sync(&db, GatewayCliKey::Codex, root.path()).inserted_records,
            0
        );
        let logs =
            usage_stats::request_logs(&db, &GatewayRequestLogFilters::default(), 0, 10, true)
                .unwrap();
        assert_eq!(logs.total, 1, "native_first={native_first}");
        assert_eq!(logs.data[0].data_source, "proxy");
        assert_eq!(count(&db), 1);
        assert_eq!(
            usage_stats::usage_summary(&db, None, None, None, true)
                .unwrap()
                .total_tokens,
            24
        );
    }
}

#[test]
fn websocket_warmup_and_incomplete_turns_do_not_heuristically_consume_native_calls() {
    for (kind, outcome) in [("websocket_warmup", "completed"), ("request", "incomplete")] {
        let root = tempfile::tempdir().unwrap();
        let db = SqliteDbState::in_memory_for_test().unwrap();
        write_native_session(root.path());
        insert_websocket_proxy(&db, kind, outcome);
        assert_eq!(
            run_sync(&db, GatewayCliKey::Codex, root.path()).inserted_records,
            1
        );
        let logs =
            usage_stats::request_logs(&db, &GatewayRequestLogFilters::default(), 0, 10, true)
                .unwrap();
        assert_eq!(logs.total, 2);
        assert!(logs.data.iter().any(|entry| entry.data_source == "session"));
    }
}
