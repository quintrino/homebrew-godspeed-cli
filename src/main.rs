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
    label_ids: Vec<String>,
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

#[derive(Deserialize, Debug)]
struct LabelsResponse {
    labels: Vec<LabelItem>,
}

#[derive(Deserialize, Debug)]
struct LabelItem {
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

fn get_labels_path() -> PathBuf {
    get_xdg_data_home().join("godspeed-cli").join("labels.toml")
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

fn parse_task(input: &str) -> (TaskRequest, Option<String>, Vec<String>) {
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
                label_names.push(label.to_string());
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
            label_ids: Vec::new(), // Will be resolved later
            notes,
        },
        list_name,
        label_names,
    )
}

fn load_cache(path: &PathBuf) -> HashMap<String, String> {
    if let Ok(content) = fs::read_to_string(path) {
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

fn save_cache(path: &PathBuf, cache: &HashMap<String, String>) -> io::Result<()> {
    let mut table = toml::Table::new();
    for (key, value) in cache {
        table.insert(key.clone(), toml::Value::String(value.clone()));
    }
    let toml_string = toml::to_string(&table).unwrap();
    fs::write(path, toml_string)?;
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

fn fetch_labels(api_key: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get("https://api.godspeedapp.com/labels")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()?;

    let labels_response: LabelsResponse = response.json()?;
    let mut map = HashMap::new();
    for label in labels_response.labels {
        map.insert(label.name.to_lowercase(), label.id);
    }
    Ok(map)
}

fn find_matching_key(cache: &HashMap<String, String>, search: &str) -> Option<String> {
    let search_lower = search.to_lowercase();

    // First try exact match
    if let Some(id) = cache.get(&search_lower) {
        return Some(id.clone());
    }

    // Then try prefix match
    for (key, id) in cache {
        if key.starts_with(&search_lower) {
            return Some(id.clone());
        }
    }

    None
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
    let (mut parsed, list_name, label_names) = parse_task(task_str);

    // Handle list resolution
    if let Some(list_name_clean) = list_name {
        let mut lists_cache = load_cache(&get_lists_path());

        if let Some(list_id) = find_matching_key(&lists_cache, &list_name_clean) {
            parsed.list_id = Some(list_id);
        } else {
            // Fetch lists from API
            lists_cache = fetch_lists(api_key)?;
            save_cache(&get_lists_path(), &lists_cache)?;

            if let Some(list_id) = find_matching_key(&lists_cache, &list_name_clean) {
                parsed.list_id = Some(list_id);
            }
        }
    }

    // Handle label resolution
    if !label_names.is_empty() {
        let mut labels_cache = load_cache(&get_labels_path());

        // Check if we need to fetch labels
        let mut need_fetch = false;
        for label_name in &label_names {
            if find_matching_key(&labels_cache, label_name).is_none() {
                need_fetch = true;
                break;
            }
        }

        if need_fetch {
            labels_cache = fetch_labels(api_key)?;
            save_cache(&get_labels_path(), &labels_cache)?;
        }

        // Resolve all label names to IDs
        for label_name in label_names {
            if let Some(label_id) = find_matching_key(&labels_cache, &label_name) {
                parsed.label_ids.push(label_id);
            }
        }
    }

    // Check for multiple lists
    let list_count = task_str.split_whitespace().filter(|w| w.starts_with('@')).count();
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
