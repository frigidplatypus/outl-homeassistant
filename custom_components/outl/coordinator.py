"""Coordinator: scans the outl workspace and owns task write-back."""

from __future__ import annotations

import logging
from datetime import date, timedelta
from pathlib import Path

from homeassistant.core import HomeAssistant
from homeassistant.helpers.update_coordinator import DataUpdateCoordinator

from .const import SCAN_INTERVAL_SECONDS
from .outl_tasks import (
    McpError,
    TaskRow,
    append_journal_task,
    collect_tasks,
    mcp_append_journal_task,
    mcp_call,
    mcp_set_todo_status,
    rewrite_task_line,
    strip_marker,
    task_text,
)

_LOGGER = logging.getLogger(__name__)


class OutlCoordinator(DataUpdateCoordinator[dict]):
    """Reads tasks from the workspace files; writes through the MCP server.

    Reads are free -- the files sit in HA's own config dir and the addon
    keeps them projected -- so the poll interval is cheap. Writes prefer the
    MCP bridge: the server owns the task grammar and logs every change as an
    op, which is what keeps GUI and HA converging. The .md compare-and-swap
    is the fallback for when the addon (and with it the bridge) is down.
    """

    def __init__(self, hass: HomeAssistant, workspace_path: str, mcp_url: str | None) -> None:
        super().__init__(
            hass,
            _LOGGER,
            name="outl",
            update_interval=timedelta(seconds=SCAN_INTERVAL_SECONDS),
        )
        self.root = Path(workspace_path)
        self.mcp_url = mcp_url

    async def _async_update_data(self) -> dict:
        result = await self.hass.async_add_executor_job(collect_tasks, self.root)
        return {"tasks": result["tasks"], "total": result["total"]}

    def _row(self, uid: str) -> TaskRow | None:
        """The newest read of one task; writes CAS against it, never a stale copy."""
        for task in (self.data or {}).get("tasks", ()):
            if task.uid == uid:
                return task
        return None

    async def async_set_status(self, uid: str, status: str) -> bool:
        """Move a task to HA ``status``. Returns whether it landed."""
        row = self._row(uid)
        if row is None:
            return False
        if self.mcp_url and row.uid_stable:
            try:
                await self.hass.async_add_executor_job(
                    mcp_set_todo_status, self.mcp_url, row.uid, status
                )
                return True
            except McpError as err:
                _LOGGER.debug("mcp status write failed, falling back to file: %s", err)
        ok = await self.hass.async_add_executor_job(
            rewrite_task_line,
            Path(row.path),
            row.line_no,
            row.text,
            task_text(row.text, status),
        )
        if ok:
            await self.async_refresh()
        return ok

    async def async_create(self, summary: str, due: str | None) -> bool:
        text = task_text(summary, "needs_action")
        if due:
            text = f"{text} due:: {due}"
        if self.mcp_url:
            try:
                await self.hass.async_add_executor_job(
                    mcp_append_journal_task, self.mcp_url, text
                )
                return True
            except McpError as err:
                _LOGGER.debug("mcp create failed, falling back to file: %s", err)
        ok = await self.hass.async_add_executor_job(
            append_journal_task, self.root, date.today().isoformat(), text
        )
        await self.async_refresh()
        return ok is not None

    async def async_delete(self, uid: str) -> bool:
        """Trash the block through the server; outl blocks are not deleted, only marked."""
        row = self._row(uid)
        if row is None:
            return False
        if self.mcp_url and row.uid_stable:
            try:
                await self.hass.async_add_executor_job(
                    mcp_call, self.mcp_url, "outl_block_delete", {"id": row.uid, "confirm": True}
                )
                return True
            except McpError as err:
                _LOGGER.debug("mcp delete failed, falling back to file: %s", err)
        # File fallback: un-mark the task (the text stays as a plain block);
        # HA "deleted" and outl "still there, un-tasked" disagree on
        # purpose, but the block -- the user's words -- survives either way.
        ok = await self.hass.async_add_executor_job(
            rewrite_task_line, Path(row.path), row.line_no, row.text, _body_of(row.text)
        )
        if ok:
            await self.async_refresh()
        return ok


def _body_of(text: str) -> str:
    split = strip_marker(text)
    return split[1] if split else text
