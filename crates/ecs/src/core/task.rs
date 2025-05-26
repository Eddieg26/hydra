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

pub mod inner {
    use async_task::FallibleTask;
    use concurrent_queue::ConcurrentQueue;
    use smol::future::FutureExt;
    use std::{
        any::Any,
        num::NonZeroUsize,
        ops::{Deref, Range},
        panic::{AssertUnwindSafe, UnwindSafe},
        sync::Arc,
        task,
        thread::JoinHandle,
    };

    pub fn available_parallelism() -> NonZeroUsize {
        std::thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap())
    }

    pub struct TaskPoolBuilder {
        /// Name for the thread pool, useful for debugging or logging.
        pool_name: Option<String>,
        /// Number of threads to spawn in the pool.
        /// If set to `None`, the number of threads will be determined by the system's available parallelism.
        pool_size: Option<usize>,
        /// Stack size for each thread in the pool.
        stack_size: Option<usize>,
        /// Callback invoked when a thread is spawned.
        on_spawn: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
        /// Callback invoked when a thread is spawned.
        on_destroy: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
    }

    impl TaskPoolBuilder {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn name(mut self, name: impl ToString) -> Self {
            self.pool_name = Some(name.to_string());
            self
        }

        pub fn size(mut self, size: usize) -> Self {
            self.pool_size = Some(size);
            self
        }

        pub fn stack_size(mut self, size: usize) -> Self {
            self.stack_size = Some(size);
            self
        }

        pub fn on_spawn(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
            self.on_spawn = Some(Arc::new(f));
            self
        }

        pub fn on_destroy(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
            self.on_destroy = Some(Arc::new(f));
            self
        }

        pub fn build(self) -> TaskPool {
            TaskPool::new(self)
        }
    }

    impl Default for TaskPoolBuilder {
        fn default() -> Self {
            Self {
                pool_name: None,
                pool_size: None,
                stack_size: None,
                on_spawn: None,
                on_destroy: None,
            }
        }
    }

    pub struct TaskPool {
        executor: Arc<smol::Executor<'static>>,
        handles: Vec<JoinHandle<()>>,
        sender: smol::channel::Sender<()>,
    }

    impl TaskPool {
        thread_local! {
            static LOCAL_EXECUTOR: smol::LocalExecutor<'static> = const { smol::LocalExecutor::new() };
        }

        pub fn builder() -> TaskPoolBuilder {
            TaskPoolBuilder::default()
        }

        pub fn new(builder: TaskPoolBuilder) -> Self {
            let (sender, receiver) = smol::channel::unbounded::<()>();

            let executor = Arc::new(smol::Executor::new());

            let pool_size = builder.pool_size.unwrap_or(available_parallelism().get());

            let handles = (0..pool_size)
                .map(|index| {
                    let executor = executor.clone();
                    let receiver = receiver.clone();
                    let on_spawn = builder.on_spawn.clone();
                    let on_destroy = builder.on_destroy.clone();
                    let stack_size = builder.stack_size;
                    let name = match builder.pool_name.as_deref() {
                        Some(name) => format!("{name}: {index}"),
                        None => format!("Thread: {index}"),
                    };

                    let mut builder = std::thread::Builder::new().name(name);
                    if let Some(size) = stack_size {
                        builder = builder.stack_size(size);
                    }

                    builder
                        .spawn(move || {
                            if let Some(on_spawn) = on_spawn {
                                on_spawn();
                                drop(on_spawn);
                            }

                            loop {
                                let result = std::panic::catch_unwind(|| {
                                    let runner = async {
                                        loop {
                                            executor.tick().await;
                                        }
                                    };

                                    smol::block_on(runner.or(receiver.recv()))
                                });

                                if let Ok(result) = result {
                                    result.unwrap_err();
                                    break;
                                }
                            }

                            if let Some(on_destroy) = on_destroy {
                                on_destroy();
                                drop(on_destroy);
                            }
                        })
                        .expect("Failed to spawn thread.")
                })
                .collect();

            Self {
                executor,
                handles,
                sender,
            }
        }

        pub fn spawn<T: Send + 'static>(
            &self,
            task: impl Future<Output = T> + Send + 'static,
        ) -> Task<T> {
            Task::new(self.executor.spawn(task))
        }

        pub fn spawn_local<T: Send + 'static>(
            &self,
            task: impl Future<Output = T> + 'static,
        ) -> Task<T> {
            Task::new(Self::LOCAL_EXECUTOR.with(|executor| executor.spawn(task)))
        }

        pub fn with_local<T>(f: impl FnOnce(&smol::LocalExecutor) -> T) -> T {
            Self::LOCAL_EXECUTOR.with(f)
        }

        pub fn scope<'scope, 'env: 'scope, T: Send + 'static>(
            &'scope self,
            f: impl FnOnce(Scope<'scope, 'env, T>),
        ) -> Vec<T> {
            use std::mem::transmute;

            let executor: &'scope smol::Executor<'scope> =
                unsafe { transmute(self.executor.deref()) };

            let local_executor = Self::with_local(|local| {
                let local_executor: &'scope smol::Executor<'scope> = unsafe { transmute(local) };
                local_executor
            });

            let tasks =
                ConcurrentQueue::<FallibleTask<Result<T, Box<dyn Any + Send>>>>::unbounded();

            {
                let tasks: &'scope &ConcurrentQueue<FallibleTask<Result<T, Box<dyn Any + Send>>>> =
                    unsafe { transmute(&tasks) };

                let scope = Scope::<T> {
                    executor,
                    local_executor,
                    tasks: &tasks,
                    _marker: Default::default(),
                };

                f(scope);
            };

            if tasks.is_empty() {
                return vec![];
            }

            let values = async {
                let mut values = Vec::with_capacity(tasks.len());
                while let Ok(task) = tasks.pop() {
                    match task.await {
                        Some(Ok(value)) => values.push(value),
                        Some(Err(error)) => std::panic::resume_unwind(error),
                        None => panic!("Task was cancelled"),
                    }
                }

                values
            };

            smol::block_on(async {
                let result = async {
                    loop {
                        let runner = async {
                            loop {
                                local_executor.tick().await;
                            }
                        };

                        let _ = AssertUnwindSafe(runner).catch_unwind().await;
                    }
                };

                result.or(values).await
            })
        }

        pub fn size(&self) -> usize {
            self.handles.len()
        }
    }

    impl Drop for TaskPool {
        fn drop(&mut self) {
            self.sender.close();

            let panicking = std::thread::panicking();

            for handle in self.handles.drain(..) {
                let res = handle.join();
                if !panicking {
                    res.expect("Thread panicked during shutdown");
                }
            }
        }
    }

    pub struct Scope<'scope, 'env: 'scope, T = ()> {
        executor: &'scope smol::Executor<'scope>,
        local_executor: &'scope smol::Executor<'scope>,
        tasks: &'scope ConcurrentQueue<FallibleTask<Result<T, Box<dyn std::any::Any + Send>>>>,
        _marker: std::marker::PhantomData<&'env mut ()>,
    }

    impl<'scope, 'env: 'scope, T: Send + 'static> Scope<'scope, 'env, T> {
        pub fn spawn(&self, task: impl Future<Output = T> + Send + 'scope) {
            let task = self
                .executor
                .spawn(AssertUnwindSafe(task).catch_unwind())
                .fallible();

            self.tasks.push(task).unwrap();
        }

        pub fn spawn_local(&self, task: impl Future<Output = T> + Send + 'scope) {
            let task = self
                .local_executor
                .spawn(AssertUnwindSafe(task).catch_unwind())
                .fallible();

            self.tasks.push(task).unwrap();
        }

        pub fn execute_local(
            &self,
            signal: impl Future<Output = Vec<T>> + Send + UnwindSafe,
        ) -> Result<Vec<T>, Box<dyn Any + Send + 'static>> {
            std::panic::catch_unwind(|| {
                let runner = async {
                    loop {
                        self.local_executor.tick().await;
                    }
                };

                smol::block_on(runner.or(signal))
            })
        }
    }

    impl<'scope, 'env, T> Drop for Scope<'scope, 'env, T> {
        fn drop(&mut self) {
            smol::block_on(async {
                while let Ok(task) = self.tasks.pop() {
                    task.cancel().await;
                }
            });
        }
    }

    pub struct Task<T>(smol::Task<T>);

    impl<T> Task<T> {
        pub fn new(task: smol::Task<T>) -> Self {
            Self(task)
        }

        pub fn detach(self) {
            self.0.detach();
        }

        pub async fn cancel(self) -> Option<T> {
            self.0.cancel().await
        }

        pub fn is_finished(&self) -> bool {
            self.0.is_finished()
        }
    }

    impl<T> Future for Task<T> {
        type Output = T;

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            std::pin::Pin::new(&mut self.0).poll(cx)
        }
    }

    macro_rules! task_pool_type {
        ($name:ident, $static_name:ident) => {
            pub struct $name($crate::core::task::inner::TaskPool);
            static $static_name: std::sync::OnceLock<$name> = std::sync::OnceLock::new();

            impl $name {
                /// Initialize the pool singleton. Only the first call will set the pool.
                pub fn init(pool: $crate::core::task::inner::TaskPool) {
                    $static_name
                        .set(Self(pool))
                        .ok()
                        .expect(concat!(stringify!($name), " already initialized"));
                }

                /// Get a reference to the singleton pool.
                pub fn get() -> &'static $name {
                    $static_name
                        .get()
                        .expect(concat!(stringify!($name), " has not been initialized."))
                }
            }

            impl std::ops::Deref for $name {
                type Target = $crate::core::task::inner::TaskPool;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }
        };
    }

    task_pool_type!(IoTaskPool, IO_TASK_POOL);

    task_pool_type!(CpuTaskPool, CPU_TASK_POOL);

    pub use scope::scoped;

    pub mod scope {
        use super::{Scope, TaskPool};
        use std::sync::OnceLock;

        static SCOPED_TASK_POOL: OnceLock<TaskPool> = OnceLock::new();

        pub fn init(pool: TaskPool) {
            SCOPED_TASK_POOL
                .set(pool)
                .ok()
                .expect("Scoped task pool already initialized");
        }

        pub fn scoped<'scope, 'env: 'scope, T: Send + 'static>(
            f: impl FnOnce(Scope<'scope, 'env, T>),
        ) -> Vec<T> {
            let pool = SCOPED_TASK_POOL
                .get()
                .expect("Scoped task pool not initialized");

            pool.scope(f)
        }
    }

    pub struct TaskPoolSizeConfig {
        /// Minimum number of threads in the pool.
        pub min: usize,
        /// Maximum number of threads in the pool.
        pub max: usize,
        /// Determines the amount of remaining threads to allocate to this pool.
        /// Amount of threads will be calculated as `remaining threads * (weight / total_weight)`.
        pub weight: f32,
    }

    impl TaskPoolSizeConfig {
        pub fn get_size(&self, thread_count: usize, total_weight: f32) -> usize {
            let size = (thread_count as f32 * (self.weight / total_weight)) as usize;

            size.clamp(self.min, self.max)
        }
    }

    impl Default for TaskPoolSizeConfig {
        fn default() -> Self {
            Self {
                min: 1,
                max: 1,
                weight: 1.0,
            }
        }
    }

    pub struct TaskPoolSettings {
        /// Range of thread counts for the task pool.
        pub thread_count: Range<usize>,
        /// Configuration for CPU-bound tasks.
        pub cpu: TaskPoolSizeConfig,
        /// Configuration for I/O-bound tasks.
        pub io: TaskPoolSizeConfig,
        /// Configuration for scoped tasks.
        pub scoped: TaskPoolSizeConfig,
    }

    impl TaskPoolSettings {
        pub fn init_task_pools(&self) {
            let total_thread_count = available_parallelism()
                .get()
                .clamp(self.thread_count.start, self.thread_count.end);

            let total_weight = self.cpu.weight + self.io.weight + self.scoped.weight;

            CpuTaskPool::init(
                TaskPool::builder()
                    .name("CPU Task Pool")
                    .size(self.cpu.get_size(total_thread_count, total_weight))
                    .build(),
            );

            IoTaskPool::init(
                TaskPool::builder()
                    .name("I/O Task Pool")
                    .size(self.io.get_size(total_thread_count, total_weight))
                    .build(),
            );

            scope::init(
                TaskPool::builder()
                    .name("Scoped Task Pool")
                    .size(self.scoped.get_size(total_thread_count, total_weight))
                    .build(),
            );
        }
    }

    impl Default for TaskPoolSettings {
        fn default() -> Self {
            Self {
                thread_count: 1..available_parallelism().get(),
                cpu: TaskPoolSizeConfig::default(),
                io: TaskPoolSizeConfig::default(),
                scoped: TaskPoolSizeConfig::default(),
            }
        }
    }
}
