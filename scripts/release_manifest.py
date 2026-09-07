"""Validate package versions; generate per-target release provenance metadata."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--artifact')
    parser.add_argument('--target')
    parser.add_argument('--tag')
    args = parser.parse_args()
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    for package in ['npm/package.json', 'scoop/noctune.json']:
        assert json.loads((ROOT / package).read_text())['version'] == version, package
    if args.tag:
        assert args.tag == f'v{version}', 'Tag must match package versions'
    if args.artifact:
        artifact = ROOT / args.artifact
        assert artifact.parent == ROOT and artifact.is_file(), 'Invalid artifact'
        digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
        Path(str(artifact) + '.sha256').write_text(f'{digest}  {artifact.name}\n', encoding='utf-8')
        metadata = dict(version=version, target=args.target, filename=artifact.name,
                        sha256=digest, size=artifact.stat().st_size, commit=os.getenv('GITHUB_SHA'))
        Path(str(artifact) + '.json').write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    print(f'Versions consistent: {version}')

if __name__ == '__main__':
    main()
