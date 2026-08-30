/**
 * Minimal zip and tar writers, for testing the archive readers.
 *
 * Written by hand for the same reason the readers are: a test that builds
 * its fixtures with the same library the code under test uses proves the
 * two agree, not that either is right. These emit the bytes by the format's
 * own layout, so a reader that only works on archives this repo produced
 * would fail here.
 */

import zlib from 'node:zlib';

/** A zip with stored (uncompressed) members. */
export function makeZip(entries) {
  const locals = [];
  const centrals = [];
  let offset = 0;

  for (const { name, body = Buffer.alloc(0), mode = null, directory = false } of entries) {
    const nameBytes = Buffer.from(directory && !name.endsWith('/') ? `${name}/` : name, 'utf8');
    const data = Buffer.from(body);
    const crc = zlib.crc32 ? zlib.crc32(data) : 0;

    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4); // version needed
    local.writeUInt16LE(0, 8); // stored
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    locals.push(local, nameBytes, data);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(mode === null ? 0 : 3 << 8, 4); // 3 = Unix, for the mode bits
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0, 10);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(mode === null ? 0 : (mode & 0o7777) << 16, 38);
    central.writeUInt32LE(offset, 42);
    centrals.push(central, nameBytes);

    offset += local.length + nameBytes.length + data.length;
  }

  const localBytes = Buffer.concat(locals);
  const centralBytes = Buffer.concat(centrals);
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(entries.length, 8);
  eocd.writeUInt16LE(entries.length, 10);
  eocd.writeUInt32LE(centralBytes.length, 12);
  eocd.writeUInt32LE(localBytes.length, 16);

  return Buffer.concat([localBytes, centralBytes, eocd]);
}

/**
 * A ustar tar.
 *
 * `longName: true` emits the entry through a GNU `L` record instead of the
 * 100-byte name field, which is how any real archive carries a deep path.
 */
export function makeTar(entries) {
  const blocks = [];

  for (const entry of entries) {
    const { name, body = Buffer.alloc(0), mode = 0o644, type = '0', link = '', longName = false } = entry;
    if (longName) {
      const nameBytes = Buffer.from(`${name}\0`, 'utf8');
      blocks.push(tarHeader({ name: '././@LongLink', size: nameBytes.length, mode: 0o644, type: 'L' }));
      blocks.push(pad(nameBytes));
    }
    const data = Buffer.from(body);
    blocks.push(
      tarHeader({
        name: longName ? name.slice(0, 100) : name,
        size: type === '0' ? data.length : 0,
        mode,
        type,
        link,
      }),
    );
    if (type === '0') blocks.push(pad(data));
  }

  // Two zero blocks terminate an archive.
  blocks.push(Buffer.alloc(1024));
  return Buffer.concat(blocks);
}

export function makeTarGz(entries) {
  return zlib.gzipSync(makeTar(entries));
}

function tarHeader({ name, size, mode, type, link = '' }) {
  const header = Buffer.alloc(512);
  header.write(name, 0, 100, 'utf8');
  header.write(octal(mode, 7), 100, 8, 'ascii');
  header.write(octal(0, 7), 108, 8, 'ascii'); // uid
  header.write(octal(0, 7), 116, 8, 'ascii'); // gid
  header.write(octal(size, 11), 124, 12, 'ascii');
  header.write(octal(0, 11), 136, 12, 'ascii'); // mtime
  header.write('        ', 148, 8, 'ascii'); // checksum placeholder: spaces
  header.write(type, 156, 1, 'ascii');
  header.write(link, 157, 100, 'utf8');
  header.write('ustar\0' + '00', 257, 8, 'ascii');

  let sum = 0;
  for (const byte of header) sum += byte;
  header.write(`${octal(sum, 6)}\0 `, 148, 8, 'ascii');
  return header;
}

function octal(value, width) {
  return value.toString(8).padStart(width, '0');
}

function pad(data) {
  const padding = (512 - (data.length % 512)) % 512;
  return padding === 0 ? data : Buffer.concat([data, Buffer.alloc(padding)]);
}
