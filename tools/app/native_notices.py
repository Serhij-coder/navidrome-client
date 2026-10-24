"""Fail-closed Ubuntu native ELF provenance for the staged development AppDir."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess


def digest(path):
    with Path(path).open('rb') as handle:
        return hashlib.file_digest(handle, 'sha256').hexdigest()


def command(args):
    try:
        return subprocess.check_output(args, text=True).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise ValueError(f'native provenance command failed: {args[0]}') from error


def elf(path):
    with path.open('rb') as handle:
        if handle.read(4) != b'\x7fELF':
            return None
    header = command(['readelf', '-h', str(path)])
    notes = command(['readelf', '-n', str(path)])
    machine = re.search(r'Machine:\s*(.+)', header)
    build = re.search(r'Build ID:\s*([0-9a-fA-F]+)', notes)
    if not machine or not build:
        raise ValueError(f'ELF lacks machine/build ID: {path}')
    return {'machine': machine.group(1).strip(), 'build_id': build.group(1).lower()}


def registered_sources(listing):
    """Only inspect absolute paths explicitly registered by ldconfig."""
    result = set()
    for line in listing.splitlines():
        if '=>' in line:
            path = Path(line.split('=>', 1)[1].strip())
            if not path.is_absolute():
                raise ValueError('ldconfig returned a nonabsolute path')
            if path.is_file():
                result.add(path)
    return sorted(result)


def package(path):
    owners = command(['dpkg-query', '-S', str(path)]).splitlines()
    names = {line.rsplit(': ', 1)[0] for line in owners if ': ' in line}
    if len(names) != 1:
        raise ValueError(f'ambiguous or missing dpkg owner: {path}')
    name = names.pop()
    fields = command(['dpkg-query', '-W', '-f=${binary:Package}\t${Version}\t${source:Package}\t${source:Version}\t${Architecture}', name]).split('\t')
    if len(fields) != 5 or not all(fields):
        raise ValueError(f'incomplete dpkg provenance: {name}')
    return dict(zip(('binary_package', 'binary_version', 'source_package', 'source_version', 'architecture'), fields))


def contained(path, root):
    resolved = path.resolve(strict=True)
    if not resolved.is_relative_to(root):
        raise ValueError(f'AppDir path escapes staging directory: {path}')
    return resolved


def collect(appdir, source_binary, output, listing, tool_sha256, copyright_root=Path('/usr/share/doc')):
    appdir = appdir.resolve()
    source_binary = source_binary.resolve(strict=True)
    source_elf = elf(source_binary)
    if not source_elf:
        raise ValueError('project source binary is not ELF')
    # Require dpkg even if an unusually small bundle contains only the app.
    command(['dpkg-query', '--version'])
    bundled_names = {path.resolve().name for path in appdir.rglob('*')
                     if (path.is_file() or path.is_symlink()) and contained(path, appdir).is_file()}
    candidates = {}
    for source in registered_sources(listing):
        if not {source.name, source.resolve().name} & bundled_names:
            continue
        identity = elf(source)
        if identity:
            for name in {source.name, source.resolve().name}:
                key = (name, identity['machine'], identity['build_id'])
                matches = candidates.setdefault(key, set())
                matches.add(source.resolve())
    output.mkdir(parents=True, exist_ok=True)
    records = []
    for path in sorted(appdir.rglob('*')):
        if not path.is_file() and not path.is_symlink():
            continue
        resolved = contained(path, appdir)
        identity = elf(resolved) if resolved.is_file() else None
        if not identity:
            continue
        record = {'path': path.relative_to(appdir).as_posix(), **identity,
                  'bundled_sha256': digest(resolved)}
        if path.is_symlink():
            record['symlink_target'] = path.readlink().as_posix()
        if resolved == appdir / 'usr/bin' / source_binary.name:
            if identity != source_elf:
                raise ValueError('bundled project executable identity changed')
            record.update(kind='project', source_path=str(source_binary), source_sha256=digest(source_binary), notice='../LICENSE')
        else:
            matches = candidates.get((resolved.name, identity['machine'], identity['build_id']), [])
            if len(matches) != 1:
                raise ValueError(f'unmapped or ambiguous bundled ELF: {path}')
            source = next(iter(matches))
            metadata = package(source)
            notice = copyright_root / metadata['binary_package'].split(':')[0] / 'copyright'
            if not notice.is_file():
                raise ValueError(f'missing exact package copyright: {notice}')
            notice_name = metadata['binary_package'].replace(':', '_') + '/copyright'
            destination = output / notice_name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(notice, destination)
            record.update(kind='distribution', source_path=str(source), source_sha256=digest(source),
                          package=metadata, copyright=notice_name, copyright_source_path=str(notice),
                          copyright_sha256=digest(destination))
        records.append(record)
    manifest = {'schema': 1, 'scope': 'Ubuntu 24.04 development native ELF provenance; not a license/source-obligation audit',
                'linuxdeploy_sha256': tool_sha256, 'files': records}
    (output / 'inventory.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    return manifest


def verify(appdir, manifest):
    appdir = appdir.resolve()
    expected = {row['path']: row for row in manifest['files']}
    actual = set()
    for path in appdir.rglob('*'):
        if path.is_file() or path.is_symlink():
            resolved = contained(path, appdir)
            if resolved.is_file() and elf(resolved):
                relative = path.relative_to(appdir).as_posix()
                actual.add(relative)
                row = expected.get(relative)
                if row is None or digest(resolved) != row['bundled_sha256'] or (path.readlink().as_posix() if path.is_symlink() else None) != row.get('symlink_target'):
                    raise ValueError(f'native ELF changed during finalization: {relative}')
    if actual != set(expected):
        raise ValueError('native ELF inventory changed during finalization')
