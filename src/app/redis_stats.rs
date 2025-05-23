use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RedisStats {
    pub memory_used: u64,
    pub memory_used_human: String,
    pub memory_peak: u64,
    pub memory_peak_human: String,
    pub memory_rss: u64,
    pub memory_rss_human: String,
    pub connected_clients: u32,
    pub blocked_clients: u32,
    pub total_commands_processed: u64,
    pub instantaneous_ops_per_sec: u32,
    pub keyspace_hits: u64,
    pub keyspace_misses: u64,
    pub hit_rate: f64,
    pub uptime_in_seconds: u64,
    pub uptime_human: String,
    pub redis_version: String,
    pub redis_mode: String,
    pub role: String,
    pub connected_slaves: u32,
    pub used_cpu_sys: f64,
    pub used_cpu_user: f64,
    pub last_updated: Instant,
}

impl Default for RedisStats {
    fn default() -> Self {
        Self {
            memory_used: 0,
            memory_used_human: "0B".to_string(),
            memory_peak: 0,
            memory_peak_human: "0B".to_string(),
            memory_rss: 0,
            memory_rss_human: "0B".to_string(),
            connected_clients: 0,
            blocked_clients: 0,
            total_commands_processed: 0,
            instantaneous_ops_per_sec: 0,
            keyspace_hits: 0,
            keyspace_misses: 0,
            hit_rate: 0.0,
            uptime_in_seconds: 0,
            uptime_human: "0s".to_string(),
            redis_version: "Unknown".to_string(),
            redis_mode: "Unknown".to_string(),
            role: "Unknown".to_string(),
            connected_slaves: 0,
            used_cpu_sys: 0.0,
            used_cpu_user: 0.0,
            last_updated: Instant::now(),
        }
    }
}

impl RedisStats {
    pub fn from_info_string(info: &str) -> Self {
        let mut stats = RedisStats::default();
        let mut parsed_data: HashMap<String, String> = HashMap::new();

        // Parse the INFO response
        for line in info.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once(':') {
                parsed_data.insert(key.to_string(), value.to_string());
            }
        }

        // Extract memory information
        if let Some(memory_used) = parsed_data.get("used_memory") {
            stats.memory_used = memory_used.parse().unwrap_or(0);
            stats.memory_used_human = format_bytes(stats.memory_used);
        }
        if let Some(memory_peak) = parsed_data.get("used_memory_peak") {
            stats.memory_peak = memory_peak.parse().unwrap_or(0);
            stats.memory_peak_human = format_bytes(stats.memory_peak);
        }
        if let Some(memory_rss) = parsed_data.get("used_memory_rss") {
            stats.memory_rss = memory_rss.parse().unwrap_or(0);
            stats.memory_rss_human = format_bytes(stats.memory_rss);
        }

        // Extract client information
        if let Some(connected_clients) = parsed_data.get("connected_clients") {
            stats.connected_clients = connected_clients.parse().unwrap_or(0);
        }
        if let Some(blocked_clients) = parsed_data.get("blocked_clients") {
            stats.blocked_clients = blocked_clients.parse().unwrap_or(0);
        }

        // Extract command statistics
        if let Some(total_commands) = parsed_data.get("total_commands_processed") {
            stats.total_commands_processed = total_commands.parse().unwrap_or(0);
        }
        if let Some(ops_per_sec) = parsed_data.get("instantaneous_ops_per_sec") {
            stats.instantaneous_ops_per_sec = ops_per_sec.parse().unwrap_or(0);
        }

        // Extract keyspace statistics
        if let Some(hits) = parsed_data.get("keyspace_hits") {
            stats.keyspace_hits = hits.parse().unwrap_or(0);
        }
        if let Some(misses) = parsed_data.get("keyspace_misses") {
            stats.keyspace_misses = misses.parse().unwrap_or(0);
        }
        
        // Calculate hit rate
        let total_requests = stats.keyspace_hits + stats.keyspace_misses;
        if total_requests > 0 {
            stats.hit_rate = (stats.keyspace_hits as f64 / total_requests as f64) * 100.0;
        }

        // Extract server information
        if let Some(uptime) = parsed_data.get("uptime_in_seconds") {
            stats.uptime_in_seconds = uptime.parse().unwrap_or(0);
            stats.uptime_human = format_duration(stats.uptime_in_seconds);
        }
        if let Some(version) = parsed_data.get("redis_version") {
            stats.redis_version = version.clone();
        }
        if let Some(mode) = parsed_data.get("redis_mode") {
            stats.redis_mode = mode.clone();
        }
        if let Some(role) = parsed_data.get("role") {
            stats.role = role.clone();
        }
        if let Some(slaves) = parsed_data.get("connected_slaves") {
            stats.connected_slaves = slaves.parse().unwrap_or(0);
        }

        // Extract CPU information
        if let Some(cpu_sys) = parsed_data.get("used_cpu_sys") {
            stats.used_cpu_sys = cpu_sys.parse().unwrap_or(0.0);
        }
        if let Some(cpu_user) = parsed_data.get("used_cpu_user") {
            stats.used_cpu_user = cpu_user.parse().unwrap_or(0.0);
        }

        stats.last_updated = Instant::now();
        stats
    }

    pub fn age(&self) -> Duration {
        self.last_updated.elapsed()
    }

    pub fn is_stale(&self, max_age: Duration) -> bool {
        self.age() > max_age
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
} 