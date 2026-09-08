# jianying-auto

把本地视频素材和文案排进剪映时间线，写出可打开的草稿目录。

剪映没有公开剪辑 API。本工具按剪映 5.9 明文草稿格式生成 `draft_content.json` / `draft_meta_info.json`，用文案长度估算每句时长，并把镜头裁切、变速、铺满画布、底部字幕写进工程。你在剪映里打开草稿后再微调、配乐、导出。

## 用法

```bash
cd rust
cargo run -p jianying-auto -- \
  --media /path/to/clips \
  --script /path/to/script.txt \
  --name 自动成片 \
  --ratio 9:16 \
  --output ./jianying-draft \
  --head-trim 0.3 \
  --mute
```

把输出目录复制到剪映「全局设置 → 草稿位置」对应的文件夹（常见路径）：

- Windows: `%LOCALAPPDATA%/JianyingPro/User Data/Projects/com.lveditor.draft`
- macOS: `~/Movies/JianyingPro Drafts`

或直接：

```bash
cargo run -p jianying-auto -- --media ./clips --script script.txt --install "$JIANYING_DRAFTS"
```

本机没有 ffprobe 时加 `--assume-seconds 8`。在 Linux 上生成、到 Windows 打开时，用 `--rewrite-from` / `--rewrite-to` 把素材路径改成剪映电脑上的绝对路径。

## 时间线规则

- 纯文本：按段落/句号切开字幕，按 `--chars-per-second` 分配时长，镜头按文件名顺序循环裁切填满。
- SRT：沿用字幕时间码，镜头仍按顺序填槽。
- 没有文案：镜头按文件名顺序拼接，并做片头/片尾裁切。
- 镜头不够长时自动变速以对齐口播。

新版剪映可能把已有草稿加密；**新建明文草稿多数版本仍能导入**。若打不开，用剪映 5.9 / 专业版，或把 JSON 作为模板再手工另存。
