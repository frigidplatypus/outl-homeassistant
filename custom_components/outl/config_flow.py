"""Config flow: the workspace lives under HA's config dir; the bridge is local."""

from __future__ import annotations

import pathlib
from typing import Any

import voluptuous as vol
from homeassistant.config_entries import ConfigFlow, ConfigFlowResult
from homeassistant.helpers import config_validation as cv

from .const import (
    CONF_MCP_URL,
    CONF_WORKSPACE_PATH,
    DEFAULT_MCP_URL,
    DEFAULT_WORKSPACE_PATH,
    DOMAIN,
)

STEP_USER_DATA_SCHEMA = vol.Schema(
    {
        vol.Required(CONF_WORKSPACE_PATH, default=DEFAULT_WORKSPACE_PATH): cv.string,
        vol.Optional(CONF_MCP_URL, default=DEFAULT_MCP_URL): cv.string,
    }
)


class OutlConfigFlow(ConfigFlow, domain=DOMAIN):
    """One entry per workspace; the id is the workspace path."""

    VERSION = 1

    async def async_step_user(
        self, user_input: dict[str, Any] | None = None
    ) -> ConfigFlowResult:
        errors: dict[str, str] = {}
        if user_input is not None:
            root = pathlib.Path(user_input[CONF_WORKSPACE_PATH])
            if not (root / "pages").is_dir() and not (root / "journals").is_dir():
                errors["base"] = "no_workspace"
            else:
                await self.async_set_unique_id(str(root))
                self._abort_if_unique_id_configured()
                return self.async_create_entry(
                    title=f"outl ({root.name})", data=user_input
                )
        return self.async_show_form(
            step_id="user", data_schema=STEP_USER_DATA_SCHEMA, errors=errors
        )
