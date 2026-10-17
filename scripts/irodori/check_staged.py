"""Check staged tokenizer and watermark cache resolution, without loading TTS weights.

Use the inspected Irodori virtualenv Python with -I -B. This creates only Sungeki
cache directories; it does not attest hashes or prove full dependency coverage.
"""
import argparse
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'workers/irodori'))
from environment import configure


def check(root):
    if not root.is_absolute() or root.resolve(strict=True) != root:
        raise ValueError('canonical staging directory required')
    report = json.loads((root / 'staging-report.json').read_text())
    if report['format_version'] != 1 or report['method'] != 'clonefile':
        raise ValueError('completed clone staging required')
    model = root / 'originals/model'
    codec = root / 'originals/codec'
    configure(root / 'work', [model, codec], root / 'originals/hf-hub')
    from huggingface_hub import snapshot_download
    watermark = Path(snapshot_download(repo_id='sony/silentcipher', local_files_only=True))
    expected_root = root / 'originals/hf-hub/models--sony--silentcipher/snapshots'
    if watermark.resolve().parent != expected_root:
        raise ValueError('watermark cache resolved outside Sungeki staging')
    recorded = {Path(item['path']) for item in report['files'] if item['role'] == 'watermark'}
    found = {p for p in watermark.rglob('*') if p.is_file()}
    if found != recorded:
        raise ValueError('watermark cache files differ from staging report')
    result = dict(format_version=1, tokenizer_loaded=False,
        watermark_snapshot=str(watermark), watermark_files=len(found),
        watermark_cache_resolved=True, watermark_model_loaded=False, tts_model_loaded=False,
        hashes_rechecked=False, dependency_completeness_checked=False)
    try:
        from transformers import AutoTokenizer
        tokenizer = AutoTokenizer.from_pretrained(str(model / 'tokenizer'),
            local_files_only=True, trust_remote_code=False, use_fast=True)
        tokens = tokenizer.encode('音声の動作確認です。', add_special_tokens=False)
        if not tokens or tokenizer.bos_token_id is None or tokenizer.pad_token_id is None:
            raise ValueError('tokenizer is missing required tokens')
        result.update(tokenizer_loaded=True, token_count=len(tokens),
                      bos_token_id=tokenizer.bos_token_id, pad_token_id=tokenizer.pad_token_id)
    except Exception as error:
        result['tokenizer_error'] = f'{type(error).__name__}: {error}'
    return result



if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('staging_directory', type=Path)
    args = parser.parse_args()
    result = check(args.staging_directory)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    sys.exit(0 if result['tokenizer_loaded'] else 1)
