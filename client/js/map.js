// Parses map.bin (BTMP v1) into a wall bitset.
export function parseMap(buf) {
  const v = new DataView(buf);
  const size = v.getUint16(5, true);
  const count = v.getUint32(7, true);
  const bits = new Uint8Array((size * size) >> 3);
  for (let k = 0, o = 11; k < count; k++, o += 6) {
    const x0 = v.getUint16(o, true), y0 = v.getUint16(o + 2, true), w = v.getUint8(o + 4), h = v.getUint8(o + 5);
    for (let y = y0; y < y0 + h; y++) for (let x = x0; x < x0 + w; x++) {
      const i = y * size + x;
      bits[i >> 3] |= 1 << (i & 7);
    }
  }
  let version = 0x811c9dc5;
  for (const b of new Uint8Array(buf)) version = Math.imul(version ^ b, 0x01000193) >>> 0;
  const isWall = (x, y) => {
    if (x < 0 || y < 0 || x >= size || y >= size) return true;
    const i = y * size + x;
    return (bits[i >> 3] & (1 << (i & 7))) !== 0;
  };
  return { size, version, isWall };
}
