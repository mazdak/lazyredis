use redis::Value;
use crate::app::MultiplexedConnection;

#[derive(Debug)]
pub struct CommandState {
    pub input_buffer: String,
    pub last_result: Option<String>,
    pub is_active: bool,
}

impl CommandState {
    pub fn new() -> Self {
        CommandState {
            input_buffer: String::new(),
            last_result: None,
            is_active: false,
        }
    }

    pub fn open(&mut self) {
        self.is_active = true;
        self.input_buffer.clear();
        self.last_result = None;
    }

    pub fn close(&mut self) {
        self.is_active = false;
    }

    pub async fn execute_command(&mut self, connection: &mut Option<MultiplexedConnection>) {
        if self.input_buffer.is_empty() {
            self.last_result = Some("Command is empty.".to_string());
            return;
        }

        if let Some(mut con) = connection.take() {
            let parts: Vec<&str> = self.input_buffer.split_whitespace().collect();
            if parts.is_empty() {
                self.last_result = Some("No command entered.".to_string());
                *connection = Some(con);
                return;
            }

            let cmd_str = parts[0];
            let args = &parts[1..];

            let mut cmd = redis::cmd(cmd_str);
            for arg in args {
                cmd.arg(*arg);
            }
            match cmd.query_async::<Value>(&mut con).await {
                Ok(val) => self.last_result = Some(format!("{:?}", val)),
                Err(e) => self.last_result = Some(format!("Error: {}", e)),
            }
            *connection = Some(con);
        } else {
            self.last_result = Some("Not connected".to_string());
        }
    }
}

impl Default for CommandState {
    fn default() -> Self {
        Self::new()
    }
}
