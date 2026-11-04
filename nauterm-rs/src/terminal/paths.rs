use super::*;
use crate::{
    path_completion::{Entry, Query, WindowsShell},
    path_reader::{Cancellation, Source},
};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, PartialEq, Eq)]
struct Key {
    cwd: String,
    directory: String,
}
struct Current {
    input: String,
    query: Query,
    key: Key,
}
struct Cached {
    entries: Vec<Entry>,
    read: Instant,
}
pub(super) struct Paths {
    source: Source,
    current: Option<Current>,
    active: Option<(u64, Key)>,
    generation: u64,
    cancel: Option<Cancellation>,
    task: Option<Task<()>>,
    cache: HashMap<String, Cached>,
    order: VecDeque<String>,
    failed: HashSet<String>,
    dismissed: Option<String>,
}
impl Paths {
    pub fn new(source: Source) -> Self {
        Self {
            source,
            current: None,
            active: None,
            generation: 0,
            cancel: None,
            task: None,
            cache: HashMap::new(),
            order: VecDeque::new(),
            failed: HashSet::new(),
            dismissed: None,
        }
    }
    fn cancel(&mut self) {
        self.cancel = None;
        self.task = None;
        self.active = None;
    }
    fn store(&mut self, directory: String, entries: Vec<Entry>) {
        self.order.retain(|old| *old != directory);
        self.order.push_back(directory.clone());
        self.cache.insert(
            directory,
            Cached {
                entries,
                read: Instant::now(),
            },
        );
        while self.order.len() > 32 {
            if let Some(old) = self.order.pop_front() {
                self.cache.remove(&old);
                self.failed.remove(&old);
            }
        }
    }
}
pub struct CompletionError(pub String);
impl EventEmitter<CompletionError> for Terminal {}
impl Terminal {
    pub(super) fn dismiss_paths(&mut self, cx: &mut Context<Self>) {
        self.paths.dismissed = Some(self.composer.read(cx).value().to_string());
        self.paths.current = None;
        self.paths.cancel();
        self.completion
            .update(cx, |completion, cx| completion.clear_resolved(cx));
    }
    pub(super) fn refresh_paths(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = if self.phase() == Phase::Completing {
            self.completion
                .update(cx, |completion, cx| completion.query_input(window, cx))
        } else {
            None
        };
        let current = input.and_then(|input| {
            if self.paths.dismissed.as_deref() == Some(&input) {
                return None;
            }
            self.paths.dismissed = None;
            let cwd = self.working_directory.value();
            let (cwd, home, local) = self.paths.source.context(cwd.as_deref())?;
            let windows_shell = if cfg!(windows) && local {
                self.local_shell()
                    .as_deref()
                    .and_then(WindowsShell::from_path)
            } else {
                None
            };
            let query = if let Some(shell) = windows_shell {
                Query::parse_windows(&input, &cwd, home, shell)?
            } else {
                Query::parse(&input, &cwd, home, local)?
            };
            let key = Key {
                cwd,
                directory: query.directory.clone(),
            };
            Some(Current { input, query, key })
        });
        let Some(current) = current else {
            self.paths.current = None;
            self.paths.cancel();
            self.completion
                .update(cx, |completion, cx| completion.clear_resolved(cx));
            return;
        };
        if self
            .paths
            .current
            .as_ref()
            .is_some_and(|old| old.key == current.key && old.input == current.input)
        {
            return;
        }
        let same_job = self
            .paths
            .active
            .as_ref()
            .is_some_and(|(_, key)| *key == current.key);
        if !same_job {
            self.paths.cancel();
        }
        let input = current.input.clone();
        let key = current.key.clone();
        let cached = self
            .paths
            .cache
            .get(&key.directory)
            .filter(|cached| cached.read.elapsed() < Duration::from_secs(5));
        let values = cached
            .map(|cached| {
                current
                    .query
                    .candidates(&cached.entries, self.appearance.composer_max_suggestions)
            })
            .unwrap_or_else(|| current.query.pending());
        let needs_read = cached.is_none() && !same_job;
        self.paths.current = Some(current);
        self.completion.update(cx, |completion, cx| {
            completion.set_resolved(&input, values, cx)
        });
        if !needs_read {
            return;
        }
        let (request, cancel) = match self.paths.source.request(key.directory.clone()) {
            Ok(request) => request,
            Err(_) => return,
        };
        self.paths.generation += 1;
        let generation = self.paths.generation;
        self.paths.active = Some((generation, key.clone()));
        self.paths.cancel = Some(cancel);
        self.paths.task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(160)).await;
            let result = cx.background_spawn(async move {request.read()}).await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.paths.active.as_ref() != Some(&(generation, key.clone())) {return;}
                this.paths.cancel = None; this.paths.active = None;
                match result {
                    Ok(entries) => {this.paths.failed.remove(&key.directory); this.paths.store(key.directory.clone(), entries);}
                    Err(_) => {
                        this.paths.store(key.directory.clone(), vec![]);
                        if this.paths.failed.insert(key.directory.clone()) {cx.emit(CompletionError("Path completion is unavailable for this directory. History and snippets are still available.".into()));}
                    }
                }
                if let Some(current) = this.paths.current.as_ref().filter(|current| current.key == key) {
                    let entries = &this.paths.cache[&key.directory].entries;
                    let values = current.query.candidates(entries, this.appearance.composer_max_suggestions);
                    let input = current.input.clone();
                    this.completion.update(cx, |completion, cx| completion.set_resolved(&input, values, cx));
                }
            });
        }));
    }
}
