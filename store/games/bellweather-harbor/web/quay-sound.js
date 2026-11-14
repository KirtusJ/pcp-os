// Short natural sounds leave space for the bell, the wire and the player's music.
// Reuse the audio context unlocked by the harbor's own Sound button.
const quayAudio = {nodes: new Set(), next: 0, sequence: 0, running: false, noise: null};
function stopHarborAmbience() {
  for (const source of quayAudio.nodes) {
    try { source.stop(); } catch (_) {}
  }
  quayAudio.nodes.clear();
  quayAudio.running = false;
  quayAudio.next = 0;
}
function quayEnvelope(ac, source, start, length, volume, filter = null) {
  const gain = ac.createGain();
  gain.gain.setValueAtTime(0, start);
  gain.gain.linearRampToValueAtTime(volume, start + length * 0.23);
  gain.gain.exponentialRampToValueAtTime(0.0001, start + length);
  source.connect(filter || gain);
  if (filter) filter.connect(gain);
  gain.connect(ac.destination);
  quayAudio.nodes.add(source);
  source.onended = () => {
    quayAudio.nodes.delete(source);
    source.disconnect();
    if (filter) filter.disconnect();
    gain.disconnect();
  };
  source.start(start);
  source.stop(start + length + 0.02);
}
function quayNote(ac, at, from, to, length, volume, type = 'sine') {
  const source = ac.createOscillator();
  source.type = type;
  source.frequency.setValueAtTime(from, at);
  source.frequency.exponentialRampToValueAtTime(to, at + length);
  quayEnvelope(ac, source, at, length, volume);
}
function quayWash(ac, at, center, length, volume) {
  if (!quayAudio.noise) {
    const buffer = ac.createBuffer(1, Math.ceil(ac.sampleRate * 0.6), ac.sampleRate);
    const samples = buffer.getChannelData(0);
    let seed = 14;
    for (let i = 0; i < samples.length; i++) {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      samples[i] = seed / 2147483648 - 1;
    }
    quayAudio.noise = buffer;
  }
  const source = ac.createBufferSource(), filter = ac.createBiquadFilter();
  source.buffer = quayAudio.noise;
  source.loop = true;
  filter.type = 'bandpass';
  filter.frequency.value = center;
  filter.Q.value = 0.65;
  quayEnvelope(ac, source, at, length, volume, filter);
}
function harborAmbience(port, enabled, parked) {
  const ac = audioContext;
  if (!enabled || !ac || ac.state !== 'running' || parked || document.hidden) {
    if (quayAudio.running) stopHarborAmbience();
    return;
  }
  const now = ac.currentTime;
  if (!quayAudio.running) {
    quayAudio.running = true;
    quayAudio.next = now + 2.4;
  }
  if (now < quayAudio.next) return;
  // Listening to the wire or working a control should have the soundstage to itself.
  if ((typeof wireTones !== 'undefined' && wireTones.length) || time() - lastTone < 700) {
    quayAudio.next = now + 2;
    return;
  }
  const phrase = quayAudio.sequence++;
  const watch = port.quay_watch(), weather = port.quay_weather(), at = now + 0.015;
  quayAudio.next = now + 6.3 + (phrase % 4) * 1.4;
  if (port.quay_sailing() && phrase % 2 === 0) {
    // A glass buoy rings and an almost inaudible fifth answers across the water.
    quayNote(ac, at, 659, 658, 0.85, 0.032);
    quayNote(ac, at + 0.46, 988, 986, 0.95, 0.015);
  } else if (weather === 1 && phrase % 2 === 0) {
    quayNote(ac, at, 146, 139, 1.25, 0.048, 'triangle');
    quayNote(ac, at + 0.12, 219, 209, 0.95, 0.014);
  } else if (weather === 2 || weather === 3 || phrase % 3 === 2) {
    const rain = weather === 2;
    quayWash(ac, at, rain ? 2300 : weather === 3 ? 570 : 940, 1.25, rain ? 0.054 : 0.095);
    if (rain) quayWash(ac, at + 0.39, 3200, 0.48, 0.025);
  } else if (watch < 2 && phrase % 3 === 0) {
    quayNote(ac, at, 1190, 1720, 0.15, 0.032);
    quayNote(ac, at + 0.17, 1580, 960, 0.26, 0.026);
  } else {
    quayWash(ac, at, 550, 0.52, 0.10);
    quayWash(ac, at + 0.72, 710, 0.78, 0.087);
  }
}

function playHarborReply(person) {
  const ac = audioContext;
  if (!sounding || !ac || ac.state !== 'running' || document.hidden) return;
  if (typeof wireTones !== 'undefined' && wireTones.length) return;
  // The answering sound must remain distinct from a gull, surf or a distant horn.
  stopHarborAmbience();
  const at = ac.currentTime + 0.015;
  quayAudio.running = true;
  quayAudio.next = at + 5;
  if (person === 0) {
    for (const [offset, pitch] of [[0, 880], [0.39, 784]]) {
      quayNote(ac, at + offset, pitch, pitch - 2, 0.76, 0.068);
      quayNote(ac, at + offset, pitch * 2.53, pitch * 2.53 - 4, 0.25, 0.016);
    }
  } else if (person === 1) {
    for (const [offset, pitch] of [[0, 264], [0.15, 301], [0.31, 262]]) {
      quayNote(ac, at + offset, pitch, pitch * 0.85, 0.105, 0.11, 'triangle');
      quayNote(ac, at + offset, pitch * 3.25, pitch * 2.9, 0.085, 0.025);
    }
  } else if (person === 2) {
    quayNote(ac, at, 870, 1035, 0.15, 0.065);
    quayNote(ac, at + 0.27, 1052, 990, 0.48, 0.059);
  }
}
