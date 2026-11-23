//! Axes moved by buttons (Axis from buttons): set, velocity, steps, encoder and smooth motion.

use super::*;

impl Engine {
    /// The state of a button-axis axis, made at its current value (a virtual stick axis may
    /// have been written by other actions).
    pub(super) fn axis_state(
        &mut self,
        target: &AxisOut,
        node: NodeId,
        now: Instant,
    ) -> &mut AxisState {
        let known = match target {
            AxisOut::Vjoy(device, axis) => self.vjoy_axes.get(&(*device, *axis)).copied(),
            AxisOut::Logical(key) => self.axes.get(key).copied(),
            // As the device takes it: a trigger 0..1, here -1..1 like the other axes.
            AxisOut::Pad(gamepad, control) => {
                self.pad_axes
                    .get(&(*gamepad, *control))
                    .map(|&v| match control.kind() {
                        ControlKind::Trigger => v * 2.0 - 1.0,
                        _ => v,
                    })
            }
        };
        let state = self
            .button_axes
            .entry(target.clone())
            .or_insert_with(|| AxisState {
                value: known.unwrap_or_else(|| target.rest()),
                node,
                held: [false; 2],
                speed: 0.0,
                target: 0.0,
                before: None,
                last_press: None,
                spring_at: None,
                moving: false,
                last_step: now,
            });
        // Another action wrote it while it stood still.
        if !state.moving
            && let Some(v) = known
        {
            state.value = v;
        }
        state
    }

    /// A button-axis input (`node`'s) went down or up.
    pub(super) fn button_axis_event(
        &mut self,
        node: NodeId,
        target: &AxisOut,
        pressed: bool,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let shared = Arc::clone(&self.nodes[node]);
        let Node::ButtonAxis { ref axis, up, .. } = *shared else {
            return;
        };
        let dir = if up { 1.0 } else { -1.0 };
        let state = self.axis_state(target, node, now);
        let write = match axis.motion {
            AxisMotion::Set => {
                state.held[usize::from(!up)] = pressed;
                if pressed {
                    state.before.get_or_insert(state.value);
                    Some(dir * axis.value)
                } else if state.held[usize::from(up)] {
                    // The other button is still held: its value again.
                    axis.return_on_release.then_some(-dir * axis.value)
                } else {
                    state.before.take().filter(|_| axis.return_on_release)
                }
            }
            AxisMotion::Velocity => {
                let was = held_direction(state.held);
                state.held[usize::from(!up)] = pressed;
                state.node = node;
                let dir = held_direction(state.held);
                // A new press, or one of two held buttons let go: it travels (again) from
                // the start speed.
                if dir != 0.0 && (pressed || dir != was || !state.moving) {
                    state.speed = axis.speed;
                    if !state.moving {
                        state.moving = true;
                        state.last_step = now;
                    }
                }
                None
            }
            AxisMotion::Stepped if pressed => {
                let v = state.value;
                let eps = ButtonAxis::MIN_STEP;
                if up {
                    axis.positions.iter().copied().find(|&p| p > v + eps)
                } else {
                    axis.positions.iter().copied().rev().find(|&p| p < v - eps)
                }
            }
            AxisMotion::Encoder if pressed => {
                // Presses per second, from the time since the last one.
                let rate = state.last_press.map_or(0.0, |t| {
                    1.0 / now.duration_since(t).as_secs_f32().max(0.001)
                });
                let fast = (rate / axis.fast_rate).min(1.0);
                let step = axis.step + (axis.max_step - axis.step) * fast;
                state.spring_at = (axis.spring_ms > 0).then(|| now + ms(axis.spring_ms));
                state.node = node;
                Some((state.value + dir * step * 2.0).clamp(-1.0, 1.0))
            }
            AxisMotion::Smooth if pressed => {
                let from = if state.moving {
                    state.target
                } else {
                    state.value
                };
                state.target = (from + dir * axis.step * 2.0).clamp(-1.0, 1.0);
                state.node = node;
                if !state.moving {
                    state.moving = true;
                    state.last_step = now;
                }
                None
            }
            _ => None,
        };
        if pressed {
            state.last_press = Some(now);
        }
        if let Some(v) = write {
            state.value = v;
            self.write_axis(target, v, now, out);
        }
    }

    /// Moves the moving button-axis axes to `now`: velocity and smooth over the real time since
    /// their last step, encoder springs when due.
    pub(super) fn step_button_axes(&mut self, now: Instant, out: &mut dyn Output) {
        let mut writes = std::mem::take(&mut self.axis_writes);
        for (target, a) in &mut self.button_axes {
            if a.spring_at.is_some_and(|t| t <= now) {
                a.spring_at = None;
                let rest = match &*self.nodes[a.node] {
                    Node::ButtonAxis { axis, .. } => axis.start.unwrap_or_else(|| target.rest()),
                    _ => target.rest(),
                };
                a.value = rest;
                writes.push((target.clone(), rest));
            }
            if !a.moving || now < a.last_step + AXIS_STEP {
                continue;
            }
            let dt = now.duration_since(a.last_step).as_secs_f32();
            a.last_step = now;
            let Node::ButtonAxis { axis, .. } = &*self.nodes[a.node] else {
                a.moving = false;
                continue;
            };
            match axis.motion {
                AxisMotion::Velocity => {
                    let dir = held_direction(a.held);
                    let top = axis.max_speed.max(axis.speed);
                    a.speed = (a.speed + axis.acceleration * dt).min(top);
                    a.value = (a.value + dir * a.speed * 2.0 * dt).clamp(-1.0, 1.0);
                    // Let go, or at the end it travels to.
                    if dir == 0.0 || a.value.abs() >= 1.0 && a.value.signum() == dir {
                        a.moving = false;
                    }
                }
                AxisMotion::Smooth => {
                    // 99 % of the way in the response time.
                    let tau = axis.response_ms as f32 / 1000.0 / 4.6;
                    a.value = a.target + (a.value - a.target) * (-dt / tau).exp();
                    if (a.value - a.target).abs() < ButtonAxis::MIN_STEP {
                        a.value = a.target;
                        a.moving = false;
                    }
                }
                _ => a.moving = false,
            }
            writes.push((target.clone(), a.value));
        }
        for (target, v) in writes.drain(..) {
            self.write_axis(&target, v, now, out);
        }
        self.axis_writes = writes;
    }

    pub(super) fn write_axis(
        &mut self,
        target: &AxisOut,
        v: f32,
        now: Instant,
        out: &mut dyn Output,
    ) {
        match target {
            AxisOut::Vjoy(device, axis) => self.set_axis(*device, *axis, v, out),
            AxisOut::Pad(gamepad, control) => {
                let v = match control.kind() {
                    ControlKind::Trigger => (v + 1.0) / 2.0,
                    _ => v,
                };
                self.pad_axis(*gamepad, *control, v, out);
            }
            AxisOut::Logical(key) => self.feed_logical(key, Value::Axis(v), now, out),
        }
    }
}
