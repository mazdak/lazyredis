use super::{value_format, App, StreamEntry};
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
        OkF: FnOnce(&mut Self, T) -> Result<(), String>,
        ErrF: FnOnce(&mut Self),
    {
        match fut.await {
            Ok(val) => {
                if let Err(message) = on_ok(self, val) {
                    self.value_viewer.selected_key_value = Some(message);
                }
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
        let fut = owned_cmd.query_async::<Value>(con);
        let err_context = format!("Failed to HGETALL for '{}' (hash)", key_name);
        self.run_fetch(
            fut,
            |app, value| parse_hash_value(app, key_name, value),
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
        let fut = owned_cmd.query_async::<Value>(con);
        let err_context = format!("Failed to ZRANGE for '{}' (zset)", key_name);
        self.run_fetch(
            fut,
            |app, value| parse_zset_value(app, key_name, value),
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
        let fut = owned_cmd.query_async::<Value>(con);
        let err_context = format!("Failed to LRANGE for '{}' (list)", key_name);
        self.run_fetch(
            fut,
            |app, value| parse_list_value(app, key_name, value),
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
        let fut = owned_cmd.query_async::<Value>(con);
        let err_context = format!("Failed to SMEMBERS for '{}' (set)", key_name);
        self.run_fetch(
            fut,
            |app, value| parse_set_value(app, key_name, value),
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
                app.value_viewer.selected_key_value_json =
                    Some(value_format::format_json_pretty(&value));
                Ok(())
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
        let result = redis::cmd("XREVRANGE")
            .arg(key_name)
            .arg("+")
            .arg("-")
            .arg("COUNT")
            .arg(100)
            .query_async::<Value>(con)
            .await;

        match result {
            Ok(value) => match parse_stream_entries(value) {
                Ok(mut entries) => {
                    entries.reverse();
                    self.value_viewer.selected_key_value_stream = Some(entries);
                    self.value_viewer.selected_key_value = None;
                    self.value_viewer.update_current_display_value();
                }
                Err(message) => {
                    self.value_viewer.selected_key_value_stream = None;
                    self.value_viewer.selected_key_value = Some(message);
                    self.value_viewer.update_current_display_value();
                }
            },
            Err(e) => {
                self.value_viewer.selected_key_value_stream = None;
                self.value_viewer.selected_key_value =
                    Some(format!("Error fetching stream: {}", e));
                self.value_viewer.update_current_display_value();
            }
        }
    }
}

fn parse_hash_value(app: &mut App, key_name: &str, value: Value) -> Result<(), String> {
    let values = expect_array(value, "HGETALL")?;
    if values.is_empty() {
        app.value_viewer.selected_key_value_hash = Some(Vec::new());
        app.value_viewer.selected_key_value = None;
        return Ok(());
    }

    let mut hash_data = Vec::new();
    for chunk in values.chunks(2) {
        if chunk.len() != 2 {
            app.value_viewer.selected_key_value_hash = None;
            return Err(format!(
                "HGETALL for '{}' (hash) returned malformed pair data.",
                key_name
            ));
        }
        let field = value_to_bytes(&chunk[0]).unwrap_or_default();
        let value_bytes = value_to_bytes(&chunk[1]).unwrap_or_default();
        hash_data.push((
            value_format::format_bytes_inline(&field),
            value_format::format_bytes_inline(&value_bytes),
        ));
    }
    app.value_viewer.selected_key_value_hash = Some(hash_data);
    app.value_viewer.selected_key_value = None;
    Ok(())
}

fn parse_zset_value(app: &mut App, key_name: &str, value: Value) -> Result<(), String> {
    let values = expect_array(value, "ZRANGE")?;
    if values.is_empty() {
        app.value_viewer.selected_key_value_zset = Some(Vec::new());
        app.value_viewer.selected_key_value = None;
        return Ok(());
    }

    let mut zset_data = Vec::new();
    for chunk in values.chunks(2) {
        if chunk.len() != 2 {
            app.value_viewer.selected_key_value_zset = None;
            return Err(format!(
                "ZRANGE for '{}' (zset) returned malformed pair data.",
                key_name
            ));
        }
        let member_bytes = value_to_bytes(&chunk[0]).unwrap_or_default();
        let member = value_format::format_bytes_inline(&member_bytes);
        let score = value_to_f64(&chunk[1]).ok_or_else(|| {
            format!(
                "ZRANGE for '{}' (zset) failed to parse score for member '{}'.",
                key_name, member
            )
        })?;
        zset_data.push((member, score));
    }
    app.value_viewer.selected_key_value_zset = Some(zset_data);
    app.value_viewer.selected_key_value = None;
    Ok(())
}

fn parse_list_value(app: &mut App, _key_name: &str, value: Value) -> Result<(), String> {
    let values = expect_array(value, "LRANGE")?;
    let list = values
        .iter()
        .map(|entry| {
            let bytes = value_to_bytes(entry).unwrap_or_default();
            value_format::format_bytes_inline(&bytes)
        })
        .collect::<Vec<String>>();
    app.value_viewer.selected_key_value_list = Some(list);
    app.value_viewer.selected_key_value = None;
    Ok(())
}

fn parse_set_value(app: &mut App, _key_name: &str, value: Value) -> Result<(), String> {
    let values = expect_array(value, "SMEMBERS")?;
    let set = values
        .iter()
        .map(|entry| {
            let bytes = value_to_bytes(entry).unwrap_or_default();
            value_format::format_bytes_inline(&bytes)
        })
        .collect::<Vec<String>>();
    app.value_viewer.selected_key_value_set = Some(set);
    app.value_viewer.selected_key_value = None;
    Ok(())
}

fn parse_stream_entries(value: Value) -> Result<Vec<StreamEntry>, String> {
    let values = expect_array(value, "XREVRANGE")?;
    let mut parsed_streams = Vec::new();
    for entry in values {
        let entry_parts = match entry {
            Value::Array(parts) => parts,
            _ => {
                return Err("Unexpected stream entry structure from XREVRANGE.".to_string())
            }
        };
        if entry_parts.len() != 2 {
            return Err("Unexpected stream entry structure from XREVRANGE.".to_string());
        }
        let id_bytes = value_to_bytes(&entry_parts[0]).unwrap_or_default();
        let id = value_format::format_bytes_inline(&id_bytes);
        let fields_data = match &entry_parts[1] {
            Value::Array(fields) => fields,
            _ => {
                return Err("Unexpected stream fields structure from XREVRANGE.".to_string())
            }
        };
        let mut fields = Vec::new();
        for chunk in fields_data.chunks(2) {
            if chunk.len() != 2 {
                return Err("Unexpected stream fields structure from XREVRANGE.".to_string());
            }
            let field_bytes = value_to_bytes(&chunk[0]).unwrap_or_default();
            let value_bytes = value_to_bytes(&chunk[1]).unwrap_or_default();
            fields.push((
                value_format::format_bytes_inline(&field_bytes),
                value_format::format_bytes_inline(&value_bytes),
            ));
        }
        parsed_streams.push(StreamEntry { id, fields });
    }
    Ok(parsed_streams)
}

fn expect_array(value: Value, command: &str) -> Result<Vec<Value>, String> {
    match value {
        Value::Nil => Ok(Vec::new()),
        Value::Array(values) => Ok(values),
        other => Err(format!(
            "Unexpected value structure from {}: {:?}",
            command, other
        )),
    }
}

fn value_to_bytes(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::BulkString(bytes) => Some(bytes.clone()),
        Value::SimpleString(text) => Some(text.as_bytes().to_vec()),
        Value::Int(num) => Some(num.to_string().into_bytes()),
        Value::Double(num) => Some(num.to_string().into_bytes()),
        Value::Okay => Some(b"OK".to_vec()),
        _ => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Double(num) => Some(*num),
        Value::Int(num) => Some(*num as f64),
        Value::BulkString(bytes) => std::str::from_utf8(bytes).ok()?.parse::<f64>().ok(),
        Value::SimpleString(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stream_entries_handles_basic_entry() {
        let value = Value::Array(vec![Value::Array(vec![
            Value::BulkString(b"1-0".to_vec()),
            Value::Array(vec![
                Value::BulkString(b"field".to_vec()),
                Value::BulkString(b"value".to_vec()),
            ]),
        ])]);

        let entries = parse_stream_entries(value).expect("parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "1-0");
        assert_eq!(
            entries[0].fields,
            vec![("field".to_string(), "value".to_string())]
        );
    }
}
