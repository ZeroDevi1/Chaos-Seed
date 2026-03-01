#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
把 CosyVoice 的 spk2info.pt 导出为 chaos-core 可读的 spk2info.json。

为什么需要这个工具？
- 上游 VoiceLab/CosyVoice 的 SFT 说话人信息通常是 torch 保存的 `spk2info.pt`
- Rust 侧不希望在推理链路中引入 torch 解析，因此改为读取一个简单 JSON：
  { "dream": { "embedding": [ ... 192 floats ... ] }, ... }

用法示例：
  python tools/export_spk2info_json_from_pt.py --input spk2info.pt --output spk2info.json
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Dict, List


def _to_f32_list(x: Any) -> List[float]:
    # torch.Tensor
    if hasattr(x, "detach") and hasattr(x, "cpu") and hasattr(x, "flatten"):
        t = x.detach().cpu().flatten()
        return [float(v) for v in t.tolist()]
    # list/tuple of numbers
    if isinstance(x, (list, tuple)):
        return [float(v) for v in x]
    raise TypeError(f"Unsupported embedding type: {type(x)}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", required=True, help="spk2info.pt 路径（torch.save 的文件）")
    ap.add_argument("--output", required=True, help="输出 spk2info.json 路径")
    args = ap.parse_args()

    in_path = Path(args.input).resolve()
    out_path = Path(args.output).resolve()

    if not in_path.exists():
        raise SystemExit(f"input not found: {in_path}")

    try:
        import torch  # type: ignore
    except Exception as e:  # pragma: no cover
        raise SystemExit(
            "missing dependency: torch. "
            "Please run this script in an environment where PyTorch is installed.\n"
            f"import torch failed: {e}"
        )

    obj = torch.load(in_path, map_location="cpu")
    if not isinstance(obj, dict):
        raise SystemExit(f"unexpected spk2info.pt format: expected dict, got {type(obj)}")

    out: Dict[str, Dict[str, List[float]]] = {}
    for spk_id, info in obj.items():
        if isinstance(info, dict) and "embedding" in info:
            emb = info["embedding"]
        else:
            # 兼容一些变体：直接保存 embedding tensor
            emb = info
        out[str(spk_id)] = {"embedding": _to_f32_list(emb)}

    out_path.parent.mkdir(parents=True, exist_ok=True)
    # UTF-8 无 BOM + LF：满足 repo 统一编码要求
    with out_path.open("w", encoding="utf-8", newline="\n") as f:
        json.dump(out, f, ensure_ascii=False, indent=2)
        f.write("\n")

    print(f"wrote: {out_path} (spk_count={len(out)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

