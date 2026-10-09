//! Deutsch.

use super::Texts;

pub const DE: Texts = Texts {
    cli_about: "Linear in herdr schnell nachschlagen",
    cli_login: "API-Schlüssel eingeben, prüfen und speichern",
    cli_logout: "Gespeicherten API-Schlüssel und Cache löschen",
    cli_whoami: "Verbundenes Konto und Workspace anzeigen",
    cli_mine: "Mir zugewiesene offene Issues auflisten",
    cli_search: "Issues suchen (z. B. Login l:bug s:started @me #ENG p:high)",
    cli_search_deep: "Tiefensuche auf dem Server (mit Kommentaren, bis zu 30 pro Minute)",
    cli_show: "Issue anzeigen (z. B. ENG-131)",
    cli_open: "herdr-Aktion: Palette mit dem Kontext des ursprünglichen Bereichs öffnen oder den Seitenbereich ein- und ausblenden",
    cli_open_target: "palette: Tastenkürzel oder Befehlspalette, url: Ctrl+Klick auf einen Linear-Issue-Link, side: Seitenbereich ein- oder ausblenden",
    cli_ui: "Oberfläche, die in einem herdr-Bereich läuft",
    write_failed: |e| format!("Fehler: Ausgabe konnte nicht geschrieben werden: {e}"),
    error_line: |m| format!("Fehler: {m}"),
    warning_line: |w| format!("Warnung: {w}"),
    no_api_key: "Kein API-Schlüssel. Zuerst `herdr-linear login` ausführen",
    no_config_dir: "Konfigurationsverzeichnis nicht gefunden (HOME ist nicht gesetzt)",
    no_state_dir: "Statusverzeichnis nicht gefunden (HOME ist nicht gesetzt)",
    config_read_failed: |e| format!("config.toml konnte nicht gelesen werden: {e}"),
    config_invalid: |e| format!("config.toml ist ungültig, Standardwerte werden verwendet: {e}"),
    teams_not_list: "teams muss eine Liste von Zeichenfolgen sein. Standardwert wird verwendet",
    template_not_string: "agent.template muss eine Zeichenfolge sein. Standardwert wird verwendet",
    int_out_of_range: |section, key, min, max| {
        format!(
            "{section}.{key} muss eine ganze Zahl von {min} bis {max} sein. Standardwert wird verwendet"
        )
    },
    language_unsupported: |v| {
        format!(
            "language \"{v}\" wird nicht unterstützt. Englisch wird verwendet (en, ko, ja, zh-CN, de)"
        )
    },
    language_not_string: "language muss eine Zeichenfolge sein. Englisch wird verwendet",
    read_failed: |p| format!("{p} konnte nicht gelesen werden"),
    empty_key: "Ein leerer Schlüssel kann nicht gespeichert werden",
    delete_failed: |p| format!("{p} konnte nicht gelöscht werden"),
    mkdir_failed: |p| format!("Verzeichnis {p} konnte nicht angelegt werden"),
    priority_urgent: "Dringend",
    priority_high: "Hoch",
    priority_medium: "Mittel",
    priority_low: "Niedrig",
    priority_none: "Keine",
    just_now: "gerade eben",
    minutes_ago: |m| format!("vor {m} Min."),
    hours_ago: |h| format!("vor {h} Std."),
    days_ago: |d| format!("vor {d} Tg."),
    rel_parent: "Übergeordnet",
    rel_blocked_by: "Blockiert von",
    rel_blocking: "Blocker für",
    rel_related: "Verwandt",
    rel_child: "Sub-Issues",
    children_over: |total| format!("über {total}"),
    children_all_done: |total| {
        if total == 1 {
            "1 erledigt".to_string()
        } else {
            format!("alle {total} erledigt")
        }
    },
    children_left: |total, left| {
        vec![
            (false, format!("{left} offen")),
            (true, format!(" · {total} gesamt")),
        ]
    },
    more_children: "… weitere (mit o im Browser ansehen)",
    image_placeholder: |index, label| format!("[Bild {index}: {label}]"),
    api_rate_limited: "Linear-API-Limit erreicht",
    api_auth: "Der API-Schlüssel ist abgelaufen oder hat keinen Zugriff",
    api_graphql: |m| format!("Linear konnte die Anfrage nicht verarbeiten: {m}"),
    api_offline: |m| format!("Offline: {m}"),
    api_decode: |m| format!("Antwort konnte nicht gelesen werden: {m}"),
    server_error: |s| format!("Linear-Serverfehler ({s})"),
    no_data: "Keine Daten in der Antwort",
    cache_recreate_failed: "Cache-Datenbank konnte nicht neu erstellt werden",
    cache_open_failed: "Cache-Datenbank konnte nicht geöffnet werden",
    run_failed: |p| format!("{p} konnte nicht ausgeführt werden"),
    tab_mine: "Meine",
    tab_recent: "Zuletzt",
    tab_all: "Alle",
    deep_limit: "Tiefensuche: höchstens 30 Suchen pro Minute. Kurz warten",
    menu_actions: "Aktionen",
    menu_copy_url: "URL kopieren",
    menu_copy_pr: |pr| format!("PR-Link kopieren ({pr})"),
    menu_copy_id: "ID kopieren",
    menu_open: "Details öffnen",
    menu_browser: "Im Browser öffnen",
    menu_links: "Links und Bilder",
    menu_relations: "Verwandte Issues",
    menu_deep_search: "Tiefensuche auf dem Server (mit Kommentaren)",
    menu_refresh: "Aktualisieren",
    menu_back: "Zurück",
    menu_close: "Schließen",
    links_title: "Links und Bilder",
    links_none: "Keine Links",
    relations_title: "Beziehungen",
    relations_loading: "Beziehungen werden geladen…",
    relations_failed: "Beziehungen: Fehler beim Laden",
    relations_none: "Keine Beziehungen",
    no_open_pr: |id| format!("{id} hat keinen offenen PR"),
    connected: |name, org| format!("Verbunden mit {org} als {name}"),
    key_expired_paste: "Schlüssel abgelaufen oder ohne Zugriff. Neuen einfügen",
    rate_limited_retry_in: |m| {
        format!(
            "Linear-API-Limit erreicht. In {} erneut versuchen",
            minuten(m)
        )
    },
    rate_limited_retry_later: "Linear-API-Limit erreicht. Kurz warten und erneut versuchen",
    throttled: |m| {
        format!(
            "Wenige API-Anfragen übrig. Auto-Suche {} pausiert",
            minuten(m)
        )
    },
    key_invalid: "Der Schlüssel ist ungültig. In Linear prüfen",
    key_offline: "Offline, daher kann der Schlüssel nicht geprüft werden",
    key_save_failed: |e| format!("Schlüssel konnte nicht gespeichert werden: {e}"),
    only_web_links: "Nur http(s)-Links können geöffnet werden",
    opened_in_browser: "Im Browser geöffnet",
    browser_failed: |e| format!("Browser konnte nicht geöffnet werden: {e}"),
    copied: |what| format!("Kopiert: {what}"),
    copy_failed: |e| format!("Kopieren fehlgeschlagen: {e}"),
    log_viewer: "Benutzerinfo",
    log_list: "Liste",
    log_search: "Suche",
    log_detail: "Details",
    log_branch: "Branch-Issue",
    log_deep_limited: "Tiefensuche: Limit erreicht",
    log_cache_error: |e| format!("Cache-Fehler: {e}"),
    log_key_save_failed: |e| format!("Speichern des Schlüssels fehlgeschlagen: {e}"),
    log_browser_failed: |e| format!("Öffnen des Browsers fehlgeschlagen: {e}"),
    log_copy_failed: |e| format!("Kopieren fehlgeschlagen: {e}"),
    log_exit: |e| format!("Beendet mit Fehler: {e}"),
    log_panic: |info| format!("Absturz (Panic): {info}"),
    log_palette_start: "Palettenstart",
    log_side_start: "Seitenbereichs-Start",
    log_side_started: |pane, workspace| {
        format!("Seitenbereichs-Start: Bereich {pane}, Workspace {workspace}")
    },
    log_side_no_pane: "Seitenbereichs-Start: eigene Bereichs-ID unbekannt, nicht gespeichert",
    log_side_record: "Seitenbereichs-Eintrag",
    log_side_record_remove: "Entfernen des Seitenbereichs-Eintrags",
    settings_warning: |w| format!("Einstellungen: {w}"),
    status_updating: "Wird aktualisiert…",
    status_offline: "Offline",
    status_error: "Fehler",
    status_updated: |ago| format!("Aktualisiert {ago}"),
    search_placeholder: " Issues suchen · l:Label s:Status @Person #Team",
    search_prompt: " / Suchen",
    loading: "Wird geladen…",
    no_results: "Keine Ergebnisse",
    pinned_header: " Aktueller Branch",
    deep_search_row: "⏎ Auf dem Server suchen (mit Kommentaren)",
    priority_named: |p| format!("Priorität: {p}"),
    no_assignee: "Nicht zugewiesen",
    pr_open: |pr| format!("{pr} offen"),
    pr_draft: " (Entwurf)",
    project_named: |name| format!("Projekt {name}"),
    cycle_named: |name| format!("Zyklus {name}"),
    parent_named: |id| format!("Übergeordnet {id}"),
    estimate_named: |e| format!("Schätzung {e}"),
    due_named: |d| format!("Fällig am {d}"),
    no_body: "(keine Beschreibung)",
    comments_header: |n, more| format!("── Kommentare {n}{} ──", if more { "+" } else { "" }),
    unknown_user: "Unbekannt",
    more_comments_tui: "Ältere Kommentare im Browser ansehen (o)",
    comments_loading: "Kommentare werden geladen…",
    detail_gone_id: |id| format!("{id}: nicht gefunden, archiviert oder gelöscht"),
    detail_loading_id: |id| format!("{id} wird geladen…"),
    detail_failed_id: |id| format!("{id}: konnte nicht geladen werden. Mit r erneut versuchen"),
    detail_gone: "Dieses Issue wurde archiviert oder gelöscht",
    hints_search: " ⏎ Öffnen  Tab Ansicht  ↑↓ Bewegen  ^K Menü  Esc Listenmodus",
    hints_list: " j/k Bewegen  / Suchen  ⏎ Öffnen  y URL kopieren  Y PR-Link  ^K Menü  q Schließen",
    hints_detail: " j/k Blättern  t Beziehungen  u Links  y URL kopieren  Y PR-Link  ^K Menü  Esc Zurück  q Schließen",
    hints_onboarding: " ⏎ Bestätigen  Esc Schließen",
    offline_footer: |m| format!(" Offline, gespeicherte Daten werden angezeigt ({m})"),
    error_footer: |m| format!(" Fehler: {m}"),
    onboarding_title: " Linear verbinden ",
    env_key_invalid: "Der Schlüssel in LINEAR_API_KEY ist ungültig",
    env_key_fix: "Variable korrigieren oder entfernen, dann erneut öffnen.",
    esc_close: "Esc Schließen",
    paste_key: "Persönlichen Linear-API-Schlüssel einfügen.",
    checking: "Wird geprüft…",
    key_label: "Schlüssel: ",
    confirm_close: "⏎ Bestätigen · Esc Schließen",
    login_prompt: "Linear-API-Schlüssel (Linear → Settings → Security & access → Personal API keys): ",
    login_env_note: "\nHinweis: LINEAR_API_KEY ist gesetzt, daher hat dieser Wert Vorrang",
    key_empty: "Der Schlüssel ist leer",
    cache_open_warning: |e| {
        format!(
            "Warnung: Cache konnte nicht geöffnet werden, diesmal wird nichts gespeichert ({e})"
        )
    },
    logged_out: "API-Schlüssel und Cache gelöscht",
    whoami_offline: "Offline, daher können die Kontoinformationen nicht abgerufen werden",
    whoami_workspace: |name, key| format!("Workspace: {name} ({key})"),
    whoami_teams: |teams| format!("Teams: {teams}"),
    whoami_scope: |n| {
        if n == 1 {
            "Suchbereich: 1 Team".to_string()
        } else {
            format!("Suchbereich: {n} Teams")
        }
    },
    whoami_remaining: |r| format!("Verbleibende Anfragen: {r} (pro Stunde)"),
    scope_warning: |teams| {
        format!(
            "Warnung: Kein Team passt zu teams ({teams}) in config.toml, daher werden alle Teams durchsucht"
        )
    },
    offline_no_results_for_key: |m| {
        format!("Offline, und für diesen Schlüssel ist nichts gespeichert: {m}")
    },
    offline_saved_results: |ago, m| format!("Offline: gespeicherte Ergebnisse von {ago} · {m}"),
    offline_no_results: |m| format!("Offline, und es gibt keine gespeicherten Ergebnisse: {m}"),
    deep_needs_query: "Die Tiefensuche braucht einen Suchbegriff",
    offline_no_issues_for_key: |m| {
        format!("Offline, und für diesen Schlüssel sind keine Issues gespeichert: {m}")
    },
    offline_local_only: |m| format!("Offline: nur gespeicherte Issues durchsucht · {m}"),
    issue_gone: |id| format!("{id} wurde archiviert oder gelöscht"),
    issue_not_found: |id| {
        format!(
            "Issue {id} nicht gefunden (möglicherweise archiviert oder gelöscht, oder kein Zugriff)"
        )
    },
    offline_issue_not_saved_for_key: |id, m| {
        format!("Offline, und {id} ist für diesen Schlüssel nicht gespeichert: {m}")
    },
    offline_issue_not_saved: |id, m| format!("Offline, und {id} ist nicht gespeichert: {m}"),
    offline_saved_detail: |m| format!("Offline: gespeicherte Kopie · {m}"),
    more_comments_cli: "Es gibt weitere Kommentare. Im Browser ansehen",
    herdr_busy: "Ein anderes herdr-Fenster, etwa Einstellungen oder Kopiermodus, ist geöffnet. Schließen und erneut versuchen",
    herdr_failed_status: |s| format!("herdr-Befehl fehlgeschlagen ({s})"),
    herdr_failed: |e| format!("herdr-Befehl fehlgeschlagen: {e}"),
    palette_open_failed: |e| format!("Palette konnte nicht geöffnet werden: {e}"),
    side_toggle_failed: |e| {
        format!("Seitenbereich konnte nicht geöffnet oder geschlossen werden: {e}")
    },
    unknown_workspace: "Workspace konnte nicht ermittelt werden",
    herdr_missing_field: |p| format!("{p} fehlt in der herdr-Antwort"),
    language_in_section: |s| {
        format!("language an den Anfang von config.toml verschieben (steht in [{s}])")
    },
    unknown_key: |k| format!("Unbekannter Eintrag {k} in config.toml wird ignoriert"),
};

/// "1 Minute", "5 Minuten"
fn minuten(n: i64) -> String {
    if n == 1 {
        "1 Minute".to_string()
    } else {
        format!("{n} Minuten")
    }
}
