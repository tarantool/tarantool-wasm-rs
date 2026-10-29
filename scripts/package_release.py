#!/usr/bin/env python3
"""Package prebuilt runtime/SDK artifacts without changing the source checkout."""
import argparse
import gzip
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def run(*args):
    return subprocess.check_output(args, text=True)


def package_sdk(stage, wasm_tools):
    for language, pattern in (
        ('python', 'dist/twr_python_sdk-{version}-py3-none-any.whl'),
        ('rust', 'target/package/tarantool-wasm-{version}.crate'),
        ('go', 'dist/twr-go-sdk-{version}.tar.gz'),
        ('js', 'dist/twr-js-sdk-{version}.tgz'),
        ('csharp', 'dist/Tarantool.Wasm.{version}.nupkg'),
    ):
        source = ROOT / 'guests' / language
        if language == 'rust':
            version = tomllib.loads((source / 'Cargo.toml').read_text())['package']['version']
        else:
            manifest = json.loads((source / 'sdk-manifest.json').read_text())
            version = manifest['sdk_version']

        artifact = source / pattern.format(version=version)
        destination = stage / 'packages' / language
        destination.mkdir(parents=True)
        shutil.copyfile(artifact, destination / artifact.name)
        shutil.copyfile(ROOT / 'scripts/sdk-readmes' / (language + '.md'), destination / 'README.md')
        package_wit(source / 'wit', stage / 'wit' / language, language, wasm_tools)


def package_wit(source, destination, language, wasm_tools):
    with tempfile.TemporaryDirectory(prefix='twr-release-wit-') as temporary:
        inputs = Path(temporary) / 'inputs'
        shutil.copytree(source, inputs)
        if language in ('python', 'go'):
            shutil.copyfile(ROOT / 'wit/deps/wasi-cli-0.2.12.wasm', inputs / 'deps/wasi-cli.wasm')
        if language in ('python', 'js'):
            runtime = inputs / 'deps/twr-runtime'
            runtime.mkdir()
            if language == 'python':
                shutil.copyfile(
                    ROOT / 'guests/python/sdk/src/twr_python_sdk/wit/lua-callback.wit',
                    runtime / 'lua-callback.wit',
                )
                shutil.copyfile(ROOT / 'guests/python/ports/ssl/wit/ssl-bio.wit', runtime / 'ssl-bio.wit')
            else:
                shutil.copyfile(source / 'runtime/host.wit', runtime / 'host.wit')

        resolved = Path(temporary) / 'resolved'
        subprocess.run([
            wasm_tools, 'component', 'wit', str(inputs),
            '--all-features', '--out-dir', str(resolved),
        ], check=True)
        # Restore direct packages' feature annotations after merging the closure.
        dependencies = {
            path.read_text().splitlines()[0]: path
            for path in (resolved / 'deps').glob('*.wit')
        }
        for index, binary in enumerate(sorted((inputs / 'deps').glob('*.wasm'))):
            decoded = Path(temporary) / str(index)
            subprocess.run([
                wasm_tools, 'component', 'wit', str(binary),
                '--all-features', '--out-dir', str(decoded),
            ], check=True)
            package, = decoded.glob('*.wit')
            declaration = package.read_text().splitlines()[0]
            shutil.copyfile(package, dependencies[declaration])

        deps = destination / 'deps'
        deps.mkdir(parents=True)
        for dependency in sorted((resolved / 'deps').glob('*.wit')):
            target = deps / dependency.stem / 'package.wit'
            target.parent.mkdir()
            shutil.copyfile(dependency, target)

        for package in resolved.glob('*.wit'):
            shutil.copyfile(package, deps.parent / package.name)

    run(wasm_tools, 'component', 'wit', str(deps.parent))
    run(wasm_tools, 'component', 'wit', str(deps.parent), '--all-features')


def package_runtime(destination, module, platform):
    header = module.read_bytes()[:32]
    if platform == 'linux-x86_64':
        if (header[:6] != b'\x7fELF\x02\x01'
                or struct.unpack_from('<HH', header, 16) != (3, 62)):
            raise ValueError('Expected a Linux x86_64 ELF shared library')
        dependencies = run('readelf', '--dynamic', '--version-info', str(module))
        versions = re.findall(r'GLIBC_(\d+(?:\.\d+)+)', dependencies)
        if not versions:
            raise ValueError('Cannot determine the GLIBC requirement from the native module')
        minimum = max(versions, key=lambda value: tuple(map(int, value.split('.'))))
        if tuple(map(int, minimum.split('.'))) > (2, 28):
            raise ValueError(f'GLIBC {minimum} exceeds the release baseline 2.28; rebuild in EL8')
        needed = re.findall(r'\(NEEDED\).*?\[(.*?)\]', dependencies)
        print(f'GLIBC requirement: {minimum}')
    else:
        if (header[:4] != b'\xcf\xfa\xed\xfe'
                or struct.unpack_from('<I', header, 4)[0] != 0x0100000C
                or struct.unpack_from('<I', header, 12)[0] != 6):
            raise ValueError('Expected a macOS arm64 Mach-O dynamic library')
        dependencies = run('otool', '-L', str(module))
        needed = [line.strip().split(' (', 1)[0] for line in dependencies.splitlines()[2:]]
        load_commands = run('otool', '-l', str(module))
        minimum, = re.findall(r'^\s*minos ([\d.]+)$', load_commands, re.MULTILINE)
        print(f'macOS deployment target: {minimum}')

    if any(re.search(r'lib(?:ssl|crypto)[.\-]', name) for name in needed):
        raise ValueError('The native module must not link to OpenSSL shared libraries')
    shutil.copyfile(module, destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir', type=Path, default=ROOT / 'dist')
    commands = parser.add_subparsers(dest='kind', required=True)
    sdk = commands.add_parser('sdk', help='Package all five prebuilt SDKs and their WIT closure')
    sdk.add_argument('--wasm-tools', default='wasm-tools')
    runtime = commands.add_parser('runtime', help='Prepare an existing release native module for upload')
    runtime.add_argument('--module', type=Path, required=True)
    runtime.add_argument('--platform', choices=['linux-x86_64', 'macos-arm64'], required=True)
    args = parser.parse_args()

    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    suffix = 'sdk' if args.kind == 'sdk' else args.platform
    name = f'tarantool-wasm-{version}-{suffix}'
    extension = ('.tar.gz' if args.kind == 'sdk'
                 else '.so' if args.platform == 'linux-x86_64' else '.dylib')
    args.output_dir.mkdir(parents=True, exist_ok=True)

    # Stage privately; failed preparation preserves the previous output.
    with tempfile.TemporaryDirectory(prefix='.twr-release-', dir=args.output_dir) as temporary:
        output = Path(temporary) / (name + extension)
        if args.kind == 'runtime':
            package_runtime(output, args.module, args.platform)
            destination = args.output_dir / output.name
            output.replace(destination)
            print(destination)
            return

        stage = Path(temporary) / name
        stage.mkdir()
        package_sdk(stage, args.wasm_tools)
        epoch = int(os.environ.get('SOURCE_DATE_EPOCH', run(
            'git', '-C', str(ROOT), 'show', '-s', '--format=%ct', 'HEAD',
        ).strip()))
        with output.open('wb') as stream, gzip.GzipFile(
            filename='', mode='wb', fileobj=stream, mtime=epoch,
        ) as compressed:
            with tarfile.open(fileobj=compressed, mode='w') as archive:
                for path in [stage, *sorted(stage.rglob('*'))]:
                    info = archive.gettarinfo(str(path), arcname=path.relative_to(stage.parent).as_posix())
                    info.uid = info.gid = 0
                    info.uname = info.gname = ''
                    info.mtime = epoch
                    info.mode = 0o755 if path.is_dir() else 0o644
                    if path.is_file():
                        with path.open('rb') as content:
                            archive.addfile(info, content)
                    else:
                        archive.addfile(info)

        destination = args.output_dir / output.name
        output.replace(destination)
        print(destination)


if __name__ == '__main__':
    main()
