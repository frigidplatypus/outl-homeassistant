"""outl: local-first outliner tasks as a Home Assistant To-do list."""

from __future__ import annotations

import logging

from homeassistant.config_entries import ConfigEntry
from homeassistant.core import HomeAssistant

from .const import CONF_MCP_URL, CONF_WORKSPACE_PATH, DEFAULT_MCP_URL, DEFAULT_WORKSPACE_PATH
from .coordinator import OutlCoordinator

_LOGGER = logging.getLogger(__name__)

PLATFORMS = ["todo"]

type OutlConfigEntry = ConfigEntry[OutlCoordinator]


async def async_setup_entry(hass: HomeAssistant, entry: OutlConfigEntry) -> bool:
    workspace = entry.data.get(CONF_WORKSPACE_PATH, DEFAULT_WORKSPACE_PATH)
    mcp_url = entry.data.get(CONF_MCP_URL, DEFAULT_MCP_URL) or None
    coordinator = OutlCoordinator(hass, workspace, mcp_url)
    await coordinator.async_config_entry_first_refresh()
    entry.async_on_unload(coordinator.async_shutdown)
    entry.runtime_data = coordinator

    await hass.config_entries.async_forward_entry_setups(entry, PLATFORMS)
    return True


async def async_unload_entry(hass: HomeAssistant, entry: OutlConfigEntry) -> bool:
    return await hass.config_entries.async_unload_platforms(entry, PLATFORMS)
