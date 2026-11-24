# OCR model files

Optional downloads land in the user models directory (`DOCAPP_MODELS`, or `~/.local/share/docapp/models` on Linux).

| File | Size (approx.) | Purpose |
|------|----------------|---------|
| `text-detection.onnx` | 2.4 MB | ocrs text detection (printed text) |
| `text-recognition.onnx` | 9.3 MB | ocrs line recognition (printed text) |
| `handwriting-recognition.onnx` | 12.4 MB | CTC handwriting word recognition |

## Sources and licenses

| File | Upstream | License |
|------|----------|---------|
| `text-detection.onnx` | [ocrs](https://github.com/robertknight/ocrs) project models | MIT |
| `text-recognition.onnx` | [ocrs](https://github.com/robertknight/ocrs) project models | MIT |
| `handwriting-recognition.onnx` | [PellelNitram/xournalpp-htr-simple-htr](https://huggingface.co/PellelNitram/xournalpp-htr-simple-htr) (SimpleHTR, IAM-trained) | MIT |

## Distribution

Canonical copies ship in the GitHub release [models-v1](https://github.com/Emmanue707/docapp/releases/tag/models-v1). The app fetches from that release first; upstream mirrors are used as fallbacks when a file is missing.

`sha256` values are pinned in `docsuite-ocr` (`DETECTION_SHA256`, `RECOGNITION_SHA256`, `HANDWRITING_SHA256`).