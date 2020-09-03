//! Simplified scope implementation used for indexing.

use crate::collections::{HashMap, HashSet};
use crate::error::CompileError;
use runestick::{MetaClosureCapture, Span};
use std::rc::Rc;
use std::{cell::RefCell, mem::ManuallyDrop};

#[derive(Debug)]
pub struct IndexScopeGuard {
    levels: Rc<RefCell<Vec<IndexScopeLevel>>>,
}

impl IndexScopeGuard {
    /// Pop the last closure scope and return captured variables.
    pub(crate) fn into_closure(self, span: Span) -> Result<Closure, CompileError> {
        let this = ManuallyDrop::new(self);

        let level = this
            .levels
            .borrow_mut()
            .pop()
            .ok_or_else(|| CompileError::internal("missing scope", span))?;

        match level {
            IndexScopeLevel::IndexClosure(closure) => Ok(Closure {
                captures: closure.captures,
                generator: closure.generator,
            }),
            _ => Err(CompileError::internal("expected closure", span)),
        }
    }

    /// Pop the last function scope and return function information.
    pub(crate) fn into_function(self, span: Span) -> Result<Function, CompileError> {
        let this = ManuallyDrop::new(self);

        let level = this
            .levels
            .borrow_mut()
            .pop()
            .ok_or_else(|| CompileError::internal("missing scope", span))?;

        match level {
            IndexScopeLevel::IndexFunction(fun) => Ok(Function {
                generator: fun.generator,
            }),
            _ => Err(CompileError::internal("expected function", span)),
        }
    }
}

impl Drop for IndexScopeGuard {
    fn drop(&mut self) {
        let exists = self.levels.borrow_mut().pop().is_some();
        debug_assert!(exists);
    }
}

#[derive(Debug)]
struct IndexScope {
    locals: HashMap<String, Span>,
}

impl IndexScope {
    /// Construct a new scope.
    pub fn new() -> Self {
        Self {
            locals: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct IndexClosure {
    /// Variables which could not be found in the immediate scope, and
    /// marked as needed to be captured from the outer scope.
    captures: Vec<MetaClosureCapture>,
    existing: HashSet<String>,
    scope: IndexScope,
    generator: bool,
}

impl IndexClosure {
    /// Construct a new closure.
    pub fn new() -> Self {
        Self {
            captures: Vec::new(),
            existing: HashSet::new(),
            scope: IndexScope::new(),
            generator: false,
        }
    }
}

pub(crate) struct Function {
    pub(crate) generator: bool,
}

pub(crate) struct Closure {
    pub(crate) captures: Vec<MetaClosureCapture>,
    pub(crate) generator: bool,
}

#[derive(Debug)]
pub struct IndexFunction {
    scope: IndexScope,
    generator: bool,
}

impl IndexFunction {
    /// Construct a new function.
    pub fn new() -> Self {
        Self {
            scope: IndexScope::new(),
            generator: false,
        }
    }
}

#[derive(Debug)]
enum IndexScopeLevel {
    /// A regular index scope.
    IndexScope(IndexScope),
    /// A marker for a closure boundary.
    ///
    /// The scope is the first scope inside of the closure.
    IndexClosure(IndexClosure),
    /// A function (completely isolated scope-wise).
    IndexFunction(IndexFunction),
}

/// An indexing scope.
pub struct IndexScopes {
    levels: Rc<RefCell<Vec<IndexScopeLevel>>>,
}

impl IndexScopes {
    /// Construct a new handler for indexing scopes.
    pub fn new() -> Self {
        Self {
            levels: Rc::new(RefCell::new(vec![IndexScopeLevel::IndexScope(
                IndexScope::new(),
            )])),
        }
    }

    /// Declare the given variable in the last scope.
    pub fn declare(&mut self, var: &str, span: Span) -> Result<(), CompileError> {
        let mut levels = self.levels.borrow_mut();

        let level = levels
            .last_mut()
            .ok_or_else(|| CompileError::internal("empty scopes", span))?;

        let scope = match level {
            IndexScopeLevel::IndexScope(scope) => scope,
            IndexScopeLevel::IndexClosure(closure) => &mut closure.scope,
            IndexScopeLevel::IndexFunction(fun) => &mut fun.scope,
        };

        scope.locals.insert(var.to_owned(), span);
        Ok(())
    }

    /// Mark that the given variable is used.
    pub fn mark_use(&mut self, var: &str) {
        let mut levels = self.levels.borrow_mut();
        let mut iter = levels.iter_mut().rev();

        let mut closures = Vec::new();
        let mut found = false;

        while let Some(level) = iter.next() {
            match level {
                IndexScopeLevel::IndexScope(scope) => {
                    if scope.locals.get(var).is_some() {
                        found = true;
                        break;
                    }
                }
                IndexScopeLevel::IndexClosure(closure) => {
                    if closure.existing.contains(var) {
                        found = true;
                        break;
                    }

                    if closure.scope.locals.get(var).is_some() {
                        found = true;
                        break;
                    }

                    closures.push(closure);
                }
                // NB: cannot capture variables outside of functions.
                IndexScopeLevel::IndexFunction(scope) => {
                    found = scope.scope.locals.get(var).is_some();
                    break;
                }
            }
        }

        // mark all traversed closures to capture the given variable.
        if found {
            for closure in closures {
                closure.captures.push(MetaClosureCapture {
                    ident: var.to_owned(),
                });

                let inserted = closure.existing.insert(var.to_owned());

                // NB: should be checked above, because closures where it's
                // already captured are skipped.
                debug_assert!(inserted);
            }
        }
    }

    /// Mark that a yield was used, meaning the encapsulating function is a
    /// generator.
    pub fn mark_yield(&mut self, span: Span) -> Result<(), CompileError> {
        let mut levels = self.levels.borrow_mut();
        let mut iter = levels.iter_mut().rev();

        while let Some(level) = iter.next() {
            match level {
                IndexScopeLevel::IndexFunction(fun) => {
                    fun.generator = true;
                    return Ok(());
                }
                IndexScopeLevel::IndexClosure(closure) => {
                    closure.generator = true;
                    return Ok(());
                }
                IndexScopeLevel::IndexScope(..) => (),
            }
        }

        Err(CompileError::YieldOutsideFunction { span })
    }

    /// Push a function.
    pub fn push_function(&mut self) -> IndexScopeGuard {
        self.levels
            .borrow_mut()
            .push(IndexScopeLevel::IndexFunction(IndexFunction::new()));

        IndexScopeGuard {
            levels: self.levels.clone(),
        }
    }

    /// Push a closure boundary.
    pub fn push_closure(&mut self) -> IndexScopeGuard {
        self.levels
            .borrow_mut()
            .push(IndexScopeLevel::IndexClosure(IndexClosure::new()));

        IndexScopeGuard {
            levels: self.levels.clone(),
        }
    }

    /// Push a new scope.
    pub fn push_scope(&mut self) -> IndexScopeGuard {
        self.levels
            .borrow_mut()
            .push(IndexScopeLevel::IndexScope(IndexScope::new()));

        IndexScopeGuard {
            levels: self.levels.clone(),
        }
    }
}