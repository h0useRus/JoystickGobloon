//! Button outputs: keys, mouse buttons, virtual buttons and how they are sent (hold, pulse, repeat, toggle).

use super::*;

impl Engine {
    /// Presses a map-to-keyboard action's keys in order, unless they are down already.
    pub(super) fn press_keys(&mut self, node: NodeId, out: &mut dyn Output) {
        if !self.keys_down.insert(node) {
            return;
        }
        if let Node::Keys { keys, .. } = &*self.nodes[node] {
            for k in keys {
                out.key(k.scan_code, k.extended, true);
            }
        }
    }

    /// An input event for a button output with a [`SendMode`] other than hold. The trigger is
    /// the press, or the release when the action runs on release.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn send_event(
        &mut self,
        node: NodeId,
        press: Press,
        send: ButtonSend,
        act: ActivationMode,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        // A hat is on while it is pushed in any direction.
        let on = match value {
            Value::Hat(direction) => direction.is_some(),
            v => v.truthy(),
        };
        let state = self.sends.entry(node).or_default();
        if state.input == on {
            return;
        }
        state.input = on;
        let triggered = on != (act == ActivationMode::Release);
        // A wheel has nothing to hold: press, release and toggle turn it one notch each time.
        if let Press::Mouse(wheel @ (MouseButton::WheelUp | MouseButton::WheelDown)) = press
            && matches!(
                send.mode,
                SendMode::Press | SendMode::Release | SendMode::Toggle
            )
        {
            if triggered {
                out.mouse_wheel(if wheel == MouseButton::WheelUp { 1 } else { -1 });
            }
            return;
        }
        let state = self.sends.entry(node).or_default();
        match send.mode {
            SendMode::Hold => {}
            SendMode::Press | SendMode::Release if triggered => {
                self.press(node, press, send.mode == SendMode::Press, out)
            }
            SendMode::Toggle if triggered => {
                let down = !state.down;
                self.press(node, press, down, out);
            }
            SendMode::Pulse if triggered => {
                self.press(node, press, true, out);
                let at = now + ms(send.pulse_ms);
                self.restart_send_timer(node, at, TimerKind::SendUp(node), false);
            }
            SendMode::Repeat if triggered => {
                if state.repeating {
                    return;
                }
                state.repeating = true;
                state.send = send;
                self.press(node, press, true, out);
                let up = now + ms(send.pulse_ms.min(send.delay_ms / 2).max(1));
                self.restart_send_timer(node, up, TimerKind::SendUp(node), false);
                let next = now + ms(send.delay_ms);
                self.restart_send_timer(node, next, TimerKind::SendNext(node), true);
                // Let go of the input in another mode: the repeat stops too. Inside smart-toggle
                // the container sends the release itself.
                if src.no_auto_release {
                    return;
                }
                self.register(
                    &src.key,
                    ReleaseEntry {
                        mode: src.mode.clone(),
                        any_mode: false,
                        on_press: act == ActivationMode::Release,
                        action: ReleaseAction::StopSend(node),
                    },
                );
            }
            SendMode::Repeat => self.stop_send(node, out),
            _ => {}
        }
    }

    /// A [`Press`] output goes down or up; the action's state remembers it.
    pub(super) fn press(&mut self, node: NodeId, press: Press, down: bool, out: &mut dyn Output) {
        match press {
            Press::Vjoy(device, button) => self.vjoy_button(device, button, down, out),
            Press::Pad(gamepad, control) => self.pad_button(gamepad, control, down, out),
            Press::Keys(node) if down => self.press_keys(node, out),
            Press::Keys(node) if self.keys_down.contains(&node) => self.release_keys(node, out),
            // Release only: the keys were pressed by another action (press only).
            Press::Keys(node) => self.let_go_of_keys(node, out),
            // A wheel turns one notch each time it goes down.
            Press::Mouse(MouseButton::WheelUp) if down => out.mouse_wheel(1),
            Press::Mouse(MouseButton::WheelDown) if down => out.mouse_wheel(-1),
            Press::Mouse(MouseButton::WheelUp | MouseButton::WheelDown) => {}
            Press::Mouse(button) => self.mouse_button(button, down, out),
        }
        let state = self.sends.entry(node).or_default();
        state.down = down;
        state.press = Some(press);
    }

    /// Replaces the action's pending release (`next` false) or next pulse (`next` true).
    pub(super) fn restart_send_timer(
        &mut self,
        node: NodeId,
        at: Instant,
        kind: TimerKind,
        next: bool,
    ) {
        let old = {
            let state = self.sends.entry(node).or_default();
            if next {
                state.next_timer.take()
            } else {
                state.up_timer.take()
            }
        };
        if let Some(id) = old {
            self.timers.retain(|t| t.id != id);
        }
        let id = self.add_timer(at, kind);
        let state = self.sends.entry(node).or_default();
        if next {
            state.next_timer = Some(id);
        } else {
            state.up_timer = Some(id);
        }
    }

    /// A pulse's time is up.
    pub(super) fn send_up(&mut self, node: NodeId, out: &mut dyn Output) {
        let Some(state) = self.sends.get_mut(&node) else {
            return;
        };
        state.up_timer = None;
        if let (true, Some(press)) = (state.down, state.press) {
            self.press(node, press, false, out);
        }
    }

    /// Repeat: the next pulse, `at` the time it was due (the period does not drift).
    pub(super) fn send_next(&mut self, node: NodeId, at: Instant, out: &mut dyn Output) {
        let Some(state) = self.sends.get_mut(&node) else {
            return;
        };
        let send = state.send;
        state.next_timer = None;
        let (true, Some(press)) = (state.repeating, state.press) else {
            return;
        };
        if state.down {
            // A pulse longer than the period: the output goes up for one pass first.
            self.press(node, press, false, out);
        }
        self.press(node, press, true, out);
        let up = at + ms(send.pulse_ms.min(send.period_ms / 2).max(1));
        self.restart_send_timer(node, up, TimerKind::SendUp(node), false);
        let next = at + ms(send.period_ms);
        self.restart_send_timer(node, next, TimerKind::SendNext(node), true);
    }

    /// Repeat ends: no more pulses, the output goes up.
    pub(super) fn stop_send(&mut self, node: NodeId, out: &mut dyn Output) {
        let Some(state) = self.sends.get_mut(&node) else {
            return;
        };
        state.repeating = false;
        let timers = [state.up_timer.take(), state.next_timer.take()];
        let (down, press) = (state.down, state.press);
        self.timers.retain(|t| !timers.contains(&Some(t.id)));
        if let (true, Some(press)) = (down, press) {
            self.press(node, press, false, out);
        }
    }

    /// A mouse button held by the profile (released by [`Engine::release_all`]).
    pub(super) fn mouse_button(&mut self, button: MouseButton, down: bool, out: &mut dyn Output) {
        out.mouse_button(button, down);
        let held = self.mouse_down.contains(&button);
        if down && !held {
            self.mouse_down.push(button);
        } else if !down && held {
            self.mouse_down.retain(|&b| b != button);
        }
    }

    /// A virtual stick button the profile presses or lets go.
    pub(super) fn vjoy_button(
        &mut self,
        device: u32,
        button: u8,
        down: bool,
        out: &mut dyn Output,
    ) {
        out.button(device, button, down);
        self.vjoy_down.retain(|&b| b != (device, button));
        if down {
            self.vjoy_down.push((device, button));
        }
    }

    /// A virtual gamepad stick or trigger value (trigger 0..1), remembered so an axis from
    /// buttons on it starts from where other actions left it.
    pub(super) fn pad_axis(
        &mut self,
        gamepad: u32,
        control: GamepadControl,
        v: f32,
        out: &mut dyn Output,
    ) {
        self.pad_axes.insert((gamepad, control), v);
        out.gamepad_axis(gamepad, control, v);
    }

    /// A virtual gamepad button the profile presses or lets go.
    pub(super) fn pad_button(
        &mut self,
        gamepad: u32,
        control: GamepadControl,
        down: bool,
        out: &mut dyn Output,
    ) {
        out.gamepad_button(gamepad, control, down);
        self.pad_down.retain(|&b| b != (gamepad, control));
        if down {
            self.pad_down.push((gamepad, control));
        }
    }
}
