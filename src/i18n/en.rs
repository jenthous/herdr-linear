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
};
