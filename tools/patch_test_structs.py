import re
from pathlib import Path


def is_test_id_line(line: str) -> bool:
    return bool(re.search(r'id:\s*"[^"]+"\.into\(\)', line))


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    p = root / "crates/w3drs-render-graph/src/exec_validate.rs"
    src = p.read_text(encoding="utf-8")
    lines = src.splitlines(keepends=True)

    out: list[str] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if (
            "Resource::Texture2d {" in line
            and "let Some" not in line
            and "matches!" not in line
            and i + 1 < len(lines)
            and is_test_id_line(lines[i + 1])
        ):
            out.append(line)
            i += 1
            depth = 1
            chunk: list[str] = []
            while i < len(lines) and depth > 0:
                cur = lines[i]
                depth += cur.count("{") - cur.count("}")
                chunk.append(cur)
                i += 1
            block = "".join(chunk)
            if "mip_level_count" not in block:
                last = chunk[-1]
                indent = last[: len(last) - len(last.lstrip())]
                chunk.insert(-1, f"{indent}mip_level_count: 1,\n")
            out.extend(chunk)
            continue
        out.append(line)
        i += 1
    lines = out

    out = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if "Pass::Compute {" in line and i + 1 < len(lines) and is_test_id_line(lines[i + 1]):
            out.append(line)
            i += 1
            depth = 1
            chunk = []
            while i < len(lines) and depth > 0:
                cur = lines[i]
                depth += cur.count("{") - cur.count("}")
                chunk.append(cur)
                i += 1
            block = "".join(chunk)
            if "indirect_dispatch" not in block:
                last = chunk[-1]
                indent = last[: len(last) - len(last.lstrip())]
                chunk.insert(-1, f"{indent}indirect_dispatch: None,\n")
            out.extend(chunk)
            continue
        out.append(line)
        i += 1
    lines = out

    out = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if "Pass::Blit {" in line and i + 1 < len(lines) and is_test_id_line(lines[i + 1]):
            out.append(line)
            i += 1
            depth = 1
            chunk = []
            while i < len(lines) and depth > 0:
                cur = lines[i]
                depth += cur.count("{") - cur.count("}")
                chunk.append(cur)
                i += 1
            block = "".join(chunk)
            if "region:" not in block:
                last = chunk[-1]
                indent = last[: len(last) - len(last.lstrip())]
                chunk.insert(-1, f"{indent}region: None,\n")
            out.extend(chunk)
            continue
        out.append(line)
        i += 1

    p.write_text("".join(out), encoding="utf-8")
    print("patched", p)


if __name__ == "__main__":
    main()
