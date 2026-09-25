use crate::issue::{Issue, Issues};
use crate::path::Path;
use std::sync::Arc;

type Check<T> = Arc<dyn Fn(T) -> Result<T, Issue> + Send + Sync>;

struct Step<T> {
    check: Check<T>,
    message: Option<String>,
}

impl<T> Clone for Step<T> {
    fn clone(&self) -> Self {
        Self {
            check: Arc::clone(&self.check),
            message: self.message.clone(),
        }
    }
}

/// The constraints and transformations a built-in decoder applies to its value, in the order they
/// were added. The first one to fail stops the rest.
pub(crate) struct Steps<T> {
    steps: Vec<Step<T>>,
    base_message: Option<String>,
}

impl<T> Clone for Steps<T> {
    fn clone(&self) -> Self {
        Self {
            steps: self.steps.clone(),
            base_message: self.base_message.clone(),
        }
    }
}

impl<T> std::fmt::Debug for Steps<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Steps")
            .field("len", &self.steps.len())
            .field("base_message", &self.base_message)
            .finish()
    }
}

impl<T> Default for Steps<T> {
    fn default() -> Self {
        Self {
            steps: Vec::new(),
            base_message: None,
        }
    }
}

impl<T> Steps<T> {
    /// Adds a step. An issue it returns is moved to the path the value was read at.
    pub(crate) fn push(&mut self, check: impl Fn(T) -> Result<T, Issue> + Send + Sync + 'static) {
        self.steps.push(Step {
            check: Arc::new(check),
            message: None,
        });
    }

    /// Adds a step that fails when `ok` does not hold, with the issue `fail` makes.
    pub(crate) fn require(
        &mut self,
        ok: impl Fn(&T) -> bool + Send + Sync + 'static,
        fail: impl Fn(&T) -> Issue + Send + Sync + 'static,
    ) {
        self.push(move |value| {
            if ok(&value) {
                Ok(value)
            } else {
                Err(fail(&value))
            }
        });
    }

    /// Gives the most recent step, or the type check when there is none, a custom message.
    pub(crate) fn set_message(&mut self, message: String) {
        match self.steps.last_mut() {
            Some(step) => step.message = Some(message),
            None => self.base_message = Some(message),
        }
    }

    /// An issue of the type check, with its custom message if one was given.
    pub(crate) fn base_issue(&self, issue: Issue) -> Issues {
        match &self.base_message {
            Some(message) => issue.with_custom_message(message.clone()).into(),
            None => issue.into(),
        }
    }

    pub(crate) fn run(&self, mut value: T, path: &Path<'_>) -> Result<T, Issues> {
        for step in &self.steps {
            value = (step.check)(value).map_err(|issue| {
                let issue = issue.at(path.to_pointer());
                match &step.message {
                    Some(message) => issue.with_custom_message(message.clone()),
                    None => issue,
                }
            })?;
        }
        Ok(value)
    }
}
