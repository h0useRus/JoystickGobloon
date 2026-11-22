//! Actions that keep state over time: macros, tempo, double tap, smart toggle, delay, hat buttons, pulses and the logical device's feedback.

use super::*;

impl Engine {
    /// Gremlin's MacroFunctor + MacroManager.queue_macro.
    pub(super) fn macro_trigger(
        &mut self,
        node: NodeId,
        repeat: MacroRepeat,
        src: &Source,
        now: Instant,
        out: &mut dyn Output,
    ) {
        match self.macros.get_mut(&node) {
            // A second activation stops a running toggle macro.
            Some(run) if matches!(repeat, MacroRepeat::Toggle { .. }) => {
                run.stop = true;
                run.queued = 0;
                return;
            }
            Some(run) => run.queued += 1,
            None => {
                self.macros.insert(node, MacroRun::default());
                self.macro_advance(node, now, out);
            }
        }
        if matches!(repeat, MacroRepeat::Hold { .. }) {
            self.register(
                &src.key,
                ReleaseEntry {
                    mode: src.mode.clone(),
                    any_mode: true,
                    on_press: false,
                    action: ReleaseAction::StopMacro(node),
                },
            );
        }
    }

    /// Executes steps until the next pause, the end of the macro or its termination.
    pub(super) fn macro_advance(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        let shared = Arc::clone(&self.nodes[node]);
        let Node::Macro {
            ref steps,
            repeat,
            ref targets,
            ..
        } = *shared
        else {
            return;
        };
        loop {
            let Some(run) = self.macros.get_mut(&node) else {
                return;
            };
            if run.between_rounds {
                run.between_rounds = false;
                let more = match repeat {
                    MacroRepeat::Single => false,
                    MacroRepeat::Count { count, .. } => run.rounds < count,
                    MacroRepeat::Hold { .. } | MacroRepeat::Toggle { .. } => true,
                };
                if run.stop || !more {
                    self.macro_finish(node, now, out);
                    return;
                }
            }
            let Some(step) = steps.get(run.pos).cloned() else {
                let delay = match repeat {
                    MacroRepeat::Single => {
                        self.macro_finish(node, now, out);
                        return;
                    }
                    MacroRepeat::Count { delay, .. }
                    | MacroRepeat::Hold { delay }
                    | MacroRepeat::Toggle { delay } => delay,
                };
                run.pos = 0;
                run.rounds += 1;
                run.between_rounds = true;
                let wait = Duration::from_secs_f64(delay.max(0.0)).max(MACRO_MIN_REPEAT);
                self.add_timer(now + wait, TimerKind::MacroStep(node));
                return;
            };
            run.pos += 1;
            if let MacroStep::Pause(d) = step {
                self.add_timer(
                    now + Duration::from_secs_f64(d.max(0.0)),
                    TimerKind::MacroStep(node),
                );
                return;
            }
            if let MacroStep::RunAction { action } = &step {
                if targets.contains(action)
                    && let Some(&target) = self.action_nodes.get(action)
                {
                    self.run_action(target, now, out);
                }
                continue;
            }
            self.macro_step(step, now, out);
        }
    }

    /// A run-action step: the action runs as a short press of a button of its own would run
    /// it, pressed now and released [`PULSE`] later.
    /// A press still held from an earlier run is released first, so presses and releases
    /// always alternate.
    pub(super) fn run_action(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        if let Some(timer) = self.run_releases.remove(&node)
            && let Some(pos) = self.timers.iter().position(|t| t.id == timer)
            && let TimerKind::RunRelease { node, src } = self.timers.remove(pos).kind
        {
            self.run_release(node, &src, now, out);
        }
        let src = Source {
            key: InputKey::new(RUN_ACTION_DEVICE, InputKind::Button, node as u32),
            mode: self.modes.shared(),
            no_auto_release: false,
        };
        self.run(node, &src, &mut Value::Button(true), now, out, 0);
        self.process_release(&src.key, true, now, out);
        let timer = self.add_timer(now + PULSE, TimerKind::RunRelease { node, src });
        self.run_releases.insert(node, timer);
    }

    pub(super) fn run_release(
        &mut self,
        node: NodeId,
        src: &Source,
        now: Instant,
        out: &mut dyn Output,
    ) {
        self.run(node, src, &mut Value::Button(false), now, out, 0);
        self.process_release(&src.key, false, now, out);
    }

    /// Ends a run and starts the next queued activation, if any.
    pub(super) fn macro_finish(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        let queued = self.macros.remove(&node).map_or(0, |r| r.queued);
        if queued > 0 {
            self.macros.insert(
                node,
                MacroRun {
                    queued: queued - 1,
                    ..MacroRun::default()
                },
            );
            self.macro_advance(node, now, out);
        }
    }

    pub(super) fn macro_step(&mut self, step: MacroStep, now: Instant, out: &mut dyn Output) {
        let id = |n: u32| n.min(255) as u8;
        match step {
            MacroStep::Key {
                scan_code,
                extended,
                pressed,
            } => out.key(scan_code, extended, pressed),
            // Wheel steps scroll one notch whatever their pressed flag says.
            MacroStep::MouseButton {
                button: MouseButton::WheelUp,
                ..
            } => out.mouse_wheel(1),
            MacroStep::MouseButton {
                button: MouseButton::WheelDown,
                ..
            } => out.mouse_wheel(-1),
            MacroStep::MouseButton { button, pressed } => out.mouse_button(button, pressed),
            MacroStep::MouseMotion { dx, dy } => out.mouse_move(dx, dy),
            MacroStep::VjoyButton {
                device,
                id: n,
                pressed,
            } => self.vjoy_button(device, id(n), pressed, out),
            MacroStep::VjoyAxis {
                device,
                id: n,
                value,
                relative,
            } => {
                let mut v = value as f32;
                if relative {
                    v += self.vjoy_axes.get(&(device, id(n))).copied().unwrap_or(0.0);
                }
                self.set_axis(device, id(n), v.clamp(-1.0, 1.0), out);
            }
            MacroStep::VjoyHat {
                device,
                id: n,
                direction,
            } => out.hat(device, id(n), direction),
            MacroStep::GamepadButton {
                gamepad,
                control,
                pressed,
            } => {
                if let Some(c) = GamepadControl::parse(&control) {
                    self.pad_button(gamepad, c, pressed, out);
                }
            }
            MacroStep::GamepadAxis {
                gamepad,
                control,
                value,
            } => {
                let Some(c) = GamepadControl::parse(&control) else {
                    return;
                };
                let low = if c.kind() == ControlKind::Trigger {
                    0.0
                } else {
                    -1.0
                };
                self.pad_axis(gamepad, c, (value as f32).clamp(low, 1.0), out);
            }
            MacroStep::GamepadDpad { gamepad, direction } => out.gamepad_dpad(gamepad, direction),
            MacroStep::LogicalButton { id: n, pressed } => {
                let key = InputKey::new(gob_core::LOGICAL_DEVICE_GUID, InputKind::Button, n);
                self.feed_logical(&key, Value::Button(pressed), now, out);
            }
            MacroStep::LogicalAxis {
                id: n,
                value,
                relative,
            } => {
                let key = InputKey::new(gob_core::LOGICAL_DEVICE_GUID, InputKind::Axis, n);
                let mut v = value as f32;
                if relative {
                    v += self.axes.get(&key).copied().unwrap_or(0.0);
                }
                self.feed_logical(&key, Value::Axis(v.clamp(-1.0, 1.0)), now, out);
            }
            MacroStep::LogicalHat { id: n, direction } => {
                let key = InputKey::new(gob_core::LOGICAL_DEVICE_GUID, InputKind::Hat, n);
                self.feed_logical(&key, Value::Hat(direction), now, out);
            }
            // Run-action steps run in `macro_advance`, which knows the macro's targets.
            MacroStep::Pause(_) | MacroStep::RunAction { .. } | MacroStep::Other => {}
        }
    }

    pub(super) fn mouse(
        &mut self,
        node: NodeId,
        action: MouseAction,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let MouseAction::Motion {
            direction,
            min_speed,
            max_speed,
            time_to_max_speed,
        } = action
        else {
            if let MouseAction::Button(button) = action {
                match button {
                    MouseButton::WheelUp if value.truthy() => out.mouse_wheel(1),
                    MouseButton::WheelDown if value.truthy() => out.mouse_wheel(-1),
                    MouseButton::WheelUp | MouseButton::WheelDown => {}
                    b => self.mouse_button(b, value.truthy(), out),
                }
            }
            return;
        };
        let key = (node, src.key.clone());
        match value {
            Value::Axis(v) => {
                // Gremlin maps the "vertical" heading 0 to 180 for axes: positive values move down.
                let heading = if direction == 0.0 { 180.0 } else { direction };
                let speed = if v.abs() < 1e-3 {
                    0.0
                } else {
                    (min_speed + v.abs() * (max_speed - min_speed)).copysign(v)
                };
                let (x, y) = compass(heading);
                self.motion
                    .set_velocity(key, (x * speed, y * speed), src.mode.clone(), now);
            }
            Value::Hat(Some(d)) => {
                let dir = compass(f32::from(d) / 100.0);
                self.motion.set_accelerated(
                    key,
                    dir,
                    min_speed,
                    max_speed,
                    time_to_max_speed,
                    src.mode.clone(),
                    now,
                );
            }
            Value::Button(true) => {
                let dir = compass(direction);
                self.motion.set_accelerated(
                    key,
                    dir,
                    min_speed,
                    max_speed,
                    time_to_max_speed,
                    src.mode.clone(),
                    now,
                );
            }
            Value::Hat(None) | Value::Button(false) => self.motion.clear(&key),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn smart_toggle(
        &mut self,
        node: NodeId,
        children: &[NodeId],
        delay: Duration,
        src: &Source,
        pressed: bool,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let inner = Source {
            no_auto_release: true,
            ..src.clone()
        };
        let phase = self.toggles.entry(node).or_default().phase;
        let (next, run) = match (phase, pressed) {
            (TogglePhase::Wait, true) => {
                let timer = self.add_timer(now + delay, TimerKind::ToggleTimeout(node));
                self.toggles.entry(node).or_default().timer = Some(timer);
                (TogglePhase::Down, true)
            }
            (TogglePhase::Down, false) => (TogglePhase::Latched, false),
            (TogglePhase::Held | TogglePhase::Latched, false) => (TogglePhase::Wait, true),
            (p, _) => (p, false),
        };
        self.toggles.entry(node).or_default().phase = next;
        if run {
            self.run_all(children, &inner, Value::Button(pressed), now, out);
        }
    }

    /// Stops relative axes and cursor motion the actions under `nodes` keep going (Gremlin's
    /// `reset_functor_tree`).
    pub(super) fn stop_motion(&mut self, nodes: &[NodeId], depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        for &node in nodes {
            let shared = Arc::clone(&self.nodes[node]);
            match *shared {
                Node::Vjoy {
                    device,
                    target: VjoyTarget::Axis(id),
                    relative: Some(_),
                    ..
                } => {
                    if let Some(r) = self.relative.get_mut(&(device, id)) {
                        r.active = false;
                        r.delta = 0.0;
                    }
                }
                Node::Logical {
                    ref key,
                    relative: Some(_),
                    ..
                } => {
                    if let Some(r) = self.logical_relative.get_mut(key) {
                        r.active = false;
                        r.delta = 0.0;
                    }
                }
                Node::Mouse { .. } => self.motion.retain(|(n, _), _| *n != node),
                Node::Seq(ref c)
                | Node::SmartToggle {
                    children: ref c, ..
                }
                | Node::Delay {
                    children: ref c, ..
                }
                | Node::Merge {
                    children: ref c, ..
                } => self.stop_motion(c, depth + 1),
                Node::Split {
                    ref lower,
                    ref upper,
                    ..
                } => {
                    self.stop_motion(lower, depth + 1);
                    self.stop_motion(upper, depth + 1);
                }
                Node::Zones { ref zones, .. } => {
                    for (_, c) in zones {
                        self.stop_motion(c, depth + 1);
                    }
                }
                Node::Tempo {
                    ref short,
                    ref long,
                    ..
                } => {
                    self.stop_motion(short, depth + 1);
                    self.stop_motion(long, depth + 1);
                }
                Node::DoubleTap {
                    ref single,
                    ref double,
                    ..
                } => {
                    self.stop_motion(single, depth + 1);
                    self.stop_motion(double, depth + 1);
                }
                Node::AxisDelta {
                    ref positive,
                    ref negative,
                    ..
                } => {
                    self.stop_motion(positive, depth + 1);
                    self.stop_motion(negative, depth + 1);
                }
                Node::Chain { ref chains, .. } => {
                    for c in chains {
                        self.stop_motion(c, depth + 1);
                    }
                }
                Node::HatButtons(ref dirs) => {
                    for (_, c) in dirs {
                        self.stop_motion(c, depth + 1);
                    }
                }
                Node::Condition { ref branches, .. } => {
                    for c in branches {
                        self.stop_motion(c, depth + 1);
                    }
                }
                Node::DualDeadzone { ref outputs, .. } => {
                    for c in outputs {
                        self.stop_motion(c, depth + 1);
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn run_all(
        &mut self,
        nodes: &[NodeId],
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        for &n in nodes {
            self.run(n, src, &mut value.clone(), now, out, 0);
        }
    }

    pub(super) fn leave_temporary(&mut self, mode: &str) {
        if self.modes.current() == mode && self.modes.is_temporary() {
            self.modes.unwind();
        }
    }

    pub(super) fn tempo_event(
        &mut self,
        node: NodeId,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let Value::Button(pressed) = value else {
            return;
        };
        let Node::Tempo {
            short,
            long,
            threshold,
            on_press,
        } = (*self.nodes[node]).clone()
        else {
            return;
        };
        if pressed {
            self.tempo.entry(node).or_default().press = Some((value, src.clone()));
            self.register(
                &src.key,
                ReleaseEntry {
                    mode: src.mode.clone(),
                    any_mode: true,
                    on_press: false,
                    action: ReleaseAction::TempoRelease(node),
                },
            );
        }
        let phase = self.tempo.entry(node).or_default().phase;
        match (phase, pressed) {
            (Phase::Wait, true) => {
                let timer = self.add_timer(now + threshold, TimerKind::TempoTimeout(node));
                let state = self.tempo.entry(node).or_default();
                state.phase = Phase::Short;
                state.timer = Some(timer);
                if on_press {
                    self.run_all(&short, src, value, now, out);
                }
            }
            (Phase::Wait, false) => {}
            (Phase::Short, false) => {
                self.cancel_tempo_timer(node);
                self.tempo.entry(node).or_default().phase = Phase::Wait;
                if on_press {
                    self.run_all(&short, src, value, now, out);
                } else {
                    self.short_pulse(node, &short, now, out);
                }
            }
            (Phase::Long, false) => {
                self.tempo.entry(node).or_default().phase = Phase::Wait;
                self.run_all(&long, src, value, now, out);
                if on_press {
                    self.run_all(&short, src, value, now, out);
                }
            }
            // A press while already pressed: invalid transition, reset like Gremlin.
            (Phase::Short | Phase::Long, true) => self.tempo_reset(node, src, value, now, out),
        }
    }

    pub(super) fn tempo_timeout(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        let Node::Tempo { long, .. } = (*self.nodes[node]).clone() else {
            return;
        };
        let state = self.tempo.entry(node).or_default();
        state.timer = None;
        if state.phase != Phase::Short {
            return;
        }
        state.phase = Phase::Long;
        if let Some((value, src)) = state.press.clone() {
            self.run_all(&long, &src, value, now, out);
        }
    }

    pub(super) fn short_pulse(
        &mut self,
        node: NodeId,
        short: &[NodeId],
        now: Instant,
        out: &mut dyn Output,
    ) {
        if let Some((value, src)) = self.tempo.get(&node).and_then(|s| s.press.clone()) {
            self.pulse(short, value, src, now, out);
        }
    }

    /// Runs `nodes` with the press value now and with a release [`PULSE`] later.
    pub(super) fn pulse(
        &mut self,
        nodes: &[NodeId],
        value: Value,
        src: Source,
        now: Instant,
        out: &mut dyn Output,
    ) {
        self.run_all(nodes, &src, value, now, out);
        self.add_timer(
            now + PULSE,
            TimerKind::PulseRelease {
                nodes: nodes.to_vec(),
                src,
            },
        );
    }

    /// A logical device input changed: record it and run its bindings like a device event.
    pub(super) fn feed_logical(
        &mut self,
        key: &InputKey,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        if self.logical_depth >= 8 {
            return;
        }
        self.logical_depth += 1;
        self.logical_changed = true;
        self.seed(key, value);
        let mode = self.modes.shared();
        self.run_scripts(key, value, &mode, now, out);
        self.dispatch(key, value, mode, now, out);
        self.logical_depth -= 1;
    }

    /// Advances relative logical axes like [`Engine::step_relative`] does for vJoy.
    pub(super) fn step_logical(&mut self, now: Instant, out: &mut dyn Output) {
        let mut moved = Vec::new();
        for (key, r) in self.logical_relative.iter_mut().filter(|(_, r)| r.active) {
            let steps = (now.saturating_duration_since(r.last_step).as_secs_f32()
                / RELATIVE_STEP.as_secs_f32())
            .floor();
            if steps < 1.0 {
                continue;
            }
            r.value = (r.value + r.delta * steps).clamp(-1.0, 1.0);
            r.last_step += RELATIVE_STEP.mul_f32(steps);
            moved.push((key.clone(), r.value));
            if r.stopping && now.saturating_duration_since(r.last_update) > RELATIVE_STOP {
                r.active = false;
            }
        }
        for (key, v) in moved {
            self.feed_logical(&key, Value::Axis(v), now, out);
        }
    }

    /// One condition of a condition action (Gremlin's comparators on the source states).
    pub(super) fn cond_holds(&self, cond: &Cond, value: Value, out: &dyn Output) -> bool {
        let states: Vec<Value> = match &cond.source {
            CondSource::Current => vec![value],
            CondSource::Inputs(keys) => keys
                .iter()
                .map(|k| match k.kind {
                    InputKind::Axis => Value::Axis(self.axes.get(k).copied().unwrap_or(0.0)),
                    InputKind::Button => {
                        Value::Button(self.buttons.get(k).copied().unwrap_or(false))
                    }
                    InputKind::Hat => Value::Hat(self.hats.get(k).copied().flatten()),
                })
                .collect(),
            CondSource::Keys(keys) => keys
                .iter()
                .map(|k| Value::Button(out.key_pressed(k.scan_code, k.extended)))
                .collect(),
            CondSource::Vjoy(device, kind, id) => {
                vec![out.vjoy_state(*device, *kind, *id).unwrap_or(match kind {
                    InputKind::Axis => Value::Axis(0.0),
                    InputKind::Button => Value::Button(false),
                    InputKind::Hat => Value::Hat(None),
                })]
            }
            CondSource::Never => return false,
        };
        compare(&cond.comparator, &states)
    }

    /// Gremlin's HatButtonsFunctor: every direction is a button pressed while the hat
    /// points exactly there (4-button mode ignores diagonals), centered presses `Center`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn hat_buttons_event(
        &mut self,
        node: NodeId,
        buttons: &[(Option<u16>, Vec<NodeId>)],
        src: &Source,
        direction: Option<u16>,
        now: Instant,
        out: &mut dyn Output,
        depth: usize,
    ) {
        for (i, (dir, children)) in buttons.iter().enumerate() {
            let pressed = *dir == direction;
            let state = self
                .hat_buttons
                .entry(node)
                .or_insert_with(|| vec![false; buttons.len()]);
            if state[i] == pressed {
                continue;
            }
            state[i] = pressed;
            let vsrc = Source {
                key: hat_button_key(node, i),
                mode: src.mode.clone(),
                no_auto_release: src.no_auto_release,
            };
            for &c in children {
                self.run(c, &vsrc, &mut Value::Button(pressed), now, out, depth + 1);
            }
            self.process_release(&vsrc.key, pressed, now, out);
        }
        // Not in Gremlin: the hat centered in a mode where it is bound differently
        // still releases what the directions pressed.
        if direction.is_some() {
            self.register(
                &src.key,
                ReleaseEntry {
                    mode: src.mode.clone(),
                    any_mode: false,
                    on_press: false,
                    action: ReleaseAction::HatButtons(node),
                },
            );
        }
    }

    /// Gremlin's DoubleTapFunctor state machine.
    pub(super) fn tap_event(
        &mut self,
        node: NodeId,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let Value::Button(pressed) = value else {
            return;
        };
        let Node::DoubleTap {
            single,
            double,
            threshold,
            exclusive,
        } = (*self.nodes[node]).clone()
        else {
            return;
        };
        if pressed {
            self.taps.entry(node).or_default().press = Some((value, src.clone()));
            self.register(
                &src.key,
                ReleaseEntry {
                    mode: src.mode.clone(),
                    any_mode: true,
                    on_press: false,
                    action: ReleaseAction::TapRelease(node),
                },
            );
        }
        use TapPhase::*;
        let phase = self.taps.entry(node).or_default().phase;
        let next = match (exclusive, phase, pressed) {
            (_, Neutral, true) => {
                if !exclusive {
                    self.run_all(&single, src, value, now, out);
                }
                let timer = self.add_timer(now + threshold, TimerKind::TapTimeout(node));
                self.taps.entry(node).or_default().timer = Some(timer);
                PressedTimed
            }
            (true, Neutral, false) => Neutral,
            (_, PressedTimed, false) => {
                if !exclusive {
                    self.run_all(&single, src, value, now, out);
                }
                Timed
            }
            (_, Pressed, false) => {
                self.run_all(&single, src, value, now, out);
                Neutral
            }
            (_, Timed, true) => {
                if !exclusive {
                    self.run_all(&single, src, value, now, out);
                }
                self.run_all(&double, src, value, now, out);
                SecondTimed
            }
            (_, SecondTimed, false) => {
                if !exclusive {
                    self.run_all(&single, src, value, now, out);
                }
                self.run_all(&double, src, value, now, out);
                Neutral
            }
            // Invalid transition: reset like Gremlin.
            _ => {
                self.tap_reset(node, src, value, now, out);
                return;
            }
        };
        self.taps.entry(node).or_default().phase = next;
    }

    pub(super) fn tap_timeout(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        let Node::DoubleTap {
            single, exclusive, ..
        } = (*self.nodes[node]).clone()
        else {
            return;
        };
        let state = self.taps.entry(node).or_default();
        state.timer = None;
        let press = state.press.clone();
        // Timeouts without a transition are stale and ignored.
        let next = match state.phase {
            TapPhase::PressedTimed => {
                if let (true, Some((value, src))) = (exclusive, press) {
                    self.run_all(&single, &src, value, now, out);
                }
                TapPhase::Pressed
            }
            TapPhase::Timed => {
                if let (true, Some((value, src))) = (exclusive, press) {
                    self.pulse(&single, value, src, now, out);
                }
                TapPhase::Neutral
            }
            phase => phase,
        };
        self.taps.entry(node).or_default().phase = next;
    }

    pub(super) fn tap_reset(
        &mut self,
        node: NodeId,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let Node::DoubleTap { single, double, .. } = (*self.nodes[node]).clone() else {
            return;
        };
        let state = self.taps.entry(node).or_default();
        state.phase = TapPhase::Neutral;
        if let Some(id) = state.timer.take() {
            self.timers.retain(|t| t.id != id);
        }
        let all: Vec<NodeId> = single.into_iter().chain(double).collect();
        self.run_all(&all, src, value, now, out);
    }

    pub(super) fn tempo_reset(
        &mut self,
        node: NodeId,
        src: &Source,
        value: Value,
        now: Instant,
        out: &mut dyn Output,
    ) {
        let Node::Tempo { short, long, .. } = (*self.nodes[node]).clone() else {
            return;
        };
        self.cancel_tempo_timer(node);
        self.tempo.entry(node).or_default().phase = Phase::Wait;
        let all: Vec<NodeId> = short.into_iter().chain(long).collect();
        self.run_all(&all, src, value, now, out);
    }

    /// A Delay's input went down (its time starts again) or up.
    pub(super) fn delay_event(
        &mut self,
        node: NodeId,
        delay: Duration,
        src: &Source,
        pressed: bool,
        now: Instant,
        out: &mut dyn Output,
    ) {
        if !pressed {
            self.delay_release(node, now, out);
            return;
        }
        if let Some(timer) = self.delays.remove(&node).and_then(|d| d.timer) {
            self.timers.retain(|t| t.id != timer);
        }
        let timer = self.add_timer(now + delay, TimerKind::DelayFire(node));
        self.delays.insert(
            node,
            DelayState {
                timer: Some(timer),
                src: src.clone(),
                held: true,
            },
        );
        // Inside a container that keeps the press latched (smart toggle), letting go of the
        // input is not a release: the container sends its own.
        if !src.no_auto_release {
            self.register(
                &src.key,
                ReleaseEntry {
                    mode: src.mode.clone(),
                    any_mode: true,
                    on_press: false,
                    action: ReleaseAction::DelayRelease(node),
                },
            );
        }
    }

    /// Let go before the time is up: cancelled while it needs the input held, else run as a
    /// short press when the time is up; after it: the children are let go.
    pub(super) fn delay_release(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        let Node::Delay {
            ref children,
            while_held,
            ..
        } = *self.nodes[node].clone()
        else {
            return;
        };
        let Some(state) = self.delays.get_mut(&node).filter(|d| d.held) else {
            return;
        };
        match state.timer {
            Some(timer) if while_held => {
                self.timers.retain(|t| t.id != timer);
                self.delays.remove(&node);
            }
            Some(_) => state.held = false,
            None => {
                if let Some(state) = self.delays.remove(&node) {
                    self.run_all(children, &state.src, Value::Button(false), now, out);
                }
            }
        }
    }

    /// A Delay's time is up: its children run (held: pressed until the input goes up; let go
    /// already: a short press).
    pub(super) fn delay_fire(&mut self, node: NodeId, now: Instant, out: &mut dyn Output) {
        let Node::Delay { ref children, .. } = *self.nodes[node].clone() else {
            return;
        };
        let Some(state) = self.delays.get_mut(&node) else {
            return;
        };
        state.timer = None;
        if state.held {
            let src = state.src.clone();
            self.run_all(children, &src, Value::Button(true), now, out);
        } else if let Some(state) = self.delays.remove(&node) {
            // The input is up already: nothing may wait for its release (the pulse lets go).
            let src = Source {
                no_auto_release: true,
                ..state.src
            };
            self.pulse(children, Value::Button(true), src, now, out);
        }
    }

    pub(super) fn cancel_tempo_timer(&mut self, node: NodeId) {
        if let Some(id) = self.tempo.get_mut(&node).and_then(|s| s.timer.take()) {
            self.timers.retain(|t| t.id != id);
        }
    }
}
