// Motion for the landing page: every section with a visual starts empty, loads in when it
// scrolls into view (rise and resolve from blur, in reading order), then every visual acts out
// what it does on a loop. Stories start and end on the exact screenshot, pause off screen and run
// on one requestAnimationFrame clock. Reduced motion (no `motion` class) shows the still page.

(() => {
  const root = document.documentElement;
  if (!root.classList.contains("motion")) {
    // Reduced motion: the logo's own SVG animations stand still too.
    document.querySelectorAll("svg.hero-logo").forEach((svg) => svg.pauseAnimations());
    return;
  }
  // ?still: everything loaded and every overlay at rest, no motion (to check against the screenshots).
  const still = new URLSearchParams(location.search).has("still");
  if (still) {
    root.classList.add("still");
    document.querySelectorAll("svg.hero-logo").forEach((svg) => svg.pauseAnimations());
  }

  // Eased keyframe tracks: [[time, value], ...], values may be numbers or arrays.
  const io = (k) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2);
  const track = (keys, t, ease = io) => {
    if (t <= keys[0][0]) return keys[0][1];
    for (let i = 1; i < keys.length; i++) {
      const [t1, v1] = keys[i];
      const [t0, v0] = keys[i - 1];
      if (t <= t1) {
        const k = ease(Math.min(1, (t - t0) / (t1 - t0 || 1)));
        return Array.isArray(v0) ? v0.map((v, j) => v + (v1[j] - v) * k) : v0 + (v1 - v0) * k;
      }
    }
    return keys[keys.length - 1][1];
  };
  // A pulse: 0 before `at`, up to 1 over `rise`, held for `hold`, back to 0 over `fall`.
  const pulse = (t, at, hold, rise = 0.18, fall = 0.3) =>
    track([[at, 0], [at + rise, 1], [at + rise + hold, 1], [at + rise + hold + fall, 0]], t);

  // Overlays sit on a screenshot at its own pixel positions.
  const overlay = (stage, W, H, x, y, w, h, cls = "") => {
    const el = document.createElement("div");
    el.className = `ov ${cls}`;
    el.setAttribute("aria-hidden", "true");
    let layer = stage.querySelector(":scope > .ovs");
    if (!layer) {
      layer = document.createElement("div");
      layer.className = "ovs";
      stage.append(layer);
    }
    Object.assign(el.style, {
      left: `${(x / W) * 100}%`,
      top: `${(y / H) * 100}%`,
      width: `${(w / W) * 100}%`,
      height: `${(h / H) * 100}%`,
    });
    layer.append(el);
    return el;
  };
  const scale = new ResizeObserver((entries) => {
    for (const e of entries) e.target.style.setProperty("--s", e.contentRect.width / e.target.dataset.w);
  });

  // An axis bar of the Devices page: track, centre tick and a fill growing from the centre.
  const axisBar = (stage, W, H, x, y, rest) => {
    const bar = overlay(stage, W, H, x, y, 100, 8, "ov-track");
    const tick = document.createElement("i");
    tick.className = "ov-tick";
    tick.style.left = "50%";
    const right = document.createElement("i");
    right.className = "ov-half";
    Object.assign(right.style, { left: "51%", width: "49%", transformOrigin: "left" });
    const left = document.createElement("i");
    left.className = "ov-half";
    Object.assign(left.style, { left: "1%", width: "49%", transformOrigin: "right" });
    bar.append(left, tick, right);
    const set = (v) => {
      right.style.transform = `scaleX(${Math.max(0, v)})`;
      left.style.transform = `scaleX(${Math.max(0, -v)})`;
    };
    set(rest);
    return set;
  };

  // One story per visual: build(stage, W, H) returns {period, draw(t)}; draw(0) and draw(period)
  // are the design at rest.
  const STORIES = {
    // The sticks move: the axis bars swing and settle, Button 3 is tapped and then held.
    devices(stage, W, H) {
      const rows = [0, 0, 1, 0, 0, 0, 0];
      const bars = rows.map((rest, i) => axisBar(stage, W, H, 229, 166 + i * 78, rest));
      const dot = overlay(stage, W, H, 616.5, 159.5, 11, 11, "ov-dot");
      const keys = [
        [[0, 0], [0.7, 0.62], [1.5, -0.45], [2.3, 0.2], [3.0, 0]],
        [[0.2, 0], [0.9, -0.55], [1.8, 0.4], [2.6, -0.1], [3.2, 0]],
        [[0.4, 1], [1.2, 0.35], [2.0, -0.4], [2.8, 0.7], [3.4, 1]],
        [[0.5, 0], [1.3, 0.3], [2.2, -0.25], [3.0, 0]],
        [[0.6, 0], [1.5, -0.3], [2.4, 0.2], [3.1, 0]],
        [[3.6, 0], [4.3, 0.8], [5.0, -0.8], [5.8, 0]],
        [[0.8, 0], [2.0, 0.9], [3.4, 0]],
      ];
      return {
        period: 6.4,
        draw(t) {
          bars.forEach((set, i) => set(track(keys[i], t)));
          dot.style.opacity = Math.max(pulse(t, 3.6, 0.15, 0.08, 0.2), pulse(t, 4.4, 0.8, 0.08, 0.25));
        },
      };
    },

    // A tap runs the short press branch; holding past 0.50 s fills the timer and runs the long one.
    tempo(stage, W, H) {
      const dot = overlay(stage, W, H, 117, 11, 11, 11, "ov-dot");
      const short = overlay(stage, W, H, 23.5, 171, 2, 144, "ov-line");
      const long = overlay(stage, W, H, 23.5, 345, 2, 112, "ov-line");
      const timer = overlay(stage, W, H, 39, 115, 56, 26, "ov-fill");
      return {
        period: 6.2,
        draw(t) {
          dot.style.opacity = Math.max(pulse(t, 0.4, 0.18, 0.08, 0.2), pulse(t, 2.8, 0.9, 0.08, 0.25));
          short.style.transform = `scaleY(${track([[0.5, 0], [1.0, 1], [2.0, 1]], t)})`;
          short.style.opacity = track([[2.0, 1], [2.4, 0]], t);
          timer.style.transform = `scaleX(${track([[2.8, 0], [3.3, 1]], t, (k) => k)})`;
          timer.style.opacity = track([[3.3, 1], [4.6, 1], [5.0, 0]], t);
          long.style.transform = `scaleY(${track([[3.3, 0], [3.8, 1]], t)})`;
          long.style.opacity = track([[4.6, 1], [5.0, 0]], t);
        },
      };
    },

    // M1 is held, then M2, then both: the toolbar badges, the layer chip and the modifier rows light.
    modifiers(stage, W, H) {
      const badge = [overlay(stage, W, H, 851, 100, 33, 24, "ov-glow"), overlay(stage, W, H, 892, 100, 34, 24, "ov-glow")];
      const chip = [
        overlay(stage, W, H, 591, 227, 68, 32, "ov-glow"),
        overlay(stage, W, H, 667, 227, 59, 32, "ov-glow"),
        overlay(stage, W, H, 734, 227, 88, 32, "ov-glow"),
      ];
      const row = [overlay(stage, W, H, 383, 357, 40, 20, "ov-glow"), overlay(stage, W, H, 383, 389, 40, 20, "ov-glow")];
      for (const el of [...badge, ...chip, ...row]) el.style.borderRadius = "calc(var(--s) * 4px)";
      return {
        period: 7.6,
        draw(t) {
          const m1 = pulse(t, 0.4, 1.4) + pulse(t, 4.8, 1.4);
          const m2 = pulse(t, 2.6, 1.4) + pulse(t, 4.8, 1.4);
          badge[0].style.opacity = row[0].style.opacity = m1;
          badge[1].style.opacity = row[1].style.opacity = m2;
          chip[0].style.opacity = pulse(t, 0.4, 1.4);
          chip[1].style.opacity = pulse(t, 2.6, 1.4);
          chip[2].style.opacity = pulse(t, 4.8, 1.4);
        },
      };
    },

    // The output tester: every axis slider sweeps in turn and comes back to the centre.
    vjoy(stage, W, H) {
      const knobs = [];
      for (let i = 0; i < 8; i++) {
        const row = overlay(stage, W, H, 148, 418 + i * 32, 328, 24, "ov-slider");
        const rail = document.createElement("i");
        rail.className = "rail";
        const knob = document.createElement("i");
        knob.className = "knob";
        knob.style.left = `${((300 - 148) / 328) * 100}%`;
        row.append(rail, knob);
        knobs.push(knob);
      }
      return {
        period: 6.0,
        draw(t) {
          knobs.forEach((knob, i) => {
            const at = 0.3 + i * 0.14;
            const a = i % 2 ? -1 : 1;
            const v = track([[at, 0], [at + 0.8, 0.85 * a], [at + 1.7, -0.6 * a], [at + 2.5, 0]], t);
            knob.style.transform = `translateX(${(v * 148 * 100) / 24}%)`;
          });
        },
      };
    },

    // A highlight steps down the device list; the hidden sticks' closed eyes pulse as it passes.
    hidhide(stage, W, H) {
      const rows = [0, 1, 2, 3].map((i) => overlay(stage, W, H, 12, 44 + i * 32, 698, 30, "ov-row"));
      const rings = [124, 156].map((y) => {
        const ring = overlay(stage, W, H, 18.5, y - 10, 20, 20, "ov-ring");
        ring.style.color = "#e5484d";
        return ring;
      });
      return {
        period: 5.6,
        draw(t) {
          rows.forEach((row, i) => (row.style.opacity = pulse(t, 0.3 + i * 0.7, 0.45, 0.2, 0.25)));
          rings.forEach((ring, i) => {
            const k = track([[1.7 + i * 0.7, 0], [2.6 + i * 0.7, 1]], t, (x) => 1 - Math.pow(1 - x, 3));
            ring.style.opacity = k > 0 && k < 1 ? 0.85 * (1 - k) : 0;
            ring.style.transform = `scale(${1 + 1.6 * k})`;
          });
        },
      };
    },

    // A live feed: the axis bars breathe and settle on their values, buttons 2 to 16 light in a chase.
    viewer(stage, W, H) {
      const colors = ["#3b8ede", "#e5484d", "#46a758", "#f0932b", "#9a9a9a", "#e8c13a", "#1fb0bf", "#8e57d4"];
      const rest = [0.99, 0, 0, -1, -1, -1, -1, -1];
      const fills = colors.map((c, i) => {
        const bar = overlay(stage, W, H, 328 + i * 80, 482, 20, 100, "ov-bar");
        const fill = document.createElement("i");
        fill.style.background = c;
        bar.append(fill);
        return fill;
      });
      const cells = [];
      for (let i = 1; i < 16; i++) cells.push(overlay(stage, W, H, 302 + i * 42, 647, 34, 34, "ov-cell"));
      const amp = [0.5, 0.8, 0.6, 0.9, 0.7, 1.0, 0.8, 0.6];
      return {
        period: 6.0,
        draw(t) {
          const env = t < 4 ? Math.sin((Math.PI * t) / 4) : 0;
          fills.forEach((fill, i) => {
            const w = Math.sin(t * (2.2 + i * 0.37) + i);
            const v = rest[i] <= -1 ? -1 + env * amp[i] * (1 + w) : rest[i] + env * amp[i] * w;
            fill.style.transform = `scaleY(${(Math.max(-1, Math.min(1, v)) + 1) / 2})`;
          });
          cells.forEach((cell, i) => (cell.style.opacity = 0.85 * pulse(t, 0.6 + i * 0.13, 0.12, 0.06, 0.25)));
        },
      };
    },

    // Before and after: each old tool dims as the matching Joystick Gobloon row lights up.
    compare(el) {
      const before = el.querySelectorAll(".card:not(.solution) .tool-list li");
      const after = el.querySelectorAll(".solution .tool-list li");
      const arrow = el.querySelector(".arrow");
      const ring = document.createElement("span");
      ring.className = "pulse";
      arrow.append(ring);
      return {
        period: 5.4,
        draw(t) {
          const step = Math.floor((t - 0.4) / 1.4);
          const live = t >= 0.4 && t < 0.4 + 3 * 1.4 - 0.3 ? step : -1;
          before.forEach((li, i) => li.classList.toggle("dim", i === live));
          after.forEach((li, i) => li.classList.toggle("lit", i === live));
          const k = live < 0 ? 1 : ((t - 0.4) % 1.4) / 0.9;
          ring.style.opacity = k < 1 ? 0.55 * (1 - k) : 0;
          ring.style.transform = `scale(${1 + 1.4 * Math.min(k, 1)})`;
        },
      };
    },

    // A soft highlight steps through all 23 actions.
    actions(el) {
      const items = [...el.querySelectorAll("li")];
      return {
        period: items.length * 0.22 + 1.6,
        draw(t) {
          const i = Math.floor((t - 0.3) / 0.22);
          items.forEach((li, j) => li.classList.toggle("lit", t >= 0.3 && (j === i || j === i - 1)));
        },
      };
    },
  };

  const running = [];
  const start = (host) => {
    const name = host.dataset.story;
    const img = host.tagName === "IMG" ? host : host.querySelector(":scope > img");
    let story;
    if (img && host.classList.contains("stage")) {
      const W = img.naturalWidth || +img.getAttribute("width");
      const H = img.naturalHeight || +img.getAttribute("height");
      host.dataset.w = W;
      scale.observe(host);
      story = STORIES[name](host, W, H);
    } else {
      story = STORIES[name](host);
    }
    story.draw(0);
    if (still) return;
    running.push({ host, story, t: 0, wait: 1.6, visible: false });
    visibility.observe(host);
  };

  // Stories run only while on screen.
  const visibility = new IntersectionObserver((entries) => {
    for (const e of entries) {
      const s = running.find((r) => r.host === e.target);
      if (s) s.visible = e.isIntersecting;
    }
    kick();
  });

  let last = null;
  let frame = 0;
  const tick = (now) => {
    const dt = last == null ? 0 : Math.min(0.1, (now - last) / 1000);
    last = now;
    let any = false;
    for (const s of running) {
      if (!s.visible) continue;
      any = true;
      if (s.wait > 0) {
        s.wait -= dt;
        continue;
      }
      s.t = (s.t + dt) % s.story.period;
      s.story.draw(s.t);
    }
    if (any) frame = requestAnimationFrame(tick);
    else (frame = 0), (last = null);
  };
  const kick = () => {
    if (!frame) frame = requestAnimationFrame(tick);
  };

  // Loading: the section's elements get delays in reading order, the screenshot resolves once
  // decoded, the counters count up.
  const load = (section) => {
    let card = 0;
    let item = 0;
    for (const el of section.querySelectorAll("[data-k]")) {
      const d = el.dataset.k === "card" ? card++ * 0.085 : 0.15 + item++ * (section.matches(".actions") ? 0.03 : 0.06);
      el.style.setProperty("--d", `${d.toFixed(3)}s`);
    }
    for (const stage of section.querySelectorAll(".stage")) {
      const img = stage.querySelector(":scope > img");
      stage.style.setProperty("--d", "0.25s");
      img.decode().catch(() => {}).then(() => {
        start(stage);
        requestAnimationFrame(() => requestAnimationFrame(() => stage.classList.add("ready")));
      });
    }
    if (section.dataset.story) start(section);
    for (const el of still ? [] : section.querySelectorAll("[data-count]")) {
      const to = +el.dataset.count;
      const t0 = performance.now() + 400;
      const step = (now) => {
        const k = Math.max(0, Math.min(1, (now - t0) / 1100));
        el.textContent = Math.round(to * (1 - Math.pow(1 - k, 3)));
        if (k < 1) requestAnimationFrame(step);
      };
      el.textContent = "0";
      requestAnimationFrame(step);
    }
    requestAnimationFrame(() => section.classList.add("in"));
    // Hover and focus transitions must not inherit the load delays.
    setTimeout(() => section.querySelectorAll("[data-k]").forEach((el) => el.style.removeProperty("--d")), 2600);
  };

  const intro = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (e.isIntersecting) {
          intro.unobserve(e.target);
          load(e.target);
        }
      }
    },
    { threshold: 0.3 },
  );
  document.querySelectorAll(".anim").forEach((el) => intro.observe(el));
})();
