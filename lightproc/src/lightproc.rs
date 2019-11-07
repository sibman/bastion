use crate::proc_data::ProcData;
use crate::proc_ext::ProcFutureExt;
use crate::proc_handle::ProcHandle;
use crate::proc_stack::*;
use crate::raw_proc::RawProc;
use crate::recoverable_handle::RecoverableHandle;
use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::marker::PhantomData;
use std::mem;
use std::panic::AssertUnwindSafe;
use std::ptr::NonNull;

pub struct LightProc {
    /// A pointer to the heap-allocated proc.
    pub(crate) raw_proc: NonNull<()>,
}

unsafe impl Send for LightProc {}
unsafe impl Sync for LightProc {}

impl LightProc {
    pub fn recoverable<F, R, S>(
        future: F,
        schedule: S,
        stack: ProcStack,
    ) -> (LightProc, RecoverableHandle<R>)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
        S: Fn(LightProc) + Send + Sync + 'static,
    {
        let recovery_future = AssertUnwindSafe(future).catch_unwind();
        let (proc, handle) = Self::build(recovery_future, schedule, stack);
        (proc, RecoverableHandle(handle))
    }

    pub fn build<F, R, S>(future: F, schedule: S, stack: ProcStack) -> (LightProc, ProcHandle<R>)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
        S: Fn(LightProc) + Send + Sync + 'static,
    {
        let raw_proc = RawProc::allocate(stack, future, schedule);
        let proc = LightProc { raw_proc: raw_proc };
        let handle = ProcHandle {
            raw_proc: raw_proc,
            _marker: PhantomData,
        };
        (proc, handle)
    }

    pub fn schedule(self) {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;
        mem::forget(self);

        unsafe {
            ((*pdata).vtable.schedule)(ptr);
        }
    }

    pub fn run(self) {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;
        mem::forget(self);

        unsafe {
            ((*pdata).vtable.run)(ptr);
        }
    }

    pub fn cancel(&self) {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        unsafe {
            (*pdata).cancel();
        }
    }

    pub fn stack(&self) -> &ProcStack {
        let offset = ProcData::offset_stack();
        let ptr = self.raw_proc.as_ptr();

        unsafe {
            let raw = (ptr as *mut u8).add(offset) as *const ProcStack;
            &*raw
        }
    }
}

impl Debug for LightProc {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        fmt.debug_struct("LightProc")
            .field("pdata", unsafe { &(*pdata) })
            .field("stack", self.stack())
            .finish()
    }
}

impl Drop for LightProc {
    fn drop(&mut self) {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        unsafe {
            // Cancel the proc.
            (*pdata).cancel();

            // Drop the future.
            ((*pdata).vtable.drop_future)(ptr);

            // Drop the proc reference.
            ((*pdata).vtable.decrement)(ptr);
        }
    }
}