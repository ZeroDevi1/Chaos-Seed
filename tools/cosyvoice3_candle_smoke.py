#!/usr/bin/env python3
"""
CosyVoice3（Candle / 纯 Rust 推理引擎）最小化冒烟验证脚本（Windows 优先）。

用途：
- 验证 third_party/cosyvoice3.rs 的 PyO3 扩展是否能正常加载
- 验证使用 convert_weights.py 产出的 Candle 权重目录是否能端到端出 wav

说明：
- 这是“路线2”的最小验证，不追求完全对齐 VoiceLab 的 SFT 推理接口（spk_id），
  这里只跑 cosyvoice3.rs 暴露的 zero-shot（需要 prompt_wav）。
"""

from __future__ import annotations

import argparse
import struct
import wave
from pathlib import Path


def save_wav_mono_i16(path: Path, audio_f32: list[float], sample_rate: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "w") as f:
        f.setnchannels(1)
        f.setsampwidth(2)
        f.setframerate(sample_rate)
        audio_i16 = [int(max(-32768, min(32767, s * 32767))) for s in audio_f32]
        f.writeframes(struct.pack(f"{len(audio_i16)}h", *audio_i16))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--model-dir", required=True, help="convert_weights.py 输出的目录")
    ap.add_argument("--prompt-wav", required=True, help="用于 zero-shot 的提示音频（wav/mp3/ogg）")
    ap.add_argument("--text", required=True, help="要合成的文本")
    ap.add_argument(
        "--mode",
        default="instruct",
        choices=["instruct", "cross_lingual", "zero_shot"],
        help="推理模式：instruct（推荐做最小验证）、cross_lingual、zero_shot",
    )
    ap.add_argument(
        "--prompt-text",
        default="You are a helpful assistant.<|endofprompt|>",
        help="提示/指令文本（instruct/zero_shot 用；zero_shot 里应与 prompt_wav 内容匹配）",
    )
    ap.add_argument("--out-wav", required=True, help="输出 wav 路径")
    ap.add_argument("--timesteps", type=int, default=10, help="flow 采样步数（默认 10）")
    ap.add_argument("--use-f16", action="store_true", help="使用 f16（CPU 上一般不建议）")
    args = ap.parse_args()

    # 延迟 import，便于在缺少扩展时给出更明确错误信息
    from cosyvoice3 import CosyVoice3, PyDevice

    model_dir = Path(args.model_dir).resolve()
    prompt_wav = Path(args.prompt_wav).resolve()
    out_wav = Path(args.out_wav).resolve()

    if not model_dir.exists():
        raise SystemExit(f"model_dir not found: {model_dir}")
    if not prompt_wav.exists():
        raise SystemExit(f"prompt_wav not found: {prompt_wav}")

    device = PyDevice.best_available()
    print(f"[cosyvoice3] device={device}")
    print(f"[cosyvoice3] loading model_dir={model_dir}")
    model = CosyVoice3(str(model_dir), device=device, use_f16=bool(args.use_f16))
    print(f"[cosyvoice3] sample_rate={model.sample_rate} has_onnx={model.has_onnx}")

    if args.mode == "cross_lingual":
        print(f"[cosyvoice3] inference_cross_lingual: timesteps={args.timesteps}")
        audio = model.inference_cross_lingual(
            text=args.text,
            prompt_wav=str(prompt_wav),
            n_timesteps=int(args.timesteps),
        )
    elif args.mode == "zero_shot":
        print(f"[cosyvoice3] inference_zero_shot: timesteps={args.timesteps}")
        audio = model.inference_zero_shot(
            text=args.text,
            prompt_text=args.prompt_text,
            prompt_wav=str(prompt_wav),
            n_timesteps=int(args.timesteps),
        )
    else:
        print(f"[cosyvoice3] inference_instruct: timesteps={args.timesteps}")
        audio = model.inference_instruct(
            text=args.text,
            instruct_text=args.prompt_text,
            prompt_wav=str(prompt_wav),
            n_timesteps=int(args.timesteps),
        )

    print(f"[cosyvoice3] audio_len={len(audio)} samples")
    save_wav_mono_i16(out_wav, audio, model.sample_rate)
    print(f"[cosyvoice3] wrote wav: {out_wav}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
