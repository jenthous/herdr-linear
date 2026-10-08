//! 영어(기본 언어).

use super::Texts;

pub const EN: Texts = Texts {
    cli_about: "Fast Linear lookup in herdr",
    cli_login: "Enter an API key, verify it, and save it",
    cli_logout: "Delete the saved API key and cache",
    cli_whoami: "Show the connected account and workspace",
    cli_mine: "List open issues assigned to you",
    cli_search: "Search issues (e.g. login l:bug s:started @me #ENG p:high)",
    cli_search_deep: "Deep search on the server (includes comments, 30 per minute)",
    cli_show: "Show an issue (e.g. ENG-131)",
    cli_open: "herdr action: open the palette with the original pane's context, or open and close the side pane",
    cli_open_target: "palette: shortcut or command palette, url: Ctrl+click on a Linear issue link, side: open or close the side pane",
    cli_ui: "The screen that runs inside a herdr pane",
    write_failed: |e| format!("error: couldn't write the output: {e}"),
    error_line: |m| format!("error: {m}"),
    warning_line: |w| format!("warning: {w}"),
    no_api_key: "No API key. Run `herdr-linear login` first",
    no_config_dir: "Can't find the config directory (HOME is not set)",
    no_state_dir: "Can't find the state directory (HOME is not set)",
    config_read_failed: |e| format!("Couldn't read config.toml: {e}"),
    config_invalid: |e| format!("config.toml is not valid, using the defaults: {e}"),
    teams_not_list: "teams must be a list of strings. Using the default",
    template_not_string: "agent.template must be a string. Using the default",
    int_out_of_range: |section, key, min, max| {
        format!("{section}.{key} must be a whole number from {min} to {max}. Using the default")
    },
    language_unsupported: |v| {
        format!("language \"{v}\" is not supported. Using English (en, ko, ja, zh-CN, de)")
    },
    language_not_string: "language must be a string. Using English",
    read_failed: |p| format!("Couldn't read {p}"),
    empty_key: "Can't save an empty key",
    delete_failed: |p| format!("Couldn't delete {p}"),
    mkdir_failed: |p| format!("Couldn't create the directory {p}"),
    priority_urgent: "Urgent",
    priority_high: "High",
    priority_medium: "Medium",
    priority_low: "Low",
    priority_none: "None",
    just_now: "just now",
    minutes_ago: |m| format!("{m}m ago"),
    hours_ago: |h| format!("{h}h ago"),
    days_ago: |d| format!("{d}d ago"),
    rel_parent: "Parent",
    rel_blocked_by: "Blocked by",
    rel_blocking: "Blocking",
    rel_related: "Related",
    rel_child: "Sub-issues",
    children_over: |total| format!("over {total}"),
    children_all_done: |total| {
        if total == 1 {
            "1 done".to_string()
        } else {
            format!("all {total} done")
        }
    },
    children_left: |total, left| {
        vec![
            (false, format!("{left} open")),
            (true, format!(" · {total} total")),
        ]
    },
    more_children: "… more (o to view in the browser)",
    image_placeholder: |index, label| format!("[image {index}: {label}]"),
    api_rate_limited: "Linear API rate limit reached",
    api_auth: "The API key has expired or lacks access",
    api_graphql: |m| format!("Linear couldn't process the request: {m}"),
    api_offline: |m| format!("Offline: {m}"),
    api_decode: |m| format!("Couldn't read the response: {m}"),
    server_error: |s| format!("Linear server error ({s})"),
    no_data: "No data in the response",
    cache_recreate_failed: "Couldn't recreate the cache DB",
    cache_open_failed: "Couldn't open the cache DB",
    run_failed: |p| format!("Couldn't run {p}"),
};
