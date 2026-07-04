import fs from "fs";

class BinaryInput {
  constructor(data) {
    this.index = 0;
    this.strings = [];
    this.buffer = new DataView(data.buffer, data.byteOffset, data.byteLength);
  }
  readByte() {
    return this.buffer.getInt8(this.index++);
  }
  readUnsignedByte() {
    return this.buffer.getUint8(this.index++);
  }
  readInt32() {
    const v = this.buffer.getInt32(this.index, true);
    this.index += 4;
    return v;
  }
  readInt(opt) {
    let b = this.readByte(),
      r = b & 0x7f;
    if (b & 0x80) {
      b = this.readByte();
      r |= (b & 0x7f) << 7;
      if (b & 0x80) {
        b = this.readByte();
        r |= (b & 0x7f) << 14;
        if (b & 0x80) {
          b = this.readByte();
          r |= (b & 0x7f) << 21;
          if (b & 0x80) r |= (this.readByte() & 0x7f) << 28;
        }
      }
    }
    return opt ? r : (r >>> 1) ^ -(r & 1);
  }
  readString() {
    let n = this.readInt(true);
    if (n === 0) return null;
    if (n === 1) return "";
    n--;
    let s = "";
    for (let i = 0; i < n; ) {
      const b = this.readUnsignedByte();
      switch (b >> 4) {
        case 12:
        case 13:
          s += String.fromCharCode(((b & 0x1f) << 6) | (this.readByte() & 0x3f));
          i += 2;
          break;
        case 14:
          s += String.fromCharCode(
            ((b & 0x0f) << 12) |
              ((this.readByte() & 0x3f) << 6) |
              (this.readByte() & 0x3f),
          );
          i += 3;
          break;
        default:
          s += String.fromCharCode(b);
          i++;
      }
    }
    return s;
  }
  readFloat() {
    const v = this.buffer.getFloat32(this.index, true);
    this.index += 4;
    return v;
  }
  readBoolean() {
    return this.readByte() !== 0;
  }
  readStringRef() {
    const idx = this.readInt(true);
    return idx === 0 ? null : this.strings[idx - 1];
  }
}

function parse36(label, withStringsBeforeBones) {
  const input = new BinaryInput(
    new Uint8Array(fs.readFileSync("public/assets/pet/chaijun/chaijun.skel")),
  );
  input.readString();
  input.readString();
  input.readFloat();
  input.readFloat();
  input.readBoolean();
  if (withStringsBeforeBones) {
    const n = input.readInt(true);
    console.log(label, "strings before bones", n);
    for (let i = 0; i < n; i++) input.strings.push(input.readString());
  }
  const boneCount = input.readInt(true);
  for (let i = 0; i < boneCount; i++) {
    input.readString();
    if (i > 0) input.readInt(true);
    for (let j = 0; j < 8; j++) input.readFloat();
    input.readByte();
  }
  if (!withStringsBeforeBones) {
    const n = input.readInt(true);
    console.log(label, "strings after bones", n);
    for (let i = 0; i < n; i++) input.strings.push(input.readString());
  }
  const slotCount = input.readInt(true);
  const slots = [];
  for (let i = 0; i < Math.min(slotCount, 5); i++) {
    slots.push({
      name: input.readString(),
      bone: input.readInt(true),
      color: input.readInt32(),
      attachment: input.readStringRef(),
    });
  }
  console.log(label, "bones", boneCount, "slots", slotCount, "sample", slots);
}

parse36("A", false);
parse36("B", true);
