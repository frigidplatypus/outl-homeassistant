"""Pure outl task grammar: parse workspace markdown, build HA task rows, and
write status changes back as canonical block text.

No Home Assistant imports: every function here runs under plain Python so
the grammar can be tested against real outl fixtures without the HA test
harness. outl is the source of truth for the vocabulary:

- task markers are the word prefixes ``TODO ``, ``DOING ``, ``DONE `` and the
  checkbox forms ``[ ] ``, ``[/] ``, ``[x] ``/``[X] `` (``outl_md::TASK_PREFIXES``);
- a ``.md`` and its ``.outl`` sidecar are written together from one tree
  snapshot, sidecar blocks in depth-first preorder == outline order in the
  file, so block N of the parsed outline is sidecar ``blocks[N]``;
- ``ref_handle`` (``blk-xxxxxx``) is the stable human handle for a block id:
  editing text does not change it, so it is the right uid for an HA entity.

Writes rewrite the block line in place (strip the old marker, apply the new
one) and leave everything else on the line -- properties, quotes, children
-- untouched. The server's ops watcher reconciles the edit into the op log.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

# (marker, finished?) -- mirrors outl_md::outline_ops::TASK_PREFIXES exactly.
TASK_MARKERS: tuple[tuple[str, bool], ...] = (
    ("TODO ", False),
    ("DOING ", False),
    ("DONE ", True),
    ("[ ] ", False),
    ("[/] ", False),
    ("[x] ", True),
    ("[X] ", True),
)

# HA TodoListStatus strings used for the status field of a row.
STATUS_OPEN = "needs_action"
STATUS_DOING = "in_progress"
STATUS_DONE = "completed"

# Canonical write markers per HA status. A word prefix, never a checkbox:
# the checkbox forms are read-compatible aliases, the words are canonical.
WRITE_MARKER: dict[str, str] = {
    STATUS_OPEN: "TODO ",
    STATUS_DOING: "DOING ",
    STATUS_DONE: "DONE ",
}

_LIST_ITEM = re.compile(r"^(\s*)-\s+(.*)$")
_TITLE_PROP = re.compile(r"^title::\s+")
# `due:: YYYY-MM-DD` anywhere on the block (page/block property syntax).
_DUE_PROP = re.compile(r"\bdue::\s*(\d{4}-\d{2}-\d{2})\b")


@dataclass(frozen=True)
class TaskRow:
    """One open-or-done task: the integration's whole view of a block."""

    uid: str  # sidecar ref_handle when known, else "line:<n>"
    uid_stable: bool  # False when no sidecar could vouch for the id
    text: str  # the block's raw text as it sits in the .md, CAS anchor
    summary: str  # body with marker and due property stripped
    status: str  # STATUS_OPEN / STATUS_DOING / STATUS_DONE
    due: str | None  # ISO date, extracted from the due:: property
    page: str  # slug (file stem) -- journals are ISO-date slugs
    path: str  # md file, for write-back
    line_no: int  # physical 0-based line index of the block in path
    indent: int


@dataclass
class ParsedPage:
    slug: str
    path: Path
    tasks: list[TaskRow] = field(default_factory=list)


def strip_marker(text: str) -> tuple[str, str] | None:
    """Split ``text`` into (marker, body) when it starts with a task marker."""
    for marker, _finished in TASK_MARKERS:
        if text.startswith(marker):
            return marker, text[len(marker) :]
    return None


def marker_finished(marker: str) -> bool:
    for m, finished in TASK_MARKERS:
        if m == marker:
            return finished
    raise ValueError(f"not a task marker: {marker!r}")


def status_of(marker: str) -> str:
    if marker == "DOING ":
        return STATUS_DOING
    return STATUS_DONE if marker_finished(marker) else STATUS_OPEN


def task_text(body: str, status: str) -> str:
    """Canonical outl block text for ``body`` at HA ``status``.

    Idempotent: any task marker (word or checkbox) on ``body`` is replaced,
    so a stale read whose marker moved while HA thought otherwise still lands
    on the intended status.
    """
    stripped = body
    split = strip_marker(body)
    if split is not None:
        stripped = split[1]
    return WRITE_MARKER[status] + stripped


def _extract_due(body: str) -> tuple[str, str | None]:
    m = _DUE_PROP.search(body)
    if not m:
        return body, None
    due = m.group(1)
    rest = (body[: m.start()] + body[m.end() :]).strip()
    # Collapse the double space left where the property sat between words.
    return re.sub(r"\s{2,}", " ", rest), due


def parse_outline_line(raw: str) -> tuple[int, str] | None:
    """Outline indent level and text for one markdown line, or None.

    ``indent`` counts two-space outline columns, matching what the sidecar
    reports for the same line.
    """
    m = _LIST_ITEM.match(raw)
    if not m:
        return None
    leading, text = m.group(1), m.group(2)
    # Outlines nest by two spaces per level; tabs are one level each.
    indent = (len(leading.expandtabs(2)) + 1) // 2
    return indent, text


def parse_page(path: Path) -> ParsedPage:
    """Parse a ``.md`` (+ sibling ``.outl``) into task rows.

    Reads are tolerant by design: an unreadable sidecar, or a block the
    sidecar has not caught up to, degrades to a line-index uid whose
    writes the integration refuses, rather than guessing an identity.
    """
    page = ParsedPage(slug=path.stem, path=path)
    try:
        md_lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return page

    sidecar = _sidecar_ids(path)
    outline: list[tuple[int, str, int]] = []  # (indent, text, line index)
    for i, raw in enumerate(md_lines):
        if _TITLE_PROP.match(raw.strip()):
            continue  # page property line, never a task row
        parsed = parse_outline_line(raw)
        if parsed is not None:
            outline.append((parsed[0], parsed[1], i))

    # Pair preorder-to-preorder: the k-th block with a given text in the
    # file is the k-th sidecar entry with that text. Sidecar `line` is
    # outline-relative and breaks once an external append lands before a
    # sweep, so identity rides on the text, and only a text the sidecar
    # has not caught up to degrades to an unstable line-index uid.
    seen: dict[str, int] = {}
    for indent, text, line_no in outline:
        split = strip_marker(text)
        if split is None:
            continue
        marker, body = split
        body, due = _extract_due(body)
        # Quoted display titles: 'Water plants' reads as Water plants.
        summary = body.strip().strip("'\"").strip()
        nth = seen.get(text, 0)
        seen[text] = nth + 1
        ids = sidecar.get(text)
        uid = ids[nth] if ids and nth < len(ids) else None
        page.tasks.append(
            TaskRow(
                uid=uid or f"line:{line_no}",
                uid_stable=uid is not None,
                text=text,
                summary=summary,
                status=status_of(marker),
                due=due,
                page=page.slug,
                path=str(path),
                line_no=line_no,
                indent=indent,
            )
        )
    return page


def _sidecar_ids(md_path: Path) -> dict[str, list[str]]:
    """Sidecar block ids grouped by block text, in preorder.

    Empty means "no usable sidecar": every row off that page gets an
    unstable uid. The id is what MCP block tools take as their ``id``
    argument and survives text edits -- which is why it, not the line
    number, is the uid HA hands back on an update. Keyed by text because
    the sidecar's ``line`` is an outline ordinal, not a physical row.
    """
    sidecar = md_path.with_suffix(".outl")
    try:
        data = json.loads(sidecar.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    blocks = data.get("blocks")
    if not isinstance(blocks, list):
        return {}
    out: dict[str, list[str]] = {}
    for b in blocks:
        if isinstance(b, dict) and b.get("id") and isinstance(b.get("text"), str):
            out.setdefault(b["text"], []).append(str(b["id"]))
    return out


def workspace_pages(root: Path) -> list[Path]:
    """Every task-bearing markdown in the workspace: pages then journals."""
    pages_dir = root / "pages"
    journals = root / "journals"
    files: list[Path] = []
    if pages_dir.is_dir():
        files.extend(sorted(pages_dir.glob("*.md")))
    if journals.is_dir():
        files.extend(sorted(journals.glob("*.md")))
    return files


def collect_tasks(root: Path, limit: int = 100) -> dict[str, Any]:
    """The workspace's task list: open-first, journals newest-day first.

    Mirrors the server's list_open_tasks ranking (started before untouched,
    newest journal day before named pages alphabetical, outline order within
    a page) so the HA list and the GUI's task list agree.
    """
    rows: list[TaskRow] = []
    for path in workspace_pages(root):
        rows.extend(parse_page(path).tasks)
    total = len(rows)
    journal = lambda slug: re.fullmatch(r"\d{4}-\d{2}-\d{2}", slug) is not None
    rank = {STATUS_DOING: 0, STATUS_OPEN: 1, STATUS_DONE: 2}
    rows.sort(
        key=lambda r: (
            rank[r.status],
            0 if journal(r.page) else 1,
            # Journals newest-first (descending slug); pages alphabetical.
            _reverse_str(r.page) if journal(r.page) else r.page,
            r.line_no,
        )
    )
    return {"total": total, "tasks": rows[:limit]}


def _reverse_str(s: str) -> str:
    """Descending sort key for equal-length ISO-date slugs."""
    return "".join(chr(0x10FFFF - ord(c)) for c in s)


def rewrite_task_line(
    path: Path, line_no: int, expect_text: str, new_text: str
) -> bool:
    """Replace one outline line when it still reads ``expect_text``.

    Compare-and-swap on the block's own text: a line that moved (the file
    was rewritten between the scan and this write) is left alone and the
    caller re-scans. Written atomically so the server's watcher never sees
    a torn file.
    """
    try:
        lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    except OSError:
        return False
    if line_no >= len(lines):
        return False
    parsed = parse_outline_line(lines[line_no].rstrip("\n"))
    if parsed is None or parsed[1] != expect_text:
        return False
    rendered = re.sub(r"^(\s*)-\s+", r"\1- ", lines[line_no].rstrip("\n"))
    body = rendered.split("- ", 1)[1]
    new_line = rendered.replace(body, new_text, 1)
    _atomic_write(path, "".join(lines[:line_no] + [new_line + "\n"] + lines[line_no + 1 :]))
    return True


def append_journal_task(root: Path, slug: str, text: str) -> str | None:
    """Append a task to a dated journal, scaffolding the file if absent."""
    target = root / "journals" / f"{slug}.md"
    try:
        if target.exists():
            content = target.read_text(encoding="utf-8")
            suffix = "" if content.endswith("\n") else "\n"
            _atomic_write(target, content + f"{suffix}- {text}\n")
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            _atomic_write(target, f"- {text}\n")
    except OSError:
        return None
    return str(target)


def _atomic_write(path: Path, content: str) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp-ha")
    tmp.write_text(content, encoding="utf-8")
    tmp.replace(path)


class McpError(RuntimeError):
    """The addon answered, but not with what the tool should answer."""


def mcp_call(base_url: str, name: str, arguments: dict, timeout: float = 10.0) -> Any:
    """One MCP ``tools/call`` against the addon's ``POST /api/mcp``.

    The bridge answers session-less JSON-RPC (verified against the live
    server); the tool result rides in ``result.content[0].text`` as JSON.
    Runs in the coordinator's executor thread, so plain urllib over
    ``socket`` is the whole dependency: no aiohttp in the grammar module.
    """
    import urllib.error
    import urllib.request

    payload = json.dumps(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        }
    ).encode("utf-8")
    req = urllib.request.Request(
        base_url.rstrip("/") + "/api/mcp",
        data=payload,
        headers={"content-type": "application/json", "accept": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            reply = json.loads(resp.read())
    except (OSError, urllib.error.URLError, ValueError) as err:
        raise McpError(f"mcp call {name}: {err}") from err
    if reply.get("error"):
        raise McpError(f"mcp call {name}: {reply['error']}")
    result = reply.get("result") or {}
    if result.get("isError"):
        raise McpError(f"mcp tool {name}: {result}")
    content = result.get("content") or [{}]
    try:
        return json.loads(content[0].get("text", ""))
    except (ValueError, TypeError, IndexError) as err:
        raise McpError(f"mcp tool {name} shape: {result}") from err



def mcp_set_todo_status(base_url: str, block_id: str, status: str) -> dict:
    """Move a block to HA ``status`` by cycling; the server owns the grammar.

    The bridge has no set-marker verb, so this walks the cycle -- the
    verified full lap is null -> TODO -> DOING -> DONE -> null, so any hop
    is at most three toggles. Un-tasked blocks count as needing action, so
    a null block already at rest is the answer for STATUS_OPEN.
    """
    goal = {STATUS_OPEN: "TODO", STATUS_DOING: "DOING", STATUS_DONE: "DONE"}[status]
    state = mcp_call(base_url, "outl_block_get", {"id": block_id})
    if status == STATUS_OPEN and state.get("todo") in (None, "TODO"):
        return state
    for _ in range(3):
        if str(state.get("todo") or "") == goal:
            return state
        state = mcp_call(base_url, "outl_block_toggle_todo", {"id": block_id})
    if str(state.get("todo") or "") != goal:
        raise McpError(f"cycle landed on {state.get('todo')!r}, not {goal!r}")
    return state


def mcp_append_journal_task(base_url: str, text: str) -> dict:
    """Append a task to today's journal through the server."""
    return mcp_call(base_url, "outl_daily_append", {"text": text})
