# Godspeed CLI User Guide

A lightweight command-line utility for quickly creating tasks in [Godspeed App](https://godspeedapp.com/).

## Installation

1. Set your API key:
```bash
export GODSPEED_API="your-api-key-here"
```

Add this to your `.zshrc` or `config.fish` to make it permanent.

Either use the binary godspeed-cli at /target/release/ or

2. Build the project:
```bash
cargo build --release
```

3. (Optional) Copy to your PATH:
```bash
cp target/release/godspeed-cli /usr/local/bin/
```


## Basic Usage

The CLI accepts input in two ways:

### Command-line arguments (recommended: use quotes)
```bash
godspeed-cli "Buy groceries and cook dinner"
```

### Via stdin
```bash
echo "Buy groceries and cook dinner" | godspeed-cli
```

## Special Syntax

### Labels with `.`
Add hashtags anywhere in your task to create labels. They'll be removed from the task title and added as labels (title-cased).

#NOTE: Because of Shell limitations, this only works with single word labels

```bash
godspeed-cli "Review pull request .Urgent .Work"
# Title: "Review pull request"
# Labels: ["Urgent", "Work"]
```

### Lists with `@`
Specify which list to add the task to using `@ListName`. The list name will be matched case-insensitively against your Godspeed lists.

#NOTE: Because of Shell limitations, this only works with single word lists

```bash
godspeed-cli "Call dentist @Personal"
# Adds to your "Personal" list
```

**Note**: Only one list can be specified per task. If multiple lists are detected, you'll receive an error notification.

### Duration with `:`
Set the task duration in minutes using `:` followed by a number.

```bash
godspeed-cli "Workout session :45"
# Title: "Workout session"
# Duration: 45 minutes
```

### Notes with `n:`
Add detailed notes to your task using `n:` followed by the note content. Everything after `n:` becomes the note.

```bash
godspeed-cli "Buy ingredients n: Need milk, eggs, flour, and butter for baking"
# Title: "Buy ingredients"
# Notes: "Need milk, eggs, flour, and butter for baking"
```

## Combining Features

You can combine all special syntax in a single task:

```bash
godspeed-cli "Write blog post @Work .Writing :120 n: Focus on the new API features and include code examples"
```

This creates a task with:
- Title: "Write blog post"
- List: Your "Work" list
- Labels: ["Writing"]
- Duration: 120 minutes
- Notes: "Focus on the new API features and include code examples"

## Multi-line Tasks

The CLI supports multi-line input, which is especially useful for detailed tasks:

```bash
echo "Project planning session
Agenda items:
- Review Q4 goals
- Discuss team resources
.Management @Work :90" | godspeed-cli
```

## Offline Cache

If the API is unreachable or a request fails, the task is automatically cached locally. The next time you run the CLI (for any task), it will:

1. Attempt to send all cached tasks first
2. Remove successfully sent tasks from the cache
3. Process your new task

This ensures you never lose tasks due to connectivity issues.

Cache location: `$XDG_DATA_HOME/godspeed-cli/cache` (usually `~/.local/share/godspeed-cli/cache`)

## List Caching

When you first reference a list with `@ListName`, the CLI fetches all your lists from the Godspeed API and caches them locally for fast lookups.

Cache location: `$XDG_DATA_HOME/godspeed-cli/lists.toml`

To refresh the list cache, simply delete this file and the CLI will re-fetch on the next run.

## Error Notifications

The CLI uses macOS notifications (via `osascript`) to alert you of errors:

- **"GODSPEED_API environment variable not set"**: You need to set your API key
- **"Failed to send task"**: The API request failed (task is cached for retry)
- **"Error: Multiple lists specified"**: You used more than one `@list` in a single task

## Tips and Tricks

### Quick capture from anywhere
Create a shell alias or keyboard shortcut:

```bash
# In .zshrc or config.fish
alias gs='godspeed-cli'

# Quick usage
gs "Task with .label @list"
```

### Shell history protection
Because `#` starts a comment in most shells, always use quotes or escape hashes

```bash
# ✅ Good - quoted
godspeed-cli "Task .Urgent #hashtag"

# ✅ Good - escaped
godspeed-cli Task \#hashtag

# ❌ Bad - shell treats #hashtag as a comment
godspeed-cli Task #hashtag
```

### Piping from other tools
Combine with other CLI tools for powerful workflows:

```bash
# From clipboard (macOS)
pbpaste | godspeed-cli

# From a file
cat todo.txt | godspeed-cli

# From a command
echo "Deploy to production @DevOps .Urgent :30" | godspeed-cli
```

### Case-insensitive matching
List names are matched case-insensitively, so these are equivalent:

```bash
godspeed-cli "Task @work"
godspeed-cli "Task @Work"
godspeed-cli "Task @WORK"
```

## Troubleshooting

### "GODSPEED_API environment variable not set"
Make sure you've exported your API key:
```bash
export GODSPEED_API="your-key"
```

### List not found
If your `@ListName` isn't being recognized:
1. Check the spelling matches your Godspeed list
2. Delete `~/.local/share/godspeed-cli/lists.toml` to refresh the cache
3. Run the CLI again to re-fetch your lists

### Task appears in cache repeatedly
If a task keeps failing and accumulating in the cache, check:
1. Your API key is valid
2. The Godspeed API is accessible
3. Your list name (if using `@`) exists

You can manually edit or clear the cache file at `~/.local/share/godspeed-cli/cache`

## Data Storage

All data is stored in `$XDG_DATA_HOME/godspeed-cli/` (typically `~/.local/share/godspeed-cli/`):

- `cache`: Failed tasks waiting to be sent (plain text, separator: `---`)
- `lists.toml`: Cached list name → ID mappings (TOML format)

## Examples

```bash
# Simple task
godspeed-cli "Call mom"

# Task with label
godspeed-cli "Review code .Development"

# Task with list and duration
godspeed-cli "Gym workout @Health :60"

# Complete task with all features
godspeed-cli "Prepare presentation @Work .Important :120 n: Include Q3 metrics and team feedback"

# Multi-line via stdin
echo "Research new framework
- Check documentation
- Test examples
- Write summary
.Development @Learning :180" | godspeed-cli
````
