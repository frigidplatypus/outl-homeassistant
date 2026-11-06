"""To-do list platform: the workspace's tasks, as an HA To-do list."""

from __future__ import annotations

import datetime
import logging

from homeassistant.components.todo import (
    TodoItem,
    TodoItemStatus,
    TodoListEntity,
    TodoListEntityFeature,
)

from .const import DOMAIN
from .coordinator import OutlCoordinator
from .outl_tasks import STATUS_DOING, STATUS_OPEN, TaskRow

_LOGGER = logging.getLogger(__name__)

# HA's stable status enum is two-valued; DOING is a more accurate
# needs_action, and the server keeps the distinction on disk regardless.
_STATUS = {
    STATUS_OPEN: TodoItemStatus.NEEDS_ACTION,
    STATUS_DOING: TodoItemStatus.NEEDS_ACTION,
    "completed": TodoItemStatus.COMPLETED,
}


def _to_item(row: TaskRow) -> TodoItem:
    due = None
    if row.due:
        try:
            due = datetime.date.fromisoformat(row.due)
        except ValueError:
            due = None
    return TodoItem(
        summary=row.summary,
        uid=row.uid,
        status=_STATUS.get(row.status, TodoItemStatus.NEEDS_ACTION),
        due=due,
    )


async def async_setup_entry(hass, entry, async_add_entities):
    coordinator: OutlCoordinator = entry.runtime_data
    async_add_entities([OutlTodoEntity(coordinator)])


class OutlTodoEntity(TodoListEntity):
    """One list per workspace -- the GUI's task list, inside HA."""

    _attr_name = "outl tasks"
    _attr_has_entity_name = False

    def __init__(self, coordinator: OutlCoordinator) -> None:
        self._coordinator = coordinator
        self._attr_unique_id = f"{DOMAIN}_{coordinator.root}"

    @property
    def todo_list_features(self) -> TodoListEntityFeature:
        return (
            TodoListEntityFeature.CREATE_TODO_ITEM
            | TodoListEntityFeature.UPDATE_TODO_ITEM
            | TodoListEntityFeature.DELETE_TODO_ITEM
            | TodoListEntityFeature.SET_DUE_DATE_ON_ITEM
        )

    @property
    def todo_items(self) -> list[TodoItem]:
        return [_to_item(row) for row in (self._coordinator.data or {}).get("tasks", ())]

    async def async_create_todo_item(self, item: TodoItem) -> None:
        if isinstance(item.due, datetime.datetime):
            due = item.due.date().isoformat()
        elif isinstance(item.due, datetime.date):
            due = item.due.isoformat()
        else:
            due = None
        if not await self._coordinator.async_create(item.summary or "", due):
            raise ValueError("outl rejected the new task")
        await self._coordinator.async_request_refresh()

    async def async_update_todo_item(self, item: TodoItem) -> None:
        if not item.uid:
            raise ValueError("outl tasks need a uid to update")
        status = (
            "completed"
            if item.status == TodoItemStatus.COMPLETED
            else "needs_action"
        )
        if not await self._coordinator.async_set_status(item.uid, status):
            await self._coordinator.async_request_refresh()
            raise ValueError(f"outl task {item.uid} moved; retry against the new state")

    async def async_delete_todo_items(self, uids: list[str]) -> None:
        for uid in uids:
            if not await self._coordinator.async_delete(uid):
                raise ValueError(f"outl task {uid} moved; retry against the new state")
        await self._coordinator.async_request_refresh()
