"""Insert mip_level_count: 1 into Resource::Texture2d blocks in exec_validate.rs tests."""
from pathlib import Path

path = Path("crates/w3drs-render-graph/src/exec_validate.rs")
src = path.read_text(encoding="utf-8")
lines = src.splitlines(keepends=True)
out = []
i = 0
while i < len(lines):
    line = lines[i]
    if "Resource::Texture2d {" in line:
        out.append(line)
        i += 1
        depth = 1
        while i < len(lines) and depth > 0:
            cur = lines[i]
            depth += cur.count("{") - cur.count("}")
            if depth == 0:
                out.append(cur)
                i += 1
                break
            # last line before closing at depth 1: insert mip if missing
            peek = lines[i + 1] if i + 1 < len(lines) else ""
            next_depth = depth + peek.count("{") - peek.count("}")
            if next_depth == 0 and "mip_level_count" not in "".join(out[-20:]) + cur:
                indent = cur[: len(cur) - len(cur.lstrip())]
                out.append(cur)
                out.append(f"{indent}mip_level_count: 1,\n")
            else:
                out.append(cur)
            i += 1
        continue
    out.append(line)
    i += 1

text = "".join(out)
# indirect_dispatch after storage_buffers_read_group1
lines = text.splitlines(keepends=True)
out = []
i = 0
while i < len(lines):
    line = lines[i]
    out.append(line)
    if "storage_buffers_read_group1:" in line:
        # find next non-empty
        j = i + 1
        while j < len(lines) and lines[j].strip() == "":
            out.append(lines[j])
            j += 1
        if j < len(lines) and lines[j].strip() == "},":
            block = "".join(lines[max(0, i - 15) : j + 1])
            if "indirect_dispatch" not in block and "Pass::Compute" in block:
                indent = lines[j][: len(lines[j]) - len(lines[j].lstrip())]
                out.append(f"{indent}indirect_dispatch: None,\n")
    i += 1

text = "".join(out)
# Blit: destination line then add region before }
lines = text.splitlines(keepends=True)
out = []
i = 0
while i < len(lines):
    line = lines[i]
    if "Pass::Blit {" in line:
        out.append(line)
        i += 1
        depth = 1
        buf = [line]
        while i < len(lines) and depth > 0:
            cur = lines[i]
            buf.append(cur)
            depth += cur.count("{") - cur.count("}")
            if depth == 0:
                block_text = "".join(buf)
                if "region:" not in block_text:
                    # insert before final }
                    closing = buf[-1]
                    indent = closing[: len(closing) - len(closing.lstrip())]
                    for bline in buf[1:-1]:
                        out.append(bline)
                    out.append(f"{indent}region: None,\n")
                    out.append(closing)
                else:
                    out.extend(buf[1:])
                i += 1
                break
            i += 1
        continue
    i += 1
    if i <= len(lines) and (not out or out[-1] != line):
        pass
# The loop above is wrong - rewrite simpler second pass for Blit only

path.write_text(text, encoding="utf-8")
print("written")
