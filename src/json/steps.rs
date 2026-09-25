use crate::issue::{Issue, Issues};
use crate::path::Path;
use std::sync::Arc;

type Transform<T> = Arc<dyn Fn(T) -> T + Send + Sync>;
type Check<T> = Arc<dyn Fn(T) -> Result<T, Issue> + Send + Sync>;

/// One thing a built-in decoder does to its value: change it, which cannot fail, or check it,
/// which can and may carry a custom message.
enum Step<T> {
    Transform(Transform<T>),
    Constraint {
        check: Check<T>,
        message: Option<String>,
    },
}

impl<T> Clone for Step<T> {
    fn clone(&self) -> Self {
        match self {
            Step::Transform(f) => Step::Transform(Arc::clone(f)),
            Step::Constraint { check, message } => Step::Constraint {
                check: Arc::clone(check),
                message: message.clone(),
            },
        }
    }
}

/// The transformations and constraints a built-in decoder applies to its value, in the order they
/// were added. The first constraint to fail stops the rest.
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
    /// Adds a transformation.
    pub(crate) fn transform(&mut self, f: impl Fn(T) -> T + Send + Sync + 'static) {
        self.steps.push(Step::Transform(Arc::new(f)));
    }

    /// Adds a constraint that fails when `ok` does not hold, with the issue `fail` makes. The
    /// issue is moved to the path the value was read at.
    pub(crate) fn require(
        &mut self,
        ok: impl Fn(&T) -> bool + Send + Sync + 'static,
        fail: impl Fn(&T) -> Issue + Send + Sync + 'static,
    ) {
        let check = move |value: T| {
            if ok(&value) {
                Ok(value)
            } else {
                Err(fail(&value))
            }
        };
        self.steps.push(Step::Constraint {
            check: Arc::new(check),
            message: None,
        });
    }

    /// Gives the most recent constraint, or the type check when there is none, a custom message.
    /// Transformations are passed over: they cannot fail, so a message on one would never show.
    pub(crate) fn set_message(&mut self, custom: String) {
        let latest = self.steps.iter_mut().rev().find_map(|step| match step {
            Step::Constraint { message, .. } => Some(message),
            Step::Transform(_) => None,
        });
        match latest {
            Some(message) => *message = Some(custom),
            None => self.base_message = Some(custom),
        }
    }

    /// An issue of the type check, with its custom message if one was given.
    pub(crate) fn base_issue(&self, issue: Issue) -> Issues {
        with_custom(issue, &self.base_message).into()
    }

    pub(crate) fn run(&self, mut value: T, path: &Path<'_>) -> Result<T, Issues> {
        for step in &self.steps {
            value = match step {
                Step::Transform(f) => f(value),
                Step::Constraint { check, message } => check(value)
                    .map_err(|issue| with_custom(issue.at(path.to_pointer()), message))?,
            };
        }
        Ok(value)
    }
}

fn with_custom(issue: Issue, message: &Option<String>) -> Issue {
    match message {
        Some(message) => issue.with_message(message.clone()),
        None => issue,
    }
}
