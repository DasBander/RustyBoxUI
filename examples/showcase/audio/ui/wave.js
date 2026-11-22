export function wavInfo(bytes) {
  const v = new DataView(bytes),
    tag = (i) => String.fromCharCode(...new Uint8Array(bytes, i, 4));
  if (bytes.byteLength < 44 || tag(0) !== 'RIFF' || tag(8) !== 'WAVE')
    throw new Error('Choose a RIFF/WAVE audio file.');
  let format,
    data = 0;
  const ranges = [];
  for (let i = 12; i + 8 <= bytes.byteLength; ) {
    const count = v.getUint32(i + 4, true),
      next = i + 8 + count;
    if (next > bytes.byteLength) throw new Error('The WAV file is incomplete.');
    if (tag(i) === 'fmt ' && count >= 16)
      format = {
        channels: v.getUint16(i + 10, true),
        rate: v.getUint32(i + 12, true),
        alignment: v.getUint16(i + 20, true),
        bits: v.getUint16(i + 22, true),
        tag: v.getUint16(i + 8, true),
        fmt: new Uint8Array(bytes.slice(i + 8, next)),
      };
    if (tag(i) === 'data') {
      ranges.push({ offset: i + 8, length: count });
      data += count;
    }
    i = next + (count % 2);
  }
  if (
    !format ||
    !data ||
    format.rate < 8000 ||
    format.rate > 192000 ||
    !format.alignment ||
    !format.channels ||
    data / format.alignment / format.rate > 600
  )
    throw new Error('Choose a WAV up to ten minutes long, sampled at 8–192 kHz.');
  return { ...format, ranges, dataBytes: data };
}

function chunkHeader(name, size) {
  const header = new Uint8Array(8);
  header.set(new TextEncoder().encode(name));
  new DataView(header.buffer).setUint32(4, size, true);
  return header;
}

// Copy uncompressed frames directly, including 24-bit PCM and extensible channel layouts.
// For compressed input the Web Audio decoder supplies the samples for a PCM16 export.
export function exportWave(source, info, audio, first, last) {
  if (
    !Number.isSafeInteger(first) ||
    !Number.isSafeInteger(last) ||
    first < 0 ||
    last > audio.length ||
    first >= last
  )
    throw new Error('Select at least one sample to export.');
  let fmt = info.fmt,
    alignment = info.alignment;
  const kind =
    info.tag === 0xfffe && fmt.length >= 40
      ? new DataView(fmt.buffer).getUint16(24, true)
      : info.tag;
  const exact =
    [1, 3].includes(kind) &&
    alignment === info.channels * Math.ceil(info.bits / 8) &&
    info.rate === audio.sampleRate &&
    audio.length * alignment === info.dataBytes;
  const payload = [];
  if (exact) {
    let position = 0;
    for (const range of info.ranges) {
      const from = Math.max(0, first * alignment - position);
      const to = Math.min(range.length, last * alignment - position);
      if (to > from) payload.push(source.slice(range.offset + from, range.offset + to));
      position += range.length;
    }
  } else {
    alignment = audio.numberOfChannels * 2;
    fmt = new Uint8Array(16);
    const fields = new DataView(fmt.buffer);
    fields.setUint16(0, 1, true);
    fields.setUint16(2, audio.numberOfChannels, true);
    fields.setUint32(4, audio.sampleRate, true);
    fields.setUint32(8, audio.sampleRate * alignment, true);
    fields.setUint16(12, alignment, true);
    fields.setUint16(14, 16, true);
    const channels = Array.from({ length: audio.numberOfChannels }, (_, i) =>
      audio.getChannelData(i)
    );
    for (let frame = first; frame < last; frame += 16384) {
      const count = Math.min(16384, last - frame);
      const samples = new DataView(new ArrayBuffer(count * alignment));
      for (let i = 0; i < count; i++) {
        for (let c = 0; c < channels.length; c++) {
          const value = Math.max(-1, Math.min(1, channels[c][frame + i]));
          samples.setInt16(
            i * alignment + c * 2,
            Math.round(value * (value < 0 ? 32768 : 32767)),
            true
          );
        }
      }
      payload.push(samples.buffer);
    }
  }
  const frames = last - first,
    dataSize = frames * alignment;
  const body = [new TextEncoder().encode('WAVE'), chunkHeader('fmt ', fmt.length), fmt];
  if (fmt.length % 2) body.push(new Uint8Array(1));
  if (exact && kind === 3) {
    const count = new DataView(new ArrayBuffer(4));
    count.setUint32(0, frames, true);
    body.push(chunkHeader('fact', 4), count.buffer);
  }
  body.push(chunkHeader('data', dataSize), ...payload);
  if (dataSize % 2) body.push(new Uint8Array(1));
  const wave = new Blob(body);
  if (wave.size > 0xffffffff) throw new Error('Selection exceeds the RIFF/WAVE size field.');
  return new Blob([chunkHeader('RIFF', wave.size), wave], { type: 'audio/wav' });
}
export function peaks(buffer) {
  const bins = Math.min(3000, buffer.length);
  return Array.from({ length: Math.min(2, buffer.numberOfChannels) }, (_, channel) => {
    const samples = buffer.getChannelData(channel),
      data = new Float32Array(bins * 2);
    for (let i = 0; i < bins; i++) {
      let low = 0,
        high = 0;
      const end = Math.floor(((i + 1) * samples.length) / bins);
      for (let j = Math.floor((i * samples.length) / bins); j < end; j++) {
        low = Math.min(low, samples[j]);
        high = Math.max(high, samples[j]);
      }
      data[i * 2] = low;
      data[i * 2 + 1] = high;
    }
    return data;
  });
}
export function timestamp(time) {
  const ms = Math.round(time * 1000);
  return (
    String(Math.floor(ms / 60000)).padStart(2, '0') +
    ':' +
    String(Math.floor(ms / 1000) % 60).padStart(2, '0') +
    '.' +
    String(ms % 1000).padStart(3, '0')
  );
}
