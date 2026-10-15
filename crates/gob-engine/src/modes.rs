//! Mode stack with Gremlin's "Oldest" cycle resolution.

#[derive(Debug, Clone, PartialEq, Eq)]
struct StackMode {
    name: String,
    previous: Option<String>,
    temporary: bool,
}

#[derive(Debug, Clone)]
pub struct ModeStack {
    stack: Vec<StackMode>,
}

impl ModeStack {
    pub fn new(start: &str) -> Self {
        Self {
            stack: vec![StackMode {
                name: start.to_string(),
                previous: None,
                temporary: false,
            }],
        }
    }

    pub fn current(&self) -> &str {
        &self.stack.last().expect("mode stack is never empty").name
    }

    pub fn is_temporary(&self) -> bool {
        self.stack.last().is_some_and(|m| m.temporary)
    }

    pub fn switch_to(&mut self, name: &str, temporary: bool) {
        let mode = StackMode {
            name: name.to_string(),
            previous: Some(self.current().to_string()),
            temporary,
        };
        // A cycle back to a mode already on the stack drops everything from it on.
        if let Some(idx) = self.stack.iter().position(|m| *m == mode) {
            self.stack.truncate(idx);
        }
        self.stack.push(mode);
    }

    /// Swaps the two topmost modes.
    pub fn previous(&mut self) {
        let n = self.stack.len();
        if n >= 2 {
            self.stack.swap(n - 1, n - 2);
        }
    }

    /// Returns to the mode below the current one.
    pub fn unwind(&mut self) {
        if self.stack.len() >= 2 {
            self.stack.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_switch_and_unwind() {
        let mut m = ModeStack::new("A");
        m.switch_to("B", true);
        assert_eq!((m.current(), m.is_temporary()), ("B", true));
        m.unwind();
        assert_eq!(m.current(), "A");
        m.unwind();
        assert_eq!(m.current(), "A", "never unwinds the last mode");
    }

    #[test]
    fn previous_swaps_top_two() {
        let mut m = ModeStack::new("A");
        m.switch_to("B", false);
        m.previous();
        assert_eq!(m.current(), "A");
        m.previous();
        assert_eq!(m.current(), "B");
    }
}
