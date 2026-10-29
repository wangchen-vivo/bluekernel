#!/usr/bin/env python3

# Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#       http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
"""Run rustc/Clippy with GN artifact bindings and build.rs environment values."""

import os
import sys

_ENV_FILE_VAR = "BUILD_SCRIPT_RUSTC_ENV_FILE"

# Variables used by this wrapper that must not leak into the compiler.
_INTERNAL_VARS = (_ENV_FILE_VAR, )

_DOUBLE_QUOTE_ESCAPES = {
    "n": "\n",
    "t": "\t",
    "r": "\r",
    "\\": "\\",
    '"': '"',
    "$": "$",
    "`": "`",
}


def _unquote(value):
    """Returns `value` with one layer of shell-style quoting removed."""
    if len(value) >= 2 and value[0] == "'" and value[-1] == "'":
        return value[1:-1].replace("'\\''", "'")
    if len(value) >= 2 and value[0] == '"' and value[-1] == '"':
        out = []
        escaped = False
        for ch in value[1:-1]:
            if escaped:
                out.append(_DOUBLE_QUOTE_ESCAPES.get(ch, ch))
                escaped = False
            elif ch == "\\":
                escaped = True
            else:
                out.append(ch)
        if escaped:
            out.append("\\")
        return "".join(out)
    return value


def _parse_env_file(path):
    env = {}
    with open(path, "r", encoding="utf-8") as f:
        for lineno, raw in enumerate(f, 1):
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            if line.startswith("export "):
                line = line[len("export "):].lstrip()
            if "=" not in line:
                raise ValueError(f"{path}:{lineno}: expected KEY=VALUE")
            key, value = line.split("=", 1)
            key = key.strip()
            if not key:
                raise ValueError(f"{path}:{lineno}: empty variable name")
            env[key] = _unquote(value.strip())
    return env


def main():
    if len(sys.argv) < 2:
        sys.stderr.write("run_rustc.py: missing compiler command\n")
        return 1

    args = sys.argv[1:]
    env = dict(os.environ)
    for name in _INTERNAL_VARS:
        env.pop(name, None)

    env_file = os.environ.get(_ENV_FILE_VAR)
    if env_file:
        try:
            env.update(_parse_env_file(env_file))
        except OSError as e:
            sys.stderr.write(f"run_rustc.py: cannot read {env_file}: {e}\n")
            return 1
        except ValueError as e:
            sys.stderr.write(f"run_rustc.py: {e}\n")
            return 1

    try:
        os.execvpe(args[0], args, env)
    except OSError as e:
        sys.stderr.write(f"run_rustc.py: cannot exec {args[0]}: {e}\n")
        return 1


if __name__ == "__main__":
    sys.exit(main())
