//! L01 — Runtime mental model.
//!
//! Your task lives in `README.md`. Replace this stub with your toy executor
//! and your hand-written futures. Do not pull in `tokio` for this lesson.
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::task::{Poll, Wake, Waker, Context};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::fmt::{self, Display};
use std::thread::{self, Thread};
use std::default::Default;
use std::time::{Duration, Instant};
use pin_project_lite::pin_project;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TaskID(u64);

impl Display for TaskID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct Task {
    fut: Pin<Box<dyn Future<Output = ()>>>,
    waker: Waker,
}

// Note the design pattern here
pub struct TaskWaker {
    id: TaskID,
    shared: Arc<Shared>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.enqueue();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.enqueue();
    }
}

impl TaskWaker {
    fn enqueue(&self) {
        self.shared.ready_queue.lock().unwrap().push_back(self.id);
        self.shared.executor_thread.unpark();
    }
}

pub struct Shared {
    executor_thread: Thread,
    ready_queue: Mutex<VecDeque<TaskID>>,
}

pub struct Executor {
    task_map: HashMap<TaskID, Task>,
    next_id: u64,
    shared: Arc<Shared>,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            task_map: HashMap::new(),
            next_id: 0,
            shared: Arc::new(Shared {
                executor_thread: thread::current(),
                ready_queue: Mutex::new(VecDeque::new()),
            }),
        }
    }

    pub fn spawn(&mut self, fut: impl Future<Output = ()> + 'static) -> TaskID {
        let new_id = TaskID(self.next_id);
        let task = Task {
            fut: Box::pin(fut),
            waker: Waker::from(Arc::new(TaskWaker {
                id: new_id,
                shared: self.shared.clone(),
            }))
        };
        self.task_map.insert(new_id, task);
        self.shared.ready_queue.lock().unwrap().push_back(new_id);
        self.next_id += 1;
        new_id
    }

    pub fn run(&mut self) {
        while !self.task_map.is_empty() {
            // Note this block dropping the queue guard early
            let id = {
                let mut queue = self.shared.ready_queue.lock().unwrap();
                queue.pop_front()
            };

            match id {
                Some(id) => {
                    if let Poll::Ready(_) = self.poll_one(id) {
                        self.task_map.remove(&id);
                        println!("Task {} finished.\n", id);
                    }
                },
                // Formally a continue; recheck followed here, unnecessary by clippy
                None => std::thread::park()
            }
        }
    }

    // TODO: Do I need Pin<&mut Self> or simply &mut self
    pub fn poll_one(&mut self, id: TaskID) -> Poll<()> {
        let task = self.task_map.get_mut(&id).unwrap();
        let mut cx = Context::from_waker(&task.waker);
        task.fut.as_mut().poll(&mut cx)
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

struct Ready<T>(Option<T>);

impl<T> Ready<T> {
    fn new(item: T) -> Self {
        Self(Some(item))
    }
}

impl<T> Unpin for Ready<T> {}

impl<T: Clone> Future for Ready<T> {
    type Output = T; 

    // Note direct return here: each future polled once before executor parks
    // Note the mutability binding of self and the customary _ before cx to notify the compiler of unused
    // variable
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Auto-ref + auto-deref to get &mut Option<T> from Pin<&mut Self>
        Poll::Ready(self.0.take().expect("Item ready"))
    }
}

struct Yield {
    yielded: bool,
}

impl Yield {
    const fn new() -> Self {
        Self {
            yielded: false,
        }
    }
}

impl Default for Yield {
    fn default() -> Self {
        Self::new()
    }
}

impl Unpin for Yield {}

impl Future for Yield {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            return Poll::Ready(())
        }
        // TODO: Same mutability problem again: contract immutable Pin
        self.yielded = true;
        // TODO: Examine execution order: why calling wake here wakes a parked executor despite
        // Pending not yet returned to executor (so that theoretically executor not parked yet)
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

struct Delay {
    until: Instant,
    delayed: bool,
}

impl Delay {
    fn new(until: Instant) -> Self {
        Self {
            until,
            delayed: false,
        }
    }
}

impl Default for Delay {
    fn default() -> Self {
        Self::new(Instant::now() + Duration::from_secs(1))
    }
}

impl Unpin for Delay {}

impl Future for Delay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Handles both Ready by default and slept cases
        if Instant::now() >= self.until {
            return Poll::Ready(())
        }
        if !self.delayed {
            // disjoint capture not activated for self here since self: Pin<&mut Self> triggers
            // deref_mut() as a borrow, not directly taking ownership of fields
            let until = self.until;
            let waker = cx.waker().clone();
            thread::spawn(move ||{
                let dur = until.saturating_duration_since(Instant::now());
                // sleep(Duration::ZERO) is a no-op
                thread::sleep(dur);
                waker.wake_by_ref();
            });
            self.delayed = true;
        }
        Poll::Pending
    }
}

pin_project! {
    struct Join2<A, B> {
        // TODO: Pin wrapped or simple Box here?
        #[pin]
        fut1: Pin<Box<dyn Future<Output = A> + 'static>>,
        #[pin]
        fut2: Pin<Box<dyn Future<Output = B> + 'static>>,
        fut1_out: Option<A>,
        fut2_out: Option<B>,
    }
}

impl<A, B> Join2<A, B> {
    fn new(fut1: impl Future<Output = A> + 'static, fut2: impl Future<Output = B> + 'static) -> Self {
        Self {
            fut1: Box::pin(fut1),
            fut2: Box::pin(fut2),
            fut1_out: None,
            fut2_out: None,
        }
    }
}

impl<A, B> Future for Join2<A, B> {
    type Output = ();

    // wake() called by subfutures, pass on context so that subfutures re-enqueues Join2 on Ready
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        
        if this.fut1_out.is_none() {
            if let Poll::Ready(v) = this.fut1.poll(cx) {
                *this.fut1_out = Some(v);
            }
        }

        if this.fut2_out.is_none() {
            if let Poll::Ready(v) = this.fut2.poll(cx) {
                *this.fut2_out = Some(v);
            }
        }

        if this.fut1_out.is_none() || this.fut2_out.is_none() {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}


fn main() {
    let mut ex = Executor::new();

    let pre_ex = Instant::now();

    ex.spawn(async {
        let v = Ready::new(String::from("Always ready!")).await;
        println!("Task ready done with return \"{v}\"");
    });
    ex.spawn(async {
        Yield::new().await;
        println!("Task yield done");
    });
    ex.spawn(async {
        let past = Instant::now().checked_sub(Duration::from_secs(1)).unwrap_or_else(Instant::now);
        Delay::new(past).await;
        println!("Task instant yield by delaying negative done");
    });
    ex.spawn(async {
        Delay::new(Instant::now() + Duration::from_secs(1)).await;
        println!("Task delaying 1s done");
    });
    ex.spawn(async {
        let fut1 = Ready::new(String::from("Random"));
        let fut2 = Delay::new(Instant::now() + Duration::from_secs(1));
        Join2::new(fut1, fut2).await;
        println!("Task Joining Ready and delay 1s done");
    });

    ex.run();
    
    let now = Instant::now();
    println!("Execution done with time {}s", now.duration_since(pre_ex).as_secs());

}
