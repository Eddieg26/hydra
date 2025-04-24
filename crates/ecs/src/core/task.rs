use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

pub fn available_parallelism() -> usize {
    std::thread::available_parallelism()
        .unwrap_or(NonZeroUsize::new(1).unwrap())
        .get()
}

pub struct TaskPool(threadpool::ThreadPool);
static TASK_POOL: OnceLock<TaskPool> = OnceLock::new();

impl TaskPool {
    pub fn get() -> &'static Self {
        match TASK_POOL.get() {
            Some(pool) => pool,
            None => {
                TASK_POOL.get_or_init(|| Self(threadpool::ThreadPool::new(available_parallelism())))
            }
        }
    }

    pub fn try_get() -> Option<&'static Self> {
        TASK_POOL.get()
    }
}

impl std::ops::Deref for TaskPool {
    type Target = threadpool::ThreadPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub struct Scope<'scope, 'env> {
    scope: &'scope std::thread::Scope<'scope, 'env>,
    task_queue: Arc<Mutex<VecDeque<Box<dyn FnOnce() + Send + 'scope>>>>,
    running_count: Arc<AtomicUsize>,
    max_task_count: usize,
}

impl<'scope, 'env> Scope<'scope, 'env> {
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'scope,
    {
        if self.running_count.load(Ordering::SeqCst) < self.max_task_count {
            self.running_count.fetch_add(1, Ordering::SeqCst);
            let scope = self.clone();
            self.scope.spawn(move || {
                f();
                scope.finish();
            });
        } else {
            let scope = self.clone();
            let mut tasks = self.task_queue.lock().unwrap();
            tasks.push_back(Box::new(move || {
                f();
                scope.finish();
            }));
        }
    }

    fn pop_task(&self) -> Option<Box<dyn FnOnce() + Send + 'scope>> {
        let mut tasks = self.task_queue.lock().unwrap();

        tasks.pop_front()
    }

    fn finish(&self) {
        self.running_count.fetch_sub(1, Ordering::SeqCst);

        if let Some(task) = self.pop_task() {
            self.running_count.fetch_add(1, Ordering::SeqCst);
            task();
        }
    }
}

impl Drop for Scope<'_, '_> {
    fn drop(&mut self) {
        self.finish();
    }
}

pub fn scope<'env, F, T>(max_task_count: usize, f: F) -> T
where
    F: for<'scope> FnOnce(Scope<'scope, 'env>) -> T,
{
    std::thread::scope(move |scope| {
        let scope = Scope {
            scope,
            task_queue: Arc::default(),
            running_count: Arc::default(),
            max_task_count,
        };

        f(scope)
    })
}

#[allow(unused_imports)]
mod tests {
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_scoped() {
        let max_task_count = super::available_parallelism();
        let task_count = max_task_count * 3;
        let counter = Arc::new(Mutex::new(0));

        super::scope(max_task_count, |scope| {
            for _ in 0..task_count {
                let counter = Arc::clone(&counter);
                scope.spawn(move || {
                    let mut count = counter.lock().unwrap();
                    *count += 1;
                });
            }
        });

        assert_eq!(*counter.lock().unwrap(), task_count as usize);
    }
}
