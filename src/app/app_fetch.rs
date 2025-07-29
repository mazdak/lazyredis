use super::{App, StreamEntry};
use redis::{aio::MultiplexedConnection, Value};
use std::future::Future;

impl App {
    async fn run_fetch<T, Fut, OkF, ErrF>(
        &mut self,
        fut: Fut,
        on_ok: OkF,
        on_err: ErrF,
        err_msg: String,
    ) where
        Fut: Future<Output = redis::RedisResult<T>>,
        OkF: FnOnce(&mut Self, T),
        ErrF: FnOnce(&mut Self),
    {
        match fut.await {
            Ok(val) => {
                on_ok(self, val);
                self.value_viewer.selected_key_value = None;
            }
            Err(e) => {
                self.value_viewer.selected_key_value = Some(format!("{}: {}", err_msg, e));
                on_err(self);
            }
        }
    }

    pub async fn fetch_and_set_hash_value(
        &mut self,
        key_name: &str,
        con: &mut MultiplexedConnection,
    ) {
        let mut owned_cmd = redis::cmd("HGETALL");
        owned_cmd.arg(key_name);
        let fut = owned_cmd.query_async::<Vec<String>>(con);
        let err_context = format!("Failed to HGETALL for '{}' (hash)", key_name);
        self.run_fetch(
            fut,
            |app, pairs| {
                if pairs.is_empty() {
                    app.value_viewer.selected_key_value_hash = Some(Vec::new());
                } else {
                    let mut hash_data: Vec<(String, String)> = Vec::new();
                    for chunk in pairs.chunks(2) {
                        if chunk.len() == 2 {
                            hash_data.push((chunk[0].clone(), chunk[1].clone()));
                        } else {
                            app.value_viewer.selected_key_value = Some(format!(
                                "HGETALL for '{}' (hash) returned malformed pair data.",
                                key_name
                            ));
                            app.value_viewer.selected_key_value_hash = None;
                            return;
                        }
                    }
                    app.value_viewer.selected_key_value_hash = Some(hash_data);
                }
            },
            |app| {
                app.value_viewer.selected_key_value_hash = None;
            },
            err_context,
        )
        .await;
    }

    pub async fn fetch_and_set_zset_value(
        &mut self,
        key_name: &str,
        con: &mut MultiplexedConnection,
    ) {
        let mut owned_cmd = redis::cmd("ZRANGE");
        owned_cmd.arg(key_name);
        owned_cmd.arg(0);
        owned_cmd.arg(-1);
        owned_cmd.arg("WITHSCORES");
        let fut = owned_cmd.query_async::<Vec<String>>(con);
        let err_context = format!("Failed to ZRANGE for '{}' (zset)", key_name);
        self.run_fetch(
            fut,
            |app, pairs| {
                if pairs.is_empty() {
                    app.value_viewer.selected_key_value_zset = Some(Vec::new());
                } else {
                    let mut zset_data: Vec<(String, f64)> = Vec::new();
                    for chunk in pairs.chunks(2) {
                        if chunk.len() == 2 {
                            let member = chunk[0].clone();
                            match chunk[1].parse::<f64>() {
                                Ok(score) => zset_data.push((member, score)),
                                Err(_) => {
                                    app.value_viewer.selected_key_value = Some(format!(
                                        "ZRANGE for '{}' (zset) failed to parse score for member '{}'.",
                                        key_name,
                                        member
                                    ));
                                    app.value_viewer.selected_key_value_zset = None;
                                    return;
                                }
                            }
                        } else {
                            app.value_viewer.selected_key_value = Some(format!(
                                "ZRANGE for '{}' (zset) returned malformed pair data.",
                                key_name
                            ));
                            app.value_viewer.selected_key_value_zset = None;
                            return;
                        }
                    }
                    app.value_viewer.selected_key_value_zset = Some(zset_data);
                }
            },
            |app| {
                app.value_viewer.selected_key_value_zset = None;
            },
            err_context,
        )
        .await;
    }

    pub async fn fetch_and_set_list_value(
        &mut self,
        key_name: &str,
        con: &mut MultiplexedConnection,
    ) {
        let mut owned_cmd = redis::cmd("LRANGE");
        owned_cmd.arg(key_name);
        owned_cmd.arg(0);
        owned_cmd.arg(-1);
        let fut = owned_cmd.query_async::<Vec<String>>(con);
        let err_context = format!("Failed to LRANGE for '{}' (list)", key_name);
        self.run_fetch(
            fut,
            |app, elements| {
                app.value_viewer.selected_key_value_list = Some(elements);
            },
            |app| {
                app.value_viewer.selected_key_value_list = None;
            },
            err_context,
        )
        .await;
    }

    pub async fn fetch_and_set_set_value(
        &mut self,
        key_name: &str,
        con: &mut MultiplexedConnection,
    ) {
        let mut owned_cmd = redis::cmd("SMEMBERS");
        owned_cmd.arg(key_name);
        let fut = owned_cmd.query_async::<Vec<String>>(con);
        let err_context = format!("Failed to SMEMBERS for '{}' (set)", key_name);
        self.run_fetch(
            fut,
            |app, members| {
                app.value_viewer.selected_key_value_set = Some(members);
            },
            |app| {
                app.value_viewer.selected_key_value_set = None;
            },
            err_context,
        )
        .await;
    }

    pub async fn fetch_and_set_json_value(
        &mut self,
        key_name: &str,
        con: &mut MultiplexedConnection,
    ) {
        let mut owned_cmd = redis::cmd("JSON.GET");
        owned_cmd.arg(key_name);
        let fut = owned_cmd.query_async::<String>(con);
        let err_context = format!("Failed to JSON.GET for '{}' (json)", key_name);
        self.run_fetch(
            fut,
            |app, value| {
                app.value_viewer.selected_key_value_json = Some(value);
            },
            |app| {
                app.value_viewer.selected_key_value_json = None;
            },
            err_context,
        )
        .await;
    }

    pub async fn fetch_and_set_stream_value(
        &mut self,
        key_name: &str,
        con: &mut MultiplexedConnection,
    ) {
        const GROUP_NAME: &str = "lazyredis_group";
        const CONSUMER_NAME: &str = "lazyredis_consumer";

        let _ = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(key_name)
            .arg(GROUP_NAME)
            .arg("$")
            .arg("MKSTREAM")
            .query_async::<()>(con)
            .await;

        match redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(GROUP_NAME)
            .arg(CONSUMER_NAME)
            .arg("COUNT")
            .arg(100)
            .arg("STREAMS")
            .arg(key_name)
            .arg(">")
            .query_async::<Value>(con)
            .await
        {
            Ok(Value::Nil) => {
                self.value_viewer.selected_key_value_stream = Some(Vec::new());
                self.value_viewer.selected_key_value = None;
                self.value_viewer.current_display_value =
                    Some("(empty stream or no new messages)".to_string());
                self.value_viewer.displayed_value_lines = None;
            }
            Ok(Value::Array(stream_data)) => {
                let mut parsed_streams: Vec<StreamEntry> = Vec::new();
                for single_stream_result in stream_data {
                    if let Value::Array(stream_specific_data) = single_stream_result {
                        if stream_specific_data.len() == 2 {
                            if let Value::Array(messages) = &stream_specific_data[1] {
                                for message_val in messages {
                                    if let Value::Array(message_parts) = message_val {
                                        if message_parts.len() == 2 {
                                            if let Value::BulkString(id_bytes) = &message_parts[0] {
                                                let id =
                                                    String::from_utf8_lossy(id_bytes).to_string();
                                                if let Value::Array(fields_data) = &message_parts[1]
                                                {
                                                    let mut fields = Vec::new();
                                                    for i in (0..fields_data.len()).step_by(2) {
                                                        if i + 1 < fields_data.len() {
                                                            if let (
                                                                Value::BulkString(f_bytes),
                                                                Value::BulkString(v_bytes),
                                                            ) = (
                                                                &fields_data[i],
                                                                &fields_data[i + 1],
                                                            ) {
                                                                fields.push((
                                                                    String::from_utf8_lossy(
                                                                        f_bytes,
                                                                    )
                                                                    .to_string(),
                                                                    String::from_utf8_lossy(
                                                                        v_bytes,
                                                                    )
                                                                    .to_string(),
                                                                ));
                                                            }
                                                        }
                                                    }
                                                    parsed_streams.push(StreamEntry { id, fields });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                self.value_viewer.selected_key_value_stream = Some(parsed_streams);
                self.value_viewer.selected_key_value = None;
                self.value_viewer.update_current_display_value();
            }
            Ok(other_value) => {
                self.value_viewer.selected_key_value_stream = None;
                self.value_viewer.selected_key_value = Some(format!(
                    "Unexpected value structure from XREADGROUP: {:?}",
                    other_value
                ));
                self.value_viewer.update_current_display_value();
            }
            Err(e) => {
                self.value_viewer.selected_key_value_stream = None;
                self.value_viewer.selected_key_value =
                    Some(format!("Error fetching stream: {}", e));
                self.value_viewer.update_current_display_value();
            }
        }
    }
}
