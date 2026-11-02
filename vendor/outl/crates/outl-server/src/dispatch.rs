//! HTTP dispatch over the shared command bodies.
//!
//! The frontend calls `invoke(cmd, args)` with camelCase keys (Tauri
//! auto-converts to snake_case for Rust). This module takes the JSON args,
//! extracts each parameter by its snake_case name, and calls the same
//! `outl_tauri_shared::commands::*` body the desktop app uses — passing
//! `&state` (an [`AppHost`]) in front, exactly as the Tauri wrappers do.
//!
//! The result is serialized to JSON and returned. A `Result<_, String>`
//! body's `Err` becomes an HTTP 400 with the message; a non-`Result` body
//! is returned as-is.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;

use outl_tauri_shared::commands as c;
use outl_tauri_shared::AppHost;

use crate::state::AppState;

/// Convert a camelCase JSON key to the snake_case Rust parameter name.
fn snake(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for (i, c) in key.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Re-key a camelCase args object to snake_case.
fn rekey(args: &Value) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    if let Some(obj) = args.as_object() {
        for (k, v) in obj {
            out.insert(snake(k), v.clone());
        }
    }
    out
}

fn s(args: &HashMap<String, Value>, key: &str) -> String {
    args.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string()
}

fn os(args: &HashMap<String, Value>, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn u32(args: &HashMap<String, Value>, key: &str) -> u32 {
    args.get(key).and_then(|v| v.as_u64()).unwrap_or(0) as u32
}

fn usize(args: &HashMap<String, Value>, key: &str) -> usize {
    args.get(key).and_then(|v| v.as_u64()).unwrap_or(0) as usize
}

fn osize(args: &HashMap<String, Value>, key: &str) -> Option<usize> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as usize)
}

fn b(args: &HashMap<String, Value>, key: &str) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn vs(args: &HashMap<String, Value>, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default()
}

fn ok<T: serde::Serialize>(value: T) -> Response {
    match serde_json::to_value(value) {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("serialize: {e}") })),
        )
            .into_response(),
    }
}

fn err(msg: String) -> Response {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": msg }))).into_response()
}

fn result<T: serde::Serialize>(r: Result<T, String>) -> Response {
    match r {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

/// The single HTTP entry point: `POST /api/invoke/<cmd>` with a JSON args
/// body. Dispatches to the shared command body and serializes the result.
pub async fn invoke(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(cmd): axum::extract::Path<String>,
    Json(args): Json<Value>,
) -> Response {
    let args = rekey(&args);
    let st = &*state;
    match cmd.as_str() {
        // ---- Workspace lifecycle ----
        "current_workspace" => ok(st
            .storage_root
            .lock()
            .clone()
            .map(|p| p.to_string_lossy().into_owned())),
        "workspace_stats" => ok(workspace_stats(st)),
        "reload_workspace" => {
            let root = st.storage_root().ok();
            match outl_tauri_shared::workspace_reload::reload_workspace_into(st).await {
                Ok(()) => {
                    if let Some(root) = root {
                        let events = st.events.clone();
                        outl_tauri_shared::reconcile::spawn_background_reconcile(
                            st.workspace.clone(),
                            root,
                            st.hlc.clone(),
                            move || events.emit("workspace-reconciled"),
                        );
                    }
                    ok(())
                }
                Err(e) => err(e),
            }
        }
        "set_workspace" => ok(()),
        "get_settings" => ok(st.settings.lock().clone()),
        "update_settings" => {
            let next_value = match args.get("next") {
                Some(v) if v.is_object() => v.clone(),
                _ => return err("update_settings: `next` must be a settings object".into()),
            };
            let next: crate::settings::Settings = match serde_json::from_value(next_value) {
                Ok(s) => s,
                Err(e) => return err(format!("update_settings: invalid settings: {e}")),
            };
            match crate::settings::save(&next) {
                Ok(()) => {
                    *st.settings.lock() = next.clone();
                    ok(next)
                }
                Err(e) => err(format!("save settings: {e}")),
            }
        }
        "get_telemetry_enabled" => ok(c::telemetry::get_telemetry_enabled()),
        "set_telemetry_enabled" => result(c::telemetry::set_telemetry_enabled(b(&args, "enabled"))),

        // ---- Theme ----
        "list_themes" => ok(c::theme::list_themes()),
        "get_theme" => ok(c::theme::get_theme(os(&args, "name"))),
        "get_theme_config" => ok(c::theme::get_theme_config()),

        // ---- Shortcuts ----
        "list_shortcut_bindings" => ok(c::shortcuts::list_shortcut_bindings()),
        "list_action_support" => ok(c::shortcuts::list_action_support()),

        // ---- Page / journal navigation ----
        "list_all_pages" => result(c::page::list_all_pages(st)),
        "search_pages" => result(c::page::search_pages(st, s(&args, "query"))),
        "search_persons" => result(c::page::search_persons(st, s(&args, "query"))),
        "search_blocks" => result(c::page::search_blocks(st, s(&args, "query")).await),
        "outl_emoji_search" => result(c::page::emoji_search(s(&args, "query"), usize(&args, "limit"))),
        "open_today_journal" => result(c::page_open::open_today_journal(st).await),
        "open_journal_for" => result(c::page_open::open_journal_for(st, s(&args, "slug")).await),
        "open_page_by_slug" => result(c::page_open::open_page_by_slug(st, s(&args, "slug")).await),
        "open_ref" => result(open_ref(st, s(&args, "target"))),
        "open_external_file" => result(c::open_with::open_external_file(st, s(&args, "source_path")).await),
        "take_pending_deep_link" => ok(None::<Value>),
        "take_pending_open_file" => ok(None::<String>),
        "previous_day" => result(c::page::previous_day(s(&args, "slug"))),
        "next_day" => result(c::page::next_day(s(&args, "slug"))),
        "today_slug_cmd" => ok(c::page::today_slug()),
        "date_title" => result(c::page::date_title(s(&args, "slug"))),
        "resolve_ref" => result(c::page::resolve_ref(st, s(&args, "target"))),
        "delete_page" => result(c::page::delete_page(st, s(&args, "slug")).await),
        "page_backlinks" => result(c::page::page_backlinks(st, s(&args, "slug")).await),
        "set_backlinks_order" => result(c::page::set_backlinks_order(st, s(&args, "order"), s(&args, "slug")).await),
        "resolve_page_labels" => result(c::page::resolve_page_labels(st, vs(&args, "node_ids"))),
        "toggle_pin" => result(c::page::toggle_pin(st, s(&args, "page_id"))),

        // ---- Templates ----
        "list_templates_cmd" => result(c::template::list_templates(st)),
        "instantiate_template_at" => result(c::template::instantiate_template_at(st, s(&args, "name"), s(&args, "target_block"))),

        // ---- Block mutations ----
        "create_block" => result(c::block::create_block(
            st,
            s(&args, "page_id"),
            os(&args, "after_id"),
            os(&args, "before_id"),
            os(&args, "parent_id"),
            os(&args, "text"),
        )),
        "edit_block" => result(c::block::edit_block(st, s(&args, "page_id"), s(&args, "id"), s(&args, "text"))),
        "split_block" => result(c::block::split_block(st, s(&args, "page_id"), s(&args, "id"), u32(&args, "char_offset"))),
        "toggle_todo" => result(c::block::toggle_todo(st, s(&args, "page_id"), s(&args, "id"))),
        "toggle_quote" => result(c::block::toggle_quote(st, s(&args, "page_id"), s(&args, "id"))),
        "delete_block" => result(c::block::delete_block(st, s(&args, "page_id"), s(&args, "id"))),
        "indent_block" => result(c::block::indent_block(st, s(&args, "page_id"), s(&args, "id"))),
        "outdent_block" => result(c::block::outdent_block(st, s(&args, "page_id"), s(&args, "id"))),
        "move_block_up" => result(c::block::move_block_up(st, s(&args, "page_id"), s(&args, "id"))),
        "move_block_down" => result(c::block::move_block_down(st, s(&args, "page_id"), s(&args, "id"))),
        "move_block_after" => result(c::block::move_block_after(st, s(&args, "page_id"), s(&args, "id"), s(&args, "after_id"))),
        "copy_block_markdown" => result(c::block::copy_block_markdown(st, s(&args, "id"))),
        "paste_block_after" => result(c::block::paste_block_after(st, s(&args, "page_id"), s(&args, "after_id"), s(&args, "text"))),
        "cut_block" => result(c::block::cut_block(st, s(&args, "page_id"), s(&args, "id"))),
        "set_block_collapsed" => result(c::block::set_block_collapsed(st, s(&args, "page_id"), s(&args, "id"), b(&args, "collapsed"))),
        "paste_markdown_at" => result(c::block::paste_markdown_at(st, s(&args, "page_id"), s(&args, "block_id"), u32(&args, "caret"), s(&args, "text"))),
        "paste_plain_at" => result(c::block::paste_plain_at(st, s(&args, "page_id"), s(&args, "block_id"), u32(&args, "caret"), s(&args, "text"))),
        "copy_markdown" => result(c::block::copy_markdown(st, vs(&args, "block_ids"))),
        "copy_block_ref" => result(c::block::copy_block_ref(st, s(&args, "id")).await),

        // ---- Undo / redo ----
        "undo_page" => result(c::history::undo_page(st, s(&args, "page_id"))),
        "redo_page" => result(c::history::redo_page(st, s(&args, "page_id"))),

        // ---- Code execution ----
        "run_code_block" => result(c::exec::run_code_block(st, s(&args, "page_id"), s(&args, "block_id")).await),
        "run_auto_run_blocks" => result(c::exec::run_auto_run_blocks(st, s(&args, "page_id")).await),
        "resolve_embeds" => result(c::exec::resolve_embeds(st, vs(&args, "handles")).await),

        // ---- Assets ----
        "open_asset" => result(c::asset::open_asset(st, s(&args, "url"))),
        "read_asset_data_url" => result(c::asset::read_asset_data_url(st, s(&args, "url")).await),
        "import_asset_file" => result(c::asset::import_asset_file(st, s(&args, "source_path")).await),
        "attach_asset" => result(c::asset::attach_asset(st, s(&args, "source_path"), s(&args, "page_id"), os(&args, "after_block_id"))),

        // ---- Properties ----
        "known_property_keys" => result(c::property::known_property_keys(st)),
        "set_page_property" => result(c::reminders::set_page_property(st, s(&args, "page_id"), s(&args, "key"), s(&args, "value"))),

        // ---- Reminders ----
        "list_reminders" => result(c::reminders::list_reminders(st)),
        "reminder_settings" => ok(c::reminders::reminder_settings()),
        "set_reminder_settings" => result(c::reminders::set_reminder_settings(b(&args, "enabled"), s(&args, "quiet_hours"))),
        "snooze_presets" => ok(c::reminders::snooze_presets()),
        "snooze_reminder" => result(c::reminders::snooze_reminder(st, s(&args, "block_id"), s(&args, "preset"))),
        "clear_reminder_snooze" => result(c::reminders::clear_reminder_snooze(st, s(&args, "block_id"))),
        "set_block_property" => result(c::reminders::set_block_property(st, s(&args, "page_id"), s(&args, "block_id"), s(&args, "key"), s(&args, "value"))),
        "mark_block_done" => result(c::reminders::mark_block_done(st, s(&args, "page_id"), s(&args, "block_id"))),
        "set_block_remind" => result(c::reminders::set_block_remind(st, s(&args, "page_id"), s(&args, "block_id"), s(&args, "rule"))),
        "deliver_due_reminders" => {
            let due = outl_tauri_shared::reminder_runtime::take_due(st);
            ok(due)
        }

        // ---- Page history / trust ----
        "page_timeline" => result(c::timeline::page_timeline(st, s(&args, "page_id"), osize(&args, "limit")).await),
        "device_activity" => result(c::trust::device_activity(st).await),
        "export_markdown" => result(c::trust::export_markdown(st, os(&args, "dest_dir")).await),
        "merged_edits" => result(outl_tauri_shared::merges::merged_edits(st)),
        "undo_merged_edit" => result(outl_tauri_shared::merges::undo_merged_edit(st, s(&args, "id"))),
        "drain_capture_inbox" => result(outl_tauri_shared::capture_inbox::drain_capture_inbox(st)),

        // ---- Overview (tasks / journal days) ----
        "list_open_tasks" => result(c::tasks::list_open_tasks(st).await),
        "journal_days_with_content" => result(c::journal_days::journal_days_with_content(st, s(&args, "from"), s(&args, "to")).await),

        // ---- Peers / sync ----
        "outl_peer_list" => result(c::peers::peer_list(st).await),
        "outl_peer_remove" => result(c::peers::peer_remove(st, s(&args, "id")).await),
        "outl_peer_status" => result(c::peers::peer_status(st).await),
        "outl_sync_now" => result(c::peers::sync_now(st).await),
        "outl_peer_pair_host" => err("pairing is done via the CLI (`outl peer pair`); the addon is an always-on peer".into()),
        "outl_peer_pair_join" => err("pairing is done via the CLI (`outl peer pair`); the addon is an always-on peer".into()),

        // ---- Plugins ----
        "plugin_list" => ok(st.plugins.list_commands()),
        "plugin_keybindings" => ok(st.plugins.list_keybindings()),
        "plugin_toolbar" => ok(st.plugins.list_toolbar()),
        "plugin_transformers" => ok(st.plugins.list_transformers()),
        "plugin_transform" => result(st.plugins.transform_block(s(&args, "plugin_id"), s(&args, "lang"), s(&args, "input"))),
        "plugin_run" => result(c::plugin::run(st, &st.plugins, s(&args, "plugin_id"), s(&args, "command_id"), os(&args, "page_id"))),
        "plugin_sync_hooks" => result(c::plugin::sync_hooks(st, &st.plugins, os(&args, "page_id"))),
        "plugin_registry_list" => result(st.plugins.registry_list()),
        "plugin_install_official" => result(st.plugins.install_official(s(&args, "id"))),
        "plugin_set_enabled" => result(st.plugins.set_enabled(s(&args, "id"), b(&args, "enabled"))),
        "plugin_uninstall" => result(st.plugins.uninstall(s(&args, "id"))),
        "plugin_settings_describe" => result(c::plugin::settings_describe(st, s(&args, "plugin_id"))),
        "plugin_config_set" => result(c::plugin::config_set(st, &st.plugins, s(&args, "plugin_id"), s(&args, "key"), s(&args, "value"))),
        "plugin_secret_set" => result(c::plugin::secret_set(s(&args, "plugin_id"), s(&args, "key"), s(&args, "value"))),
        "plugin_secret_remove" => result(c::plugin::secret_remove(s(&args, "plugin_id"), s(&args, "key"))),

        _ => err(format!("no handler for {cmd}")),
    }
}

/// `workspace_stats` body, inlined from the desktop (it reads the state
/// fields directly rather than going through a shared body).
fn workspace_stats(st: &AppState) -> outl_tauri_shared::WorkspaceSummary {
    let guard = st.workspace.lock();
    let storage_root = st
        .storage_root
        .lock()
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    match guard.as_ref() {
        Some(ws) => outl_tauri_shared::WorkspaceSummary {
            blocks: ws.tree().node_count(),
            ops: ws.log().len(),
            actor: ws.actor.to_string(),
            storage_root,
            ready: true,
            ops_complete: !ws.booted_from_snapshot(),
        },
        None => outl_tauri_shared::WorkspaceSummary {
            blocks: 0,
            ops: 0,
            actor: st.hlc.actor().to_string(),
            storage_root,
            ready: false,
            ops_complete: true,
        },
    }
}

/// `open_ref` body, reimplemented without a Tauri `AppHandle`. The desktop
/// uses the handle only to emit a non-critical `ref-projection-failed`
/// toast; here that is dropped (the op log already has the mutation).
fn open_ref<S: outl_tauri_shared::AppHost>(
    state: &S,
    target: String,
) -> Result<outl_tauri_shared::PageView, String> {
    use outl_tauri_shared::helpers::{build_page_view, reproject_stale_md, with_ws, with_ws_mut};
    let root = state.storage_root()?;
    let id = with_ws_mut(state, |ws| {
        outl_actions::open_or_create_by_ref(ws, state.hlc(), &target).map_err(|e| e.to_string())
    })?;
    let outcome = with_ws_mut(state, |ws| {
        Ok(reproject_stale_md(ws, &root, id, &format!("open_ref {target}")))
    })?;
    let _ = &outcome.other_error;
    with_ws(state, |ws| {
        let view = build_page_view(ws, &root, id).map_err(|e| e.to_string())?;
        Ok(view.with_ahead_of_log_check(outcome.ahead_of_log))
    })
}
