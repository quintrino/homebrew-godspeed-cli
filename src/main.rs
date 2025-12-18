use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::Command;

#[derive(Serialize, Deserialize, Debug)]
struct TaskRequest {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    list_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_minutes: Option<i32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    label_names: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    notes: String,
}

#[derive(Deserialize, Debug)]
struct ListsResponse {
    lists: Vec<ListItem>,
}

#[derive(Deserialize, Debug)]
struct ListItem {
    id: String,
    name: String,
}

fn get_xdg_data_home() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".local").join("share")
    } else {
        PathBuf::from(".local").join("share")
    }
}

fn get_cache_path() -> PathBuf {
    get_xdg_data_home().join("godspeed-cli").join("cache")
}

fn get_lists_path() -> PathBuf {
    get_xdg_data_home().join("godspeed-cli").join("lists.toml")
}

fn ensure_directories() -> io::Result<()> {
    let data_dir = get_xdg_data_home().join("godspeed-cli");
    fs::create_dir_all(&data_dir)?;
    Ok(())
}

fn send_notification(message: &str) {
    let script = format!(
        r#"display notification "{}" with title "Godspeed CLI""#,
        message.replace('"', "\\\"")
    );
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output();
}

fn parse_task(input: &str) -> (TaskRequest, Option<String>) {
    let mut title = String::new();
    let mut list_name: Option<String> = None;
    let mut duration_minutes: Option<i32> = None;
    let mut label_names: Vec<String> = Vec::new();

    // Check for notes separator
    let (main_part, notes_part) = if let Some(pos) = input.find(" n:") {
        let (main, note) = input.split_at(pos);
        (main, note.trim_start_matches(" n:").trim())
    } else {
        (input, "")
    };

    let notes = notes_part.to_string();

    for word in main_part.split_whitespace() {
        if word.starts_with('.') {
            // Extract label
            let label = word.trim_start_matches('.');
            if !label.is_empty() {
                label_names.push(titlecase(label));
            }
        } else if word.starts_with('@') {
            // Extract list name
            let list = word.trim_start_matches('@');
            if !list.is_empty() {
                list_name = Some(list.to_string());
            }
        } else if word.starts_with(':') {
            // Extract duration
            let duration_str = word.trim_start_matches(':');
            if let Ok(duration) = duration_str.parse::<i32>() {
                duration_minutes = Some(duration);
            } else {
                // If parsing fails, include it in the title
                if !title.is_empty() {
                    title.push(' ');
                }
                title.push_str(word);
            }
        } else {
            if !title.is_empty() {
                title.push(' ');
            }
            title.push_str(word);
        }
    }

    let title = title.trim_end().to_string();

    (
        TaskRequest {
            title,
            list_id: None, // Will be resolved later
            duration_minutes,
            label_names,
            notes,
        },
        list_name,
    )
}

fn titlecase(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

fn load_lists_cache() -> HashMap<String, String> {
    let lists_path = get_lists_path();
    if let Ok(content) = fs::read_to_string(&lists_path) {
        if let Ok(table) = content.parse::<toml::Table>() {
            let mut map = HashMap::new();
            for (key, value) in table {
                if let Some(val_str) = value.as_str() {
                    map.insert(key.to_lowercase(), val_str.to_string());
                }
            }
            return map;
        }
    }
    HashMap::new()
}

fn save_lists_cache(lists: &HashMap<String, String>) -> io::Result<()> {
    let lists_path = get_lists_path();
    let mut table = toml::Table::new();
    for (key, value) in lists {
        table.insert(key.clone(), toml::Value::String(value.clone()));
    }
    let toml_string = toml::to_string(&table).unwrap();
    fs::write(&lists_path, toml_string)?;
    Ok(())
}

fn fetch_lists(api_key: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get("https://api.godspeedapp.com/lists")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()?;

    let lists_response: ListsResponse = response.json()?;
    let mut map = HashMap::new();
    for list in lists_response.lists {
        map.insert(list.name.to_lowercase(), list.id);
    }
    Ok(map)
}

fn send_task(task: &TaskRequest, api_key: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .post("https://api.godspeedapp.com/tasks")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(task)
        .send()?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("API error: {}", response.status()).into())
    }
}

fn add_to_cache(task_str: &str) -> io::Result<()> {
    let cache_path = get_cache_path();
    let mut cache_content = fs::read_to_string(&cache_path).unwrap_or_default();
    cache_content.push_str(task_str);
    cache_content.push('\n');
    cache_content.push_str("---\n");
    fs::write(&cache_path, cache_content)?;
    Ok(())
}

fn get_cached_tasks() -> Vec<String> {
    let cache_path = get_cache_path();
    if let Ok(content) = fs::read_to_string(&cache_path) {
        content
            .split("---\n")
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect()
    } else {
        Vec::new()
    }
}

fn remove_from_cache(task_str: &str) -> io::Result<()> {
    let cache_path = get_cache_path();
    let content = fs::read_to_string(&cache_path).unwrap_or_default();
    let remaining: Vec<&str> = content
        .split("---\n")
        .filter(|s| !s.trim().is_empty() && s.trim() != task_str)
        .collect();

    let new_content = if remaining.is_empty() {
        String::new()
    } else {
        remaining.join("---\n") + "\n---\n"
    };

    fs::write(&cache_path, new_content)?;
    Ok(())
}

fn process_task(task_str: &str, api_key: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (mut parsed, list_name) = parse_task(task_str);

    // Handle list resolution
    if let Some(list_name_clean) = list_name {
        let list_name_lower = list_name_clean.to_lowercase();
        let mut lists_cache = load_lists_cache();

        if let Some(list_id) = lists_cache.get(&list_name_lower) {
            parsed.list_id = Some(list_id.clone());
        } else {
            // Fetch lists from API
            lists_cache = fetch_lists(api_key)?;
            save_lists_cache(&lists_cache)?;

            if let Some(list_id) = lists_cache.get(&list_name_lower) {
                parsed.list_id = Some(list_id.clone());
            }
        }
    }

    // Check for multiple lists
    let list_count = task_str.split_whitespace().filter(|w| w.starts_with('~')).count();
    if list_count > 1 {
        send_notification("Error: Multiple lists specified");
        return Err("Multiple lists specified".into());
    }

    send_task(&parsed, api_key)?;
    Ok(())
}

fn main() {
    if let Err(e) = ensure_directories() {
        eprintln!("Failed to create directories: {}", e);
        return;
    }

    let api_key = match env::var("GODSPEED_API") {
        Ok(key) => key,
        Err(_) => {
            send_notification("GODSPEED_API environment variable not set");
            eprintln!("Error: GODSPEED_API environment variable not set");
            return;
        }
    };

    // Get input from args or stdin
    let input = {
        let args: Vec<String> = env::args().skip(1).collect();
        if !args.is_empty() {
            // Join all arguments with spaces to handle multi-word input
            args.join(" ")
        } else {
            // Read from stdin
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer).unwrap_or_default();
            buffer.trim_end().to_string()
        }
    };

    // Process cached tasks first
    let cached_tasks = get_cached_tasks();
    for cached_task in cached_tasks {
        if let Ok(_) = process_task(&cached_task, &api_key) {
            let _ = remove_from_cache(&cached_task);
        }
    }

    // Process current input
    if !input.is_empty() {
        if let Err(e) = process_task(&input, &api_key) {
            eprintln!("Failed to send task: {}", e);
            let _ = add_to_cache(&input);
            send_notification("Failed to send task");
        }
    }
}
