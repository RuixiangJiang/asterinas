// SPDX-License-Identifier: MPL-2.0

use aster_frame::user::UserSpace;

use super::{PosixThread, PosixThreadExt, RealTimer};
use crate::{
    prelude::*,
    process::{
        posix_thread::name::ThreadName,
        signal::{
            constants::SIGALRM, sig_mask::SigMask, sig_queues::SigQueues,
            signals::kernel::KernelSignal,
        },
        Credentials, Process,
    },
    thread::{status::ThreadStatus, task, thread_table, Thread, Tid},
};

/// The builder to build a posix thread
pub struct PosixThreadBuilder {
    // The essential part
    tid: Tid,
    user_space: Arc<UserSpace>,
    process: Weak<Process>,
    credentials: Credentials,

    // Optional part
    thread_name: Option<ThreadName>,
    set_child_tid: Vaddr,
    clear_child_tid: Vaddr,
    sig_mask: SigMask,
    sig_queues: SigQueues,
    is_main_thread: bool,
}

impl PosixThreadBuilder {
    pub fn new(tid: Tid, user_space: Arc<UserSpace>, credentials: Credentials) -> Self {
        Self {
            tid,
            user_space,
            process: Weak::new(),
            credentials,
            thread_name: None,
            set_child_tid: 0,
            clear_child_tid: 0,
            sig_mask: SigMask::new_empty(),
            sig_queues: SigQueues::new(),
            is_main_thread: true,
        }
    }

    pub fn process(mut self, process: Weak<Process>) -> Self {
        self.process = process;
        self
    }

    pub fn thread_name(mut self, thread_name: Option<ThreadName>) -> Self {
        self.thread_name = thread_name;
        self
    }

    pub fn set_child_tid(mut self, set_child_tid: Vaddr) -> Self {
        self.set_child_tid = set_child_tid;
        self
    }

    pub fn clear_child_tid(mut self, clear_child_tid: Vaddr) -> Self {
        self.clear_child_tid = clear_child_tid;
        self
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn is_main_thread(mut self, is_main_thread: bool) -> Self {
        self.is_main_thread = is_main_thread;
        self
    }

    pub fn sig_mask(mut self, sig_mask: SigMask) -> Self {
        self.sig_mask = sig_mask;
        self
    }

    pub fn build(self) -> Arc<Thread> {
        let Self {
            tid,
            user_space,
            process,
            credentials,
            thread_name,
            set_child_tid,
            clear_child_tid,
            sig_mask,
            sig_queues,
            is_main_thread,
        } = self;

        let real_timer = RealTimer::new(move || {
            let process = {
                let Some(current_thread) = thread_table::get_thread(tid) else {
                    return;
                };
                let posix_thread = current_thread.as_posix_thread().unwrap();
                posix_thread.process()
            };

            let signal = KernelSignal::new(SIGALRM);
            process.enqueue_signal(signal);
        })
        .unwrap();

        let thread = Arc::new_cyclic(|thread_ref| {
            let task = task::create_new_user_task(user_space, thread_ref.clone());
            let status = ThreadStatus::Init;
            let posix_thread = PosixThread {
                process,
                is_main_thread,
                name: Mutex::new(thread_name),
                set_child_tid: Mutex::new(set_child_tid),
                clear_child_tid: Mutex::new(clear_child_tid),
                credentials,
                real_timer: Mutex::new(real_timer),
                sig_mask: Mutex::new(sig_mask),
                sig_queues,
                sig_context: Mutex::new(None),
                sig_stack: Mutex::new(None),
                robust_list: Mutex::new(None),
            };

            Thread::new(tid, task, posix_thread, status)
        });
        thread_table::add_thread(thread.clone());
        thread
    }
}
