/**
 * Reads the *table of contents* of a release archive, without unpacking it.
 *
 * This exists because of a specific failure. Relay's installer downloads an
 * archive and spawns an executable out of it, and for four releases the
 * catalogue pointed at a project that had stopped shipping one: `OHF-Voice/
 * piper1-gpl` publishes `piper_tts-1.7.0-cp39-abi3-win_amd64.whl`, a Python
 * wheel. A wheel is a zip. It downloads, it hashes, it extracts — and there
 * is no `piper.exe` anywhere inside it.
 *
 * A filename match cannot tell those apart. Listing the entries can. So the
 * generator opens every artifact it pins and asserts the executable the
 * manifest promises is really in there, is not empty, and on Unix is marked
 * runnable. "The asset is named correctly" becomes "the archive contains an
 * engine", which is the claim the manifest is actually making.
 *
 * Deliberately dependency-free: this runs as a release step from a clean
 * checkout, and reading a central directory or walking 512-byte tar headers
 * is not worth a supply-chain surface. Nothing here decompresses a zip
 * member — only the central directory is parsed — so a hostile archive
 * cannot make the reader do work proportional to its uncompressed size.
 */

import zlib from 'node:zlib';

/** Mirrors the installer's own cap on what an archive may expand to. */
export const MAX_EXTRACTED_BYTES = 1024 * 1024 * 1024;

const ZIP_EOCD_SIGNATURE = 0x06054b50;
const ZIP_CENTRAL_SIGNATURE = 0x02014b50;
const ZIP_EOCD64_LOCATOR_SIGNATURE = 0x07064b50;
/** The end-of-central-directory record, plus the largest possible comment. */
const ZIP_EOCD_SEARCH_LIMIT = 22 + 0xffff;

export class ArchiveError extends Error {}

/**
 * Entries in a zip, by path.
 *
 * Reads the central directory rather than the local headers: it is the
 * authoritative index, and it is what an extractor should agree with.
 */
export function indexZip(bytes) {
  const eocd = findEndOfCentralDirectory(bytes);
  const entryCount = bytes.readUInt16LE(eocd + 10);
  const directoryOffset = bytes.readUInt32LE(eocd + 16);

  // Zip64 is not something a 20 MB engine archive needs, and guessing at a
  // format we have not implemented is worse than saying so.
  if (entryCount === 0xffff || directoryOffset === 0xffffffff) {
    throw new ArchiveError('this is a zip64 archive, which this reader does not parse');
  }
  if (findSignatureBackwards(bytes, ZIP_EOCD64_LOCATOR_SIGNATURE, eocd) !== -1) {
    throw new ArchiveError('this is a zip64 archive, which this reader does not parse');
  }

  const entries = new Map();
  let cursor = directoryOffset;

  for (let index = 0; index < entryCount; index += 1) {
    if (cursor + 46 > bytes.length || bytes.readUInt32LE(cursor) !== ZIP_CENTRAL_SIGNATURE) {
      throw new ArchiveError(`central directory entry ${index} is malformed`);
    }
    const nameLength = bytes.readUInt16LE(cursor + 28);
    const extraLength = bytes.readUInt16LE(cursor + 30);
    const commentLength = bytes.readUInt16LE(cursor + 32);
    const externalAttributes = bytes.readUInt32LE(cursor + 38);
    const size = bytes.readUInt32LE(cursor + 24);
    const name = bytes.toString('utf8', cursor + 46, cursor + 46 + nameLength);

    // The high 16 bits carry the Unix mode when the zip was made on a Unix
    // host. Windows-made zips leave them zero, which is why an absent
    // executable bit is never treated as a failure for a zip.
    const unixMode = (externalAttributes >>> 16) & 0o7777;

    entries.set(normalise(name), {
      path: normalise(name),
      size,
      mode: unixMode || null,
      isDirectory: name.endsWith('/'),
      isLink: false,
    });
    cursor += 46 + nameLength + extraLength + commentLength;
  }

  return entries;
}

/** Entries in a gzipped tar, by path. */
export function indexTarGz(bytes) {
  let tar;
  try {
    tar = zlib.gunzipSync(bytes, { maxOutputLength: MAX_EXTRACTED_BYTES });
  } catch (error) {
    throw new ArchiveError(`could not decompress: ${error.message}`);
  }
  return indexTar(tar);
}

/** Entries in an uncompressed tar, by path. */
export function indexTar(tar) {
  const entries = new Map();
  let offset = 0;
  // A GNU long-name entry describes the *next* header, so it has to be
  // carried across one iteration.
  let pendingLongName = null;

  while (offset + 512 <= tar.length) {
    const header = tar.subarray(offset, offset + 512);
    // Two zero blocks end the archive; one is enough to stop reading.
    if (header.every((byte) => byte === 0)) break;

    const magic = header.toString('ascii', 257, 262);
    if (magic !== 'ustar') {
      throw new ArchiveError(`tar header at ${offset} is not ustar`);
    }

    const size = readOctal(header, 124, 12);
    const typeFlag = String.fromCharCode(header[156] || 0x30);
    const dataBlocks = Math.ceil(size / 512);
    const dataStart = offset + 512;

    if (typeFlag === 'L') {
      // GNU long name: the payload is the next entry's path.
      pendingLongName = tar
        .toString('utf8', dataStart, dataStart + size)
        .replace(/\0+$/, '');
      offset = dataStart + dataBlocks * 512;
      continue;
    }

    const prefix = readString(header, 345, 155);
    const name = readString(header, 0, 100);
    const path = pendingLongName ?? (prefix ? `${prefix}/${name}` : name);
    pendingLongName = null;

    entries.set(normalise(path), {
      path: normalise(path),
      size,
      mode: readOctal(header, 100, 8) & 0o7777,
      isDirectory: typeFlag === '5' || path.endsWith('/'),
      isLink: typeFlag === '1' || typeFlag === '2',
      linkTarget: typeFlag === '1' || typeFlag === '2' ? readString(header, 157, 100) : null,
    });

    offset = dataStart + dataBlocks * 512;
  }

  if (entries.size === 0) {
    throw new ArchiveError('the tar contains no entries');
  }
  return entries;
}

/** Dispatches on the manifest's declared archive kind. */
export function indexArchive(kind, bytes) {
  switch (kind) {
    case 'zip':
      return indexZip(bytes);
    case 'tar_gz':
      return indexTarGz(bytes);
    default:
      throw new ArchiveError(`no reader for archive kind ${kind}`);
  }
}

/**
 * Checks that an archive really contains a runnable engine at `path`.
 *
 * Returns the entry. Throws with a message that says what *was* in there,
 * because "piper.exe is missing" and "this is a Python package" call for
 * very different fixes.
 */
export function requireExecutable(entries, path, { requireExecutableBit }) {
  const entry = entries.get(normalise(path));

  if (!entry || entry.isDirectory) {
    throw new ArchiveError(
      `the archive has no ${path}.\n${describeContents(entries)}`,
    );
  }
  if (entry.size === 0) {
    throw new ArchiveError(`${path} is empty in the archive`);
  }
  // Only meaningful for a tar, and for a zip built on a Unix host. A zip
  // made on Windows carries no modes at all, and demanding one there would
  // reject a perfectly good artifact.
  if (requireExecutableBit && entry.mode !== null && (entry.mode & 0o111) === 0) {
    throw new ArchiveError(`${path} is not marked executable in the archive`);
  }
  return entry;
}

/** A short, useful account of an archive that turned out to be the wrong thing. */
export function describeContents(entries) {
  const paths = [...entries.keys()];
  const pythonish = paths.filter((p) => p.endsWith('.py') || p.endsWith('.pyd')).length;
  const lines = [];

  if (pythonish > 0) {
    lines.push(
      `  It holds ${pythonish} Python file(s) — this looks like a Python package ` +
        '(a wheel), not a standalone program. A wheel needs an interpreter and ' +
        'its dependencies installed; Relay downloads an executable and runs it.',
    );
  }
  const sample = paths.filter((p) => !p.endsWith('/')).slice(0, 8);
  lines.push(`  ${entries.size} entries, including:`);
  for (const path of sample) lines.push(`    ${path}`);
  if (paths.length > sample.length) lines.push('    …');
  return lines.join('\n');
}

function normalise(path) {
  return path.replace(/\\/g, '/').replace(/^\.\//, '').replace(/\/+$/, '');
}

function readString(header, start, length) {
  const raw = header.subarray(start, start + length);
  const end = raw.indexOf(0);
  return raw.toString('utf8', 0, end === -1 ? raw.length : end);
}

function readOctal(header, start, length) {
  const text = readString(header, start, length).trim();
  if (text === '') return 0;
  const value = Number.parseInt(text, 8);
  if (!Number.isFinite(value)) {
    throw new ArchiveError(`tar header field at ${start} is not octal: ${JSON.stringify(text)}`);
  }
  return value;
}

function findEndOfCentralDirectory(bytes) {
  const eocd = findSignatureBackwards(bytes, ZIP_EOCD_SIGNATURE, bytes.length);
  if (eocd === -1) {
    throw new ArchiveError('no end-of-central-directory record: this is not a zip');
  }
  return eocd;
}

function findSignatureBackwards(bytes, signature, before) {
  const floor = Math.max(0, before - ZIP_EOCD_SEARCH_LIMIT);
  for (let offset = before - 4; offset >= floor; offset -= 1) {
    if (bytes.readUInt32LE(offset) === signature) return offset;
  }
  return -1;
}
