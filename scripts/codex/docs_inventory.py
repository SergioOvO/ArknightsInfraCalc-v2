#!/usr/bin/env python3
"""Parse, validate, and render the repository documentation inventory."""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


SCHEMA_VERSION = 2
META_KEYS = {
    "文档角色",
    "生命周期状态",
    "领域键",
    "当前真源",
    "复核触发",
    "摘要",
    "生成器",
    "替代项",
    "历史原因",
    "快照日期",
    "源摘要",
    "文档摘要",
    "复核原因",
    "复核结论",
    "稳定事实",
    "证据引用",
    "转换自",
    "转换处置",
    "事实映射",
}
DYNAMIC_META_KEYS = {
    "源摘要",
    "文档摘要",
    "复核原因",
    "复核结论",
    "稳定事实",
    "证据引用",
}
ROLES = {
    "canonical",
    "current-reference",
    "active-change",
    "decision",
    "generated-reference",
    "evidence",
    "archive",
}
ROLE_STATES = {
    "canonical": {"current"},
    "current-reference": {"current"},
    "active-change": {"proposal", "ready-on-request", "in-progress", "blocked"},
    "decision": {"proposed", "accepted", "superseded"},
    "generated-reference": {"generated"},
    "evidence": {"current", "closed"},
    "archive": {"completed", "superseded", "historical"},
}
MUTABLE_ROLES = {"canonical", "current-reference", "active-change"}
REVIEWABLE_ROLES = MUTABLE_ROLES | {"decision", "generated-reference"}
README_EXCEPTIONS = {"docs/TODO/README.md", "docs/ARCHIVE/README.md"}
GENERATED_SECTIONS = {
    "canonical": ("<!-- BEGIN GENERATED CANONICAL -->", "<!-- END GENERATED CANONICAL -->"),
    "active": ("<!-- BEGIN GENERATED ACTIVE CHANGES -->", "<!-- END GENERATED ACTIVE CHANGES -->"),
    "migration-ledger": ("<!-- BEGIN GENERATED MIGRATION LEDGER -->", "<!-- END GENERATED MIGRATION LEDGER -->"),
}
REVIEW_CAUSES = {
    "lifecycle-migration",
    "source-change",
    "document-change",
    "owner-change",
    "user-ruling",
    "transition",
}
TRANSITION_DISPOSITIONS = {
    "archive-historical",
    "archive-audit",
    "archive-completed",
    "archive-superseded",
    "move-active-change",
    "delete-after-absorb",
    "canonical-transfer",
}
META_RE = re.compile(r"^>\s*([^：:]+)[：:]\s*(.*?)\s*$")
H1_RE = re.compile(r"^#\s+(.+?)\s*$")


class InventoryError(ValueError):
    pass


@dataclass(frozen=True)
class Document:
    path: str
    title: str
    metadata: dict[str, str]
    text: str
    metadata_start: int
    metadata_end: int

    @property
    def role(self) -> str:
        return self.metadata.get("文档角色", "")

    @property
    def status(self) -> str:
        return self.metadata.get("生命周期状态", "")

    @property
    def domains(self) -> list[str]:
        return split_values(self.metadata.get("领域键", ""))

    @property
    def owners(self) -> list[str]:
        return split_values(self.metadata.get("当前真源", ""))

    @property
    def triggers(self) -> list[str]:
        return split_values(self.metadata.get("复核触发", ""))


def split_values(value: str) -> list[str]:
    if not value:
        return []
    return [item.strip().replace("\\", "/") for item in re.split(r"[；;]", value) if item.strip()]


def _repo_relative(repo: Path, path: Path) -> str:
    return path.resolve(strict=False).relative_to(repo.resolve()).as_posix()


def governed_markdown_paths(repo: Path) -> list[Path]:
    paths: set[Path] = set()
    for root_name in ("docs", "plans"):
        root = repo / root_name
        if root.is_dir():
            paths.update(root.rglob("*.md"))
    feedback = repo / "feedback"
    if feedback.is_dir():
        paths.update(path for path in feedback.glob("*.md") if path.is_file())
    return sorted(paths, key=lambda path: _repo_relative(repo, path))


def parse_document(repo: Path, path: Path, text: str | None = None) -> Document:
    relative = _repo_relative(repo, path)
    source = path.read_text(encoding="utf-8") if text is None else text
    lines = source.splitlines()
    if not lines:
        raise InventoryError(f"empty Markdown document: {relative}")
    title_match = H1_RE.match(lines[0])
    if not title_match:
        raise InventoryError(f"document must start with one H1 title: {relative}")

    index = 1
    while index < len(lines) and not lines[index].strip():
        index += 1
    start = index
    metadata: dict[str, str] = {}
    while index < len(lines) and lines[index].startswith(">"):
        match = META_RE.match(lines[index])
        if not match:
            raise InventoryError(f"invalid lifecycle metadata line: {relative}:{index + 1}")
        key, value = match.groups()
        key = key.strip()
        if key not in META_KEYS:
            raise InventoryError(f"unknown document metadata key {key!r}: {relative}:{index + 1}")
        if key in metadata:
            raise InventoryError(f"duplicate document metadata key {key!r}: {relative}:{index + 1}")
        metadata[key] = value.strip()
        index += 1
    if not metadata:
        raise InventoryError(f"document lacks lifecycle metadata after H1: {relative}")
    return Document(relative, title_match.group(1), metadata, source, start, index)


def load_inventory(repo: Path) -> tuple[list[Document], list[str]]:
    documents: list[Document] = []
    errors: list[str] = []
    for path in governed_markdown_paths(repo):
        try:
            documents.append(parse_document(repo, path))
        except (OSError, UnicodeError, InventoryError) as error:
            errors.append(str(error))
    return documents, errors


def _is_repo_glob(value: str) -> bool:
    if not value or value.startswith(("/", "../")) or "\\" in value:
        return False
    return not any(part == ".." for part in Path(value).parts)


def path_matches_pattern(path: str, pattern: str) -> bool:
    normalized = pattern.rstrip("/")
    if any(character in normalized for character in "*?["):
        return fnmatch.fnmatchcase(path, normalized)
    return path == normalized or path.startswith(f"{normalized}/")


def dependency_patterns(document: Document) -> list[str]:
    return [*document.triggers, *split_values(document.metadata.get("生成器", ""))]


def markdown_anchors(text: str) -> set[str]:
    anchors: set[str] = set()
    counts: dict[str, int] = {}
    for line in text.splitlines():
        match = re.match(r"^#{1,6}\s+(.+?)\s*#*\s*$", line)
        if not match:
            continue
        title = re.sub(r"[`*_~]", "", match.group(1)).strip().lower()
        slug = re.sub(r"[^\w\-\s\u4e00-\u9fff]", "", title, flags=re.UNICODE)
        slug = re.sub(r"[\s\-]+", "-", slug).strip("-")
        suffix = counts.get(slug, 0)
        counts[slug] = suffix + 1
        anchors.add(f"{slug}-{suffix}" if suffix else slug)
    return anchors


def _path_role_errors(document: Document) -> list[str]:
    errors: list[str] = []
    path = document.path
    role = document.role
    if path.startswith("plans/"):
        errors.append(f"root plans must not contain Markdown: {path}")
    if path.startswith("docs/TODO/"):
        if path == "docs/TODO/README.md":
            if role != "current-reference":
                errors.append(f"TODO README must be current-reference: {path}")
        elif role != "active-change":
            errors.append(f"TODO document must be active-change: {path}")
    elif path.startswith("docs/ADR/") and role != "decision":
        errors.append(f"ADR document must be decision: {path}")
    elif path.startswith("docs/ARCHIVE/"):
        if path == "docs/ARCHIVE/README.md":
            if role != "current-reference":
                errors.append(f"ARCHIVE README must be current-reference: {path}")
        elif role != "archive":
            errors.append(f"archive path must use archive role: {path}")
    elif path.startswith("feedback/") and role not in {"evidence", "current-reference"}:
        errors.append(f"tracked feedback Markdown must be evidence/current-reference: {path}")
    elif path.startswith("docs/") and role in {"active-change", "archive"}:
        errors.append(f"current docs path cannot use {role} role: {path}")
    if role == "active-change" and not (path.startswith("docs/TODO/") and path != "docs/TODO/README.md"):
        errors.append(f"active-change role is only allowed under docs/TODO: {path}")
    if role == "decision" and not path.startswith("docs/ADR/"):
        errors.append(f"decision role is only allowed under docs/ADR: {path}")
    if role == "archive" and not path.startswith("docs/ARCHIVE/"):
        errors.append(f"archive role is only allowed under docs/ARCHIVE: {path}")
    if role == "evidence" and not path.startswith("feedback/"):
        errors.append(f"evidence role requires a named evidence root: {path}")
    if role in {"canonical", "generated-reference"} and not path.startswith("docs/"):
        errors.append(f"{role} role is only allowed in docs current paths: {path}")
    return errors


def validate_document(document: Document, *, final: bool) -> list[str]:
    errors: list[str] = []
    role = document.role
    status = document.status
    if role not in ROLES:
        errors.append(f"invalid document role {role!r}: {document.path}")
        return errors
    if status not in ROLE_STATES[role]:
        errors.append(f"invalid lifecycle status {status!r} for {role}: {document.path}")
    if not document.metadata.get("摘要"):
        errors.append(f"document summary is required: {document.path}")
    errors.extend(_path_role_errors(document))

    if role == "canonical":
        if not document.domains:
            errors.append(f"canonical document requires at least one domain key: {document.path}")
        if document.owners != ["self"]:
            errors.append(f"canonical current owner must be self: {document.path}")
    elif role in {"current-reference", "active-change", "decision", "generated-reference", "evidence"}:
        if not document.owners:
            errors.append(f"{role} document requires current owner links: {document.path}")
    elif role == "archive" and not (
        document.metadata.get("替代项") or document.metadata.get("历史原因")
    ):
        errors.append(f"archive document requires replacement or historical reason: {document.path}")

    if role == "generated-reference" and not document.metadata.get("生成器"):
        errors.append(f"generated-reference requires a generator: {document.path}")
    mutable = role in MUTABLE_ROLES or (role == "evidence" and status == "current")
    if mutable and not (document.triggers or document.metadata.get("生成器")):
        errors.append(f"mutable document requires review triggers or generator: {document.path}")
    for trigger in document.triggers:
        if not _is_repo_glob(trigger):
            errors.append(f"invalid review trigger {trigger!r}: {document.path}")
        if trigger.startswith(("docs/", "plans/", "feedback/")):
            errors.append(f"review trigger must not target governed Markdown roots: {document.path}: {trigger}")
    for generator in split_values(document.metadata.get("生成器", "")):
        if not _is_repo_glob(generator) or any(char in generator for char in "*?["):
            errors.append(f"generator must be a concrete repository path: {document.path}: {generator}")
        if generator.startswith(("docs/", "plans/", "feedback/")):
            errors.append(f"generator must not target governed Markdown roots: {document.path}: {generator}")
    for owner in document.owners:
        if owner == "self":
            continue
        if not _is_repo_glob(owner) or any(char in owner for char in "*?["):
            errors.append(f"current owner must be a concrete repository path: {document.path}: {owner}")
    transition_from = split_values(document.metadata.get("转换自", ""))
    transition_disposition = document.metadata.get("转换处置", "")
    if bool(transition_from) != bool(transition_disposition):
        errors.append(f"transition-from and disposition must appear together: {document.path}")
    if transition_disposition and transition_disposition not in TRANSITION_DISPOSITIONS:
        errors.append(f"invalid transition disposition: {document.path}: {transition_disposition}")
    for source in transition_from:
        if not _is_repo_glob(source) or any(char in source for char in "*?["):
            errors.append(f"transition source must be a concrete repository path: {document.path}: {source}")
    if transition_disposition:
        if transition_disposition.startswith("archive-") and role != "archive":
            errors.append(f"archive transition requires archive carrier: {document.path}")
        if transition_disposition == "move-active-change" and role != "active-change":
            errors.append(f"active transition requires active-change carrier: {document.path}")
        if transition_disposition == "canonical-transfer" and role != "canonical":
            errors.append(f"canonical transfer requires canonical carrier: {document.path}")
        if transition_disposition == "delete-after-absorb" and not split_values(document.metadata.get("事实映射", "")):
            errors.append(f"delete-after-absorb requires fact mappings: {document.path}")
    if final and role == "active-change" and status == "in-progress":
        errors.append(f"in-progress change cannot pass final check: {document.path}")
    return errors


def validate_inventory(repo: Path, documents: list[Document], *, final: bool) -> list[str]:
    errors: list[str] = []
    by_path = {document.path: document for document in documents}
    for document in documents:
        errors.extend(validate_document(document, final=final))
    domain_owners: dict[str, list[str]] = {}
    for document in documents:
        if document.role == "canonical":
            for domain in document.domains:
                domain_owners.setdefault(domain, []).append(document.path)
    for domain, owners in sorted(domain_owners.items()):
        if len(owners) != 1:
            errors.append(f"domain key must have exactly one canonical owner: {domain}: {owners}")
    for document in documents:
        for owner in document.owners:
            if owner == "self":
                continue
            target = by_path.get(owner)
            if target is None:
                errors.append(f"current owner must be a governed current document: {document.path}: {owner}")
                continue
            if target.role not in {"canonical", "current-reference"}:
                errors.append(f"current owner is not current truth: {document.path}: {owner}")
        if document.metadata.get("转换处置") == "delete-after-absorb":
            for mapping in split_values(document.metadata.get("事实映射", "")):
                target_path, fragment = (mapping.split("#", 1) + [""])[:2]
                target = by_path.get(target_path)
                if target is None or target.role not in {"canonical", "current-reference"}:
                    errors.append(f"delete fact mapping must target current truth: {document.path}: {mapping}")
                elif not fragment or fragment not in markdown_anchors(target.text):
                    errors.append(f"delete fact mapping fragment does not exist: {document.path}: {mapping}")
    return sorted(set(errors))


def _without_generated_sections(text: str) -> str:
    result = text
    for begin, end in GENERATED_SECTIONS.values():
        pattern = re.compile(re.escape(begin) + r".*?" + re.escape(end), re.DOTALL)
        result = pattern.sub(f"{begin}\n{end}", result)
    return result


def document_digest(document: Document) -> str:
    lines = _without_generated_sections(document.text).splitlines()
    projected: list[str] = []
    for line in lines:
        match = META_RE.match(line)
        if match and match.group(1).strip() in DYNAMIC_META_KEYS:
            continue
        projected.append(line.rstrip())
    value = "\n".join(projected).rstrip() + "\n"
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _tracked_objects(repo: Path) -> list[tuple[str, str, str]]:
    try:
        result = subprocess.run(
            ["git", "-c", "core.quotepath=false", "-C", str(repo), "ls-files", "--stage", "-z"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise InventoryError(f"cannot read tracked Git blobs from {repo}") from error

    objects: list[tuple[str, str, str]] = []
    try:
        for record in result.stdout.split(b"\0"):
            if not record:
                continue
            attributes, raw_path = record.split(b"\t", 1)
            mode, raw_oid, stage = attributes.split(b" ")
            if stage != b"0":
                raise InventoryError(f"unmerged tracked path cannot be hashed: {raw_path.decode('utf-8')}")
            oid = raw_oid.decode("ascii")
            if not oid.strip("0"):
                raise InventoryError(f"tracked path lacks a Git object: {raw_path.decode('utf-8')}")
            if mode == b"160000":
                object_type = "gitlink"
            elif mode.startswith((b"100", b"120")):
                object_type = "blob"
            else:
                raise InventoryError(f"unsupported tracked mode {mode.decode('ascii')}: {raw_path.decode('utf-8')}")
            objects.append((raw_path.decode("utf-8"), object_type, oid))
    except (UnicodeError, ValueError) as error:
        raise InventoryError("cannot parse tracked Git blob inventory") from error
    return sorted(objects)


def _trigger_objects(repo: Path, pattern: str) -> list[tuple[str, str, str]]:
    return [item for item in _tracked_objects(repo) if path_matches_pattern(item[0], pattern)]


def source_digest(repo: Path, document: Document) -> str:
    digest = hashlib.sha256()
    digest.update(f"docs-source-digest-v{SCHEMA_VERSION}\0".encode())
    generators = set(split_values(document.metadata.get("生成器", "")))
    for pattern in sorted(dependency_patterns(document)):
        digest.update(f"pattern\0{pattern}\0".encode())
        objects = _trigger_objects(repo, pattern)
        if pattern in generators and (
            len(objects) != 1 or objects[0][0] != pattern or objects[0][1] != "blob"
        ):
            raise InventoryError(f"generator must resolve to exactly one tracked Git blob: {document.path}: {pattern}")
        if not objects:
            digest.update(b"empty\0")
        for relative, object_type, oid in objects:
            digest.update(f"file\0{relative}\0".encode())
            digest.update(f"{object_type}\0{oid}\0".encode())
    return digest.hexdigest()


def is_reviewable(document: Document) -> bool:
    return document.role in REVIEWABLE_ROLES or (document.role == "evidence" and document.status == "current")


def review_errors(repo: Path, document: Document) -> list[str]:
    if not is_reviewable(document):
        return []
    errors: list[str] = []
    try:
        expected_source = source_digest(repo, document)
    except InventoryError as error:
        return [f"{error}: {document.path}"]
    expected_document = document_digest(document)
    if document.metadata.get("源摘要") != expected_source:
        errors.append(f"source digest drift: {document.path}")
    if document.metadata.get("文档摘要") != expected_document:
        errors.append(f"document digest drift: {document.path}")
    for key in ("复核原因", "复核结论", "稳定事实", "证据引用"):
        if not document.metadata.get(key):
            errors.append(f"review record lacks {key}: {document.path}")
    if document.metadata.get("复核原因") not in REVIEW_CAUSES:
        errors.append(f"invalid review cause: {document.path}")
    if document.metadata.get("复核结论") not in {"updated", "unchanged"}:
        errors.append(f"invalid review disposition: {document.path}")
    if len(document.metadata.get("稳定事实", "").strip()) < 12:
        errors.append(f"review stable fact is not specific: {document.path}")
    for reference in split_values(document.metadata.get("证据引用", "")):
        if reference.startswith("tracked:"):
            target = reference.removeprefix("tracked:")
            if not _is_repo_glob(target) or not (repo / target).is_file():
                errors.append(f"tracked review evidence does not exist: {document.path}: {target}")
        elif reference.startswith("test:"):
            target = reference.removeprefix("test:").split("::", 1)[0]
            if not _is_repo_glob(target) or not (repo / target).is_file():
                errors.append(f"test review evidence does not exist: {document.path}: {target}")
        elif reference.startswith(("command:", "artifact:")):
            continue
        else:
            errors.append(f"unsupported review evidence reference: {document.path}: {reference}")
    return errors


def _line_ending(line: str) -> str:
    if line.endswith("\r\n"):
        return "\r\n"
    return "\n"


def write_review_record(repo: Path, document: Document, *, cause: str) -> None:
    if cause not in REVIEW_CAUSES:
        raise InventoryError(f"unsupported review cause: {cause}")
    if not is_reviewable(document):
        return
    expected_source = source_digest(repo, document)
    expected_document = document_digest(document)
    if (
        document.metadata.get("源摘要") == expected_source
        and document.metadata.get("文档摘要") == expected_document
        and document.metadata.get("复核原因") == cause
        and all(document.metadata.get(key) for key in ("复核原因", "复核结论", "稳定事实", "证据引用"))
    ):
        return
    values = {
        "源摘要": expected_source,
        "文档摘要": expected_document,
        "复核原因": cause,
        "复核结论": "updated",
        "稳定事实": document.metadata["摘要"],
        "证据引用": f"tracked:{document.path}",
    }
    path = repo / document.path
    text = path.read_bytes().decode("utf-8")
    lines = text.splitlines(keepends=True)
    index = 1
    while index < len(lines) and not lines[index].strip():
        index += 1
    start = index
    while index < len(lines) and lines[index].startswith(">"):
        match = META_RE.match(lines[index].rstrip("\r\n"))
        if not match or match.group(1).strip() not in META_KEYS:
            break
        index += 1
    ending = _line_ending(lines[start])
    kept: list[str] = []
    for line in lines[start:index]:
        match = META_RE.match(line.rstrip("\r\n"))
        if match and match.group(1).strip() in DYNAMIC_META_KEYS:
            continue
        kept.append(line)
    kept.extend(f"> {key}：{value}{ending}" for key, value in values.items())
    path.write_bytes("".join([*lines[:start], *kept, *lines[index:]]).encode("utf-8"))


def refresh_review_records(repo: Path, documents: list[Document], *, cause: str) -> None:
    for document in documents:
        write_review_record(repo, document, cause=cause)


def docs_impact_entry(document: Document) -> dict[str, object]:
    return {
        "path": document.path,
        "source_digest": document.metadata["源摘要"],
        "document_digest": document.metadata["文档摘要"],
        "cause": document.metadata["复核原因"],
        "disposition": document.metadata["复核结论"],
        "stable_facts": split_values(document.metadata["稳定事实"]),
        "evidence": split_values(document.metadata["证据引用"]),
    }


def write_docs_impact_metadata(path: Path, documents: list[Document]) -> None:
    value = json.loads(path.read_text(encoding="utf-8"))
    entries = [
        docs_impact_entry(document)
        for document in documents
        if document.metadata.get("复核结论") in {"updated", "unchanged"}
    ]
    value["docs_impact"] = {
        "status": "updated",
        "entries": entries,
        "reason": "全量生命周期迁移已逐文档绑定 source/document digest 和可核对 evidence",
    }
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def render_canonical_table(documents: Iterable[Document]) -> str:
    rows: list[tuple[str, str, str]] = []
    for document in documents:
        if document.role != "canonical":
            continue
        link = Path(document.path).relative_to("docs").as_posix()
        for domain in document.domains:
            rows.append((domain, document.title, link))
    lines = ["| 领域键 | 权威入口 |", "|---|---|"]
    lines.extend(f"| `{domain}` | [{title}]({link}) |" for domain, title, link in sorted(rows))
    return "\n".join(lines)


def render_active_table(documents: Iterable[Document]) -> str:
    rows = [document for document in documents if document.role == "active-change"]
    lines = ["| 文件 | 状态 | 用途 |", "|---|---|---|"]
    for document in sorted(rows, key=lambda item: item.path):
        link = Path(document.path).name
        lines.append(
            f"| [{document.title}]({link}) | `{document.status}` | {document.metadata.get('摘要', '')} |"
        )
    return "\n".join(lines)


def _git_blob_sha(repo: Path, commit: str, path: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", f"{commit}:{path}"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result.stdout.strip()


def render_migration_ledger(repo: Path, base: str, documents: list[Document]) -> str:
    _, _, base_paths = load_inventory_at(repo, base)
    by_path = {document.path: document for document in documents}
    transitions: dict[str, list[Document]] = {}
    for document in documents:
        for source in split_values(document.metadata.get("转换自", "")):
            transitions.setdefault(source, []).append(document)
    lines = [
        "| Base path | Blob | Disposition | Destination / current owner | Fact mapping |",
        "|---|---|---|---|---|",
    ]
    for path in base_paths:
        blob = _git_blob_sha(repo, base, path)[:12]
        if path in by_path:
            document = by_path[path]
            disposition = f"keep:{document.role}/{document.status}"
            owner = "；".join(document.domains or document.owners or split_values(document.metadata.get("替代项", "")))
            mapping = "原路径保留；按 lifecycle metadata 和 current owner 对账"
        else:
            carriers = transitions.get(path, [])
            if len(carriers) != 1:
                disposition = "ERROR"
                owner = "missing transition carrier"
                mapping = "未闭合"
            else:
                carrier = carriers[0]
                disposition = carrier.metadata.get("转换处置", "transition")
                owner = carrier.path
                mapping = (
                    "原文完整保留"
                    if disposition != "delete-after-absorb"
                    else "；".join(split_values(carrier.metadata.get("事实映射", "")))
                )
        lines.append(f"| `{path}` | `{blob}` | `{disposition}` | {owner} | {mapping} |")
    return "\n".join(lines)


def replace_generated_section(text: str, section: str, body: str) -> str:
    begin, end = GENERATED_SECTIONS[section]
    replacement = f"{begin}\n{body}\n{end}"
    pattern = re.compile(re.escape(begin) + r".*?" + re.escape(end), re.DOTALL)
    if pattern.search(text):
        return pattern.sub(replacement, text)
    raise InventoryError(f"generated section markers are missing: {section}")


def check_generated_sections(repo: Path, documents: list[Document]) -> list[str]:
    errors: list[str] = []
    targets = {
        "canonical": repo / "docs/INDEX.md",
        "active": repo / "docs/TODO/README.md",
    }
    bodies = {
        "canonical": render_canonical_table(documents),
        "active": render_active_table(documents),
    }
    for section, path in targets.items():
        try:
            text = path.read_text(encoding="utf-8")
            expected = replace_generated_section(text, section, bodies[section])
        except (OSError, UnicodeError, InventoryError) as error:
            errors.append(str(error))
            continue
        if expected != text:
            errors.append(f"generated documentation section drift: {_repo_relative(repo, path)}")
    return errors


def write_generated_sections(repo: Path, documents: list[Document]) -> None:
    targets = {
        "canonical": (repo / "docs/INDEX.md", render_canonical_table(documents)),
        "active": (repo / "docs/TODO/README.md", render_active_table(documents)),
    }
    for section, (path, body) in targets.items():
        text = path.read_text(encoding="utf-8")
        path.write_text(replace_generated_section(text, section, body), encoding="utf-8")


def write_migration_ledger(repo: Path, documents: list[Document], base: str, path: str) -> None:
    target = repo / path
    text = target.read_text(encoding="utf-8")
    body = render_migration_ledger(repo, base, documents)
    target.write_text(replace_generated_section(text, "migration-ledger", body), encoding="utf-8")


def _git_changed_paths(repo: Path, base: str) -> list[str]:
    paths: set[str] = set()
    for arguments in (
        ["diff", "--name-only", "--diff-filter=ACMRD", base, "HEAD"],
        ["diff", "--name-only", "--diff-filter=ACMRD"],
        ["diff", "--cached", "--name-only", "--diff-filter=ACMRD"],
        ["ls-files", "--others", "--exclude-standard"],
    ):
        result = subprocess.run(
            ["git", "-c", "core.quotepath=false", "-C", str(repo), *arguments],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        paths.update(line for line in result.stdout.splitlines() if line)
    return sorted(paths)


def _git_text(repo: Path, commit: str, path: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), "show", f"{commit}:{path}"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result.stdout.decode("utf-8")


def load_inventory_at(repo: Path, commit: str) -> tuple[list[Document], list[str], list[str]]:
    result = subprocess.run(
        ["git", "-C", str(repo), "ls-tree", "-r", "--name-only", commit, "--", "docs", "plans", "feedback"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    paths = [
        line
        for line in result.stdout.splitlines()
        if line.endswith(".md")
        and (line.startswith(("docs/", "plans/")) or line in {"feedback/README.md", "feedback/TRACKING.md"})
    ]
    documents: list[Document] = []
    errors: list[str] = []
    for relative in paths:
        try:
            documents.append(parse_document(repo, repo / relative, _git_text(repo, commit, relative)))
        except (UnicodeError, InventoryError, subprocess.CalledProcessError) as error:
            errors.append(str(error))
    return documents, errors, sorted(paths)


def _config_ignores(repo: Path, commit: str | None = None) -> list[str]:
    try:
        if commit:
            text = _git_text(repo, commit, "scripts/codex/docs_impact.toml")
            value = tomllib.loads(text)
        else:
            with (repo / "scripts/codex/docs_impact.toml").open("rb") as handle:
                value = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError, UnicodeError, subprocess.CalledProcessError):
        return []
    ignores = value.get("ignore_globs", [])
    return [str(item) for item in ignores] if isinstance(ignores, list) else []


def source_coverage_errors(
    changed_paths: Iterable[str], documents: Iterable[Document], ignore_globs: Iterable[str],
    base_documents: Iterable[Document] = (),
) -> list[str]:
    errors: list[str] = []
    triggers = [
        (document.path, pattern)
        for document in [*documents, *base_documents]
        for pattern in dependency_patterns(document)
    ]
    for path in sorted(set(changed_paths)):
        if path.endswith(".md") and path.startswith(("docs/", "plans/", "feedback/")):
            continue
        if any(path_matches_pattern(path, pattern) for pattern in ignore_globs):
            continue
        owners = {owner for owner, pattern in triggers if path_matches_pattern(path, pattern)}
        if not owners:
            errors.append(f"changed source path has no document owner: {path}")
    return errors


def continuity_errors(
    repo: Path,
    base: str,
    head_documents: list[Document],
    base_documents: list[Document],
    base_paths: list[str],
) -> list[str]:
    errors: list[str] = []
    head_by_path = {document.path: document for document in head_documents}
    transition_carriers: dict[str, list[str]] = {}
    for document in head_documents:
        for source in split_values(document.metadata.get("转换自", "")):
            transition_carriers.setdefault(source, []).append(document.path)

    if not base_documents and base_paths:
        ledger_path = repo / "docs/ARCHIVE/done/文档生命周期一致性重建.md"
        if not ledger_path.is_file():
            errors.append("legacy base requires the archived lifecycle migration ledger")
        else:
            ledger = ledger_path.read_text(encoding="utf-8")
            expected = render_migration_ledger(repo, base, head_documents)
            try:
                rendered = replace_generated_section(ledger, "migration-ledger", expected)
            except InventoryError as error:
                errors.append(str(error))
            else:
                if rendered != ledger:
                    errors.append("archived migration ledger does not match deterministic base disposition")
    else:
        base_domains = {domain for document in base_documents for domain in document.domains}
        head_domains = {domain for document in head_documents for domain in document.domains}
        removed_domains = sorted(base_domains - head_domains)
        if removed_domains:
            errors.append(f"canonical domain keys disappeared without continuity: {removed_domains}")
        for document in base_documents:
            if document.path in head_by_path:
                continue
            carriers = transition_carriers.get(document.path, [])
            if len(carriers) != 1:
                errors.append(
                    f"removed document requires exactly one transition carrier: {document.path}: {carriers}"
                )

        base_by_path = {document.path: document for document in base_documents}
        changed_paths = _git_changed_paths(repo, base)
        required_reviews: set[str] = set()
        for path in changed_paths:
            if path.endswith(".md") and path.startswith(("docs/", "plans/", "feedback/")):
                if path in head_by_path:
                    required_reviews.add(path)
                continue
            for document in [*base_documents, *head_documents]:
                if any(path_matches_pattern(path, trigger) for trigger in dependency_patterns(document)):
                    if document.path in head_by_path:
                        required_reviews.add(document.path)

        dependents: dict[str, set[str]] = {}
        for document in head_documents:
            if document.role in {"archive"}:
                continue
            for owner in document.owners:
                if owner == "self":
                    continue
                dependents.setdefault(owner, set()).add(document.path)
                if document.role == "current-reference":
                    dependents.setdefault(document.path, set()).add(owner)
        while True:
            expanded = set(required_reviews)
            for path in required_reviews:
                expanded.update(dependents.get(path, set()))
            if expanded == required_reviews:
                break
            required_reviews = expanded

        for path in sorted(required_reviews):
            head_document = head_by_path.get(path)
            if head_document is None or head_document.role not in REVIEWABLE_ROLES | {"evidence"}:
                continue
            base_document = base_by_path.get(path)
            if base_document is None:
                continue
            keys = ["源摘要", "文档摘要", "复核原因", "复核结论", "稳定事实", "证据引用"]
            base_signature = tuple(base_document.metadata.get(key, "") for key in keys)
            head_signature = tuple(head_document.metadata.get(key, "") for key in keys)
            if base_signature == head_signature:
                errors.append(f"dependent document review record did not change: {path}")

    changed_paths = _git_changed_paths(repo, base)
    ignores = _config_ignores(repo)
    errors.extend(source_coverage_errors(changed_paths, head_documents, ignores, base_documents))
    base_ignores = set(_config_ignores(repo, base))
    added_ignores = sorted(set(ignores) - base_ignores)
    if added_ignores:
        lifecycle = head_by_path.get("docs/文档生命周期.md")
        if lifecycle is None or lifecycle.metadata.get("复核原因") not in REVIEW_CAUSES:
            errors.append(f"new docs-impact exclusions require lifecycle owner review: {added_ignores}")
    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--check", action="store_true", help="run final hard checks")
    parser.add_argument("--report", action="store_true", help="print inventory violations without final-only rules")
    parser.add_argument("--write-indexes", action="store_true")
    parser.add_argument("--write-migration-ledger", metavar="PATH")
    parser.add_argument("--refresh-reviews", action="store_true")
    parser.add_argument("--review-cause", choices=sorted(REVIEW_CAUSES), default="lifecycle-migration")
    parser.add_argument("--write-docs-impact-metadata", type=Path)
    parser.add_argument("--base", help="check changed source coverage against this Git base")
    parser.add_argument("--ignore", action="append", default=[])
    return parser


def main() -> int:
    args = build_parser().parse_args()
    repo = args.repo_root.resolve()
    documents, errors = load_inventory(repo)
    errors.extend(validate_inventory(repo, documents, final=args.check))
    if args.write_indexes and not errors:
        write_generated_sections(repo, documents)
    if args.write_migration_ledger and not errors:
        if not args.base:
            errors.append("--write-migration-ledger requires --base")
        else:
            write_migration_ledger(repo, documents, args.base, args.write_migration_ledger)
    if args.refresh_reviews and not errors:
        try:
            refresh_review_records(repo, documents, cause=args.review_cause)
        except InventoryError as error:
            errors.append(str(error))
        if not errors:
            documents, refresh_errors = load_inventory(repo)
            errors.extend(refresh_errors)
    if args.write_docs_impact_metadata and not errors:
        write_docs_impact_metadata(args.write_docs_impact_metadata, documents)
    if args.check:
        errors.extend(check_generated_sections(repo, documents))
        for document in documents:
            errors.extend(review_errors(repo, document))
    if args.base:
        try:
            subprocess.run(
                ["git", "-C", str(repo), "cat-file", "-e", f"{args.base}^{{commit}}"],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            base_documents, base_errors, base_paths = load_inventory_at(repo, args.base)
            if base_documents and base_errors:
                errors.extend(base_errors)
            errors.extend(continuity_errors(repo, args.base, documents, base_documents, base_paths))
        except subprocess.CalledProcessError:
            errors.append(f"cannot verify Git base commit: {args.base}")
    if errors:
        for error in sorted(set(errors)):
            print(f"error: {error}", file=sys.stderr)
        print(f"FAIL docs_inventory documents={len(documents)} errors={len(set(errors))}")
        return 1
    print(f"PASS docs_inventory documents={len(documents)} schema={SCHEMA_VERSION}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
