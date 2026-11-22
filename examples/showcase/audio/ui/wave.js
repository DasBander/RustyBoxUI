export function wavInfo(bytes) {
  const v = new DataView(bytes),
    tag = (i) => String.fromCharCode(...new Uint8Array(bytes, i, 4));
  if (bytes.byteLength < 44 || tag(0) !== 'RIFF' || tag(8) !== 'WAVE')
    throw new Error('Choose a RIFF/WAVE audio file.');
  let format,
    data = 0;
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
      };
    if (tag(i) === 'data') data += count;
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
  return format;
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
