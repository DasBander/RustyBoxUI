// A filtered noise loop provides wind. Gate and countdown sounds share the user's master volume.
export class FlightAudio {
  volume = 0.45;

  unlock() {
    if (!window.AudioContext) return;
    if (!this.context) {
      const ctx = new AudioContext();
      this.context = ctx;
      this.master = ctx.createGain();
      this.master.gain.value = this.volume;
      this.master.connect(ctx.destination);
      const buffer = ctx.createBuffer(1, ctx.sampleRate * 2, ctx.sampleRate);
      const samples = buffer.getChannelData(0);
      let last = 0;
      for (let i = 0; i < samples.length; i++) {
        last = (last + (Math.random() * 2 - 1) * 0.04) / 1.025;
        samples[i] = last * 3;
      }
      const source = ctx.createBufferSource();
      source.buffer = buffer;
      source.loop = true;
      this.filter = ctx.createBiquadFilter();
      this.filter.type = 'lowpass';
      this.wind = ctx.createGain();
      this.wind.gain.value = 0;
      source.connect(this.filter).connect(this.wind).connect(this.master);
      source.start();
    }
    if (this.context.state === 'suspended') this.context.resume().catch(() => {});
  }
  setVolume(volume) {
    this.volume = volume;
    if (this.master) this.master.gain.setTargetAtTime(volume, this.context.currentTime, 0.03);
  }
  air(active, boosting = false) {
    if (!this.context) return;
    const mode = active ? (boosting ? 'boost' : 'glide') : 'silent';
    if (mode === this.mode) return;
    this.mode = mode;
    this.wind.gain.setTargetAtTime(
      active ? (boosting ? 0.33 : 0.16) : 0,
      this.context.currentTime,
      0.11
    );
    this.filter.frequency.setTargetAtTime(boosting ? 2150 : 960, this.context.currentTime, 0.16);
  }
  note(hz, duration = 0.12, delay = 0) {
    if (!this.context || !this.volume) return;
    const ctx = this.context,
      start = ctx.currentTime + delay;
    const oscillator = ctx.createOscillator(),
      gain = ctx.createGain();
    oscillator.type = 'sine';
    oscillator.frequency.value = hz;
    gain.gain.setValueAtTime(0.0001, start);
    gain.gain.exponentialRampToValueAtTime(0.19, start + 0.012);
    gain.gain.exponentialRampToValueAtTime(0.0001, start + duration);
    oscillator.connect(gain).connect(this.master);
    oscillator.start(start);
    oscillator.stop(start + duration + 0.01);
    oscillator.onended = () => {
      oscillator.disconnect();
      gain.disconnect();
    };
  }
}
