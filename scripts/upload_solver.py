#!/usr/bin/env python3
"""Upload and install the production infra-cli solver binary.

Defaults match the beta frontend host used by this project:

    python3 scripts/upload_solver.py

Optional overrides:

    SSH_PASSWORD=... python3 scripts/upload_solver.py --target arkinfra-server
    python3 scripts/upload_solver.py --no-restart

If SSH_PASSWORD is set, the script uses sshpass. Otherwise it relies on the
normal ssh/scp configuration, keys, or interactive prompt.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import textwrap
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

DEFAULT_SSH_TARGET = "arkinfra-server"
DEFAULT_LOCAL_ELF = ROOT / "target" / "release" / "infra-cli"
DEFAULT_REMOTE_TMP = "/root/infra-cli.new"
DEFAULT_REMOTE_DATA_TMP = "/root/infra-data.new.tar.gz"
DEFAULT_APP_DIR = "/root/ArknightsInfraCalc-v2_beta_test_frontend-main"
DEFAULT_PORT = 4174

DATA_DIR = ROOT / "data"
SKIP_DATA_DIRS = {"baked"}
SKIP_DATA_SUFFIXES = (":Zone.Identifier",)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def ensure_elf(path: Path) -> None:
    with path.open("rb") as f:
        magic = f.read(4)
    if magic != b"\x7fELF":
        raise SystemExit(f"{path} is not a Linux ELF. Do not upload infra-cli.exe.")


def with_password_tool(base_cmd: list[str], password: str) -> list[str]:
    if not password:
        return base_cmd

    if shutil.which("sshpass") is None:
        raise SystemExit(
            "SSH_PASSWORD is set, but sshpass was not found. "
            "Install sshpass or unset SSH_PASSWORD and use ssh normally."
        )

    return ["sshpass", "-e", *base_cmd]


def run(cmd: list[str], *, password: str, stdin: str | None = None) -> None:
    env = os.environ.copy()
    if password:
        env["SSHPASS"] = password

    print(f"\n$ {shlex.join(cmd)}")
    subprocess.run(cmd, input=stdin, text=True, env=env, check=True)


def should_bundle_data(path: Path) -> bool:
    rel = path.relative_to(DATA_DIR)
    if any(part in SKIP_DATA_DIRS for part in rel.parts):
        return False
    if any(path.name.endswith(suffix) for suffix in SKIP_DATA_SUFFIXES):
        return False
    return path.is_file()


def create_data_bundle() -> Path:
    if not DATA_DIR.is_dir():
        raise SystemExit(f"data directory does not exist: {DATA_DIR}")

    tmp = tempfile.NamedTemporaryFile(prefix="infra-data-", suffix=".tar.gz", delete=False)
    tmp_path = Path(tmp.name)
    tmp.close()
    try:
        with tarfile.open(tmp_path, "w:gz") as tar:
            for path in sorted(DATA_DIR.rglob("*")):
                if should_bundle_data(path):
                    tar.add(path, arcname=Path("data") / path.relative_to(DATA_DIR))
    except Exception:
        tmp_path.unlink(missing_ok=True)
        raise
    return tmp_path


def remote_update_script(
    app: str,
    new_cli: str,
    data_archive: str | None,
    port: int,
    restart: bool,
) -> str:
    data_block = ""
    if data_archive:
        data_block = f"""
        data_archive={shlex.quote(data_archive)}
        test -f "$data_archive"
        rm -rf "bin/data.tmp-$ts" "bin/data.new-$ts"
        mkdir -p "bin/data.tmp-$ts"
        tar -xzf "$data_archive" -C "bin/data.tmp-$ts"
        test -f "bin/data.tmp-$ts/data/skill_table.json"
        test -f "bin/data.tmp-$ts/data/operator_instances.json"
        test -f "bin/data.tmp-$ts/data/standalone_roster.json"
        mv "bin/data.tmp-$ts/data" "bin/data.new-$ts"
        rm -rf "bin/data.tmp-$ts"
        rm -rf "bin/data.backup-$ts"
        if [ -d bin/data ]; then
          mv bin/data "bin/data.backup-$ts"
        fi
        mv "bin/data.new-$ts" bin/data
        rm -f "$data_archive"

        echo "Installed data:"
        sha256sum bin/data/skill_table.json bin/data/operator_instances.json bin/data/standalone_roster.json || true
        """

    restart_block = ""
    if restart:
        restart_block = f"""
        if ! command -v node >/dev/null 2>&1; then
          if [ -s /root/.nvm/nvm.sh ]; then
            . /root/.nvm/nvm.sh
            nvm use default >/dev/null 2>&1 || nvm use node >/dev/null 2>&1 || true
          fi
        fi

        if ! command -v node >/dev/null 2>&1; then
          node_bin=$(find /root -path '*/bin/node' -type f -executable 2>/dev/null | head -1 || true)
          if [ -n "${{node_bin:-}}" ]; then
            export PATH="$(dirname "$node_bin"):$PATH"
          fi
        fi

        echo "Node:"
        if ! command -v node; then
          echo "node was not found; solver was installed but frontend was not restarted" >&2
          exit 1
        fi
        node --version

        old_pid=$(ss -ltnp 'sport = :{port}' | sed -n 's/.*pid=\\([0-9]\\+\\).*/\\1/p' | head -1 || true)
        if [ -n "${{old_pid:-}}" ]; then
          children=$(pgrep -P "$old_pid" || true)
          [ -n "$children" ] && kill $children || true
          kill "$old_pid" || true
          sleep 2
          kill -0 "$old_pid" 2>/dev/null && kill -9 "$old_pid" || true
        fi

        nohup ./node_modules/.bin/next start -H 0.0.0.0 -p {port} > server/next.log 2>&1 &

        echo "Listening port:"
        ss -ltnp | grep ':{port}' || true

        echo "Health:"
        curl -sS http://127.0.0.1:{port}/api/health || true

        echo
        echo "Next log tail:"
        tail -n 80 server/next.log || true
        """

    return (
        textwrap.dedent(
            f"""
            set -euo pipefail

            app={shlex.quote(app)}
            new_cli={shlex.quote(new_cli)}
            ts=$(date +%Y%m%d%H%M%S)

            cd "$app"

            test -f "$new_cli"
            test -f bin/infra-cli

            echo "Current solver:"
            sha256sum bin/infra-cli || true

            echo "New solver:"
            sha256sum "$new_cli" || true

            cp -a bin/infra-cli "bin/infra-cli.backup-$ts"
            cp "$new_cli" "bin/infra-cli.new-$ts"
            chmod +x "bin/infra-cli.new-$ts"
            mv -f "bin/infra-cli.new-$ts" bin/infra-cli
            chmod +x bin/infra-cli
            rm -f "$new_cli"

            echo "Installed solver:"
            sha256sum bin/infra-cli || true
            echo "Backup: $app/bin/infra-cli.backup-$ts"
            """
        ).strip()
        + "\n"
        + textwrap.dedent(data_block).strip()
        + "\n"
        + textwrap.dedent(restart_block).strip()
        + "\n"
    )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", default=DEFAULT_SSH_TARGET, help="ssh target alias/host")
    parser.add_argument(
        "--local-elf",
        type=Path,
        default=DEFAULT_LOCAL_ELF,
        help="local Linux infra-cli ELF to upload",
    )
    parser.add_argument("--remote-tmp", default=DEFAULT_REMOTE_TMP, help="remote temp path")
    parser.add_argument(
        "--remote-data-tmp",
        default=DEFAULT_REMOTE_DATA_TMP,
        help="remote temp path for bundled runtime data",
    )
    parser.add_argument("--app-dir", default=DEFAULT_APP_DIR, help="remote frontend app dir")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT, help="frontend port to restart")
    parser.add_argument(
        "--no-data",
        action="store_true",
        help="upload solver only; do not sync data into app/bin/data",
    )
    parser.add_argument(
        "--no-restart",
        action="store_true",
        help="install solver but do not restart the frontend",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    password = os.environ.get("SSH_PASSWORD", "")

    local_elf = args.local_elf.expanduser().resolve()
    if not local_elf.is_file():
        raise SystemExit(
            f"local ELF does not exist: {local_elf}\n"
            "Build it first with: cargo build --release -p infra-cli"
        )
    ensure_elf(local_elf)

    print(f"Local ELF: {local_elf}")
    print(f"Local sha256: {sha256(local_elf)}")
    print(f"SSH target: {args.target}")
    print(f"Remote app: {args.app_dir}")

    scp_cmd = with_password_tool(
        ["scp", str(local_elf), f"{args.target}:{args.remote_tmp}"],
        password,
    )
    run(scp_cmd, password=password)

    data_bundle: Path | None = None
    try:
        if not args.no_data:
            data_bundle = create_data_bundle()
            print(f"Data bundle: {data_bundle}")
            data_scp_cmd = with_password_tool(
                ["scp", str(data_bundle), f"{args.target}:{args.remote_data_tmp}"],
                password,
            )
            run(data_scp_cmd, password=password)

        data_archive = None if args.no_data else args.remote_data_tmp
        script = remote_update_script(
            app=args.app_dir,
            new_cli=args.remote_tmp,
            data_archive=data_archive,
            port=args.port,
            restart=not args.no_restart,
        )
        ssh_cmd = with_password_tool(["ssh", args.target, "bash", "-s"], password)
        run(ssh_cmd, password=password, stdin=script)
    finally:
        if data_bundle is not None:
            data_bundle.unlink(missing_ok=True)

    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as exc:
        raise SystemExit(exc.returncode) from exc
