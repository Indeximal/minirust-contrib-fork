# Machine Locks

This file describes how system locks work in MiniRust.
This does not match the actual lock implementations in Rust, it serves more as a specification for idealized locks.

## The Lock State

```rust
pub enum LockState {
    Unlocked,
    LockedBy(ThreadId),
}

type LockId = Int;
```

## Lock operations

The ThreadManager provides the key operations on locks.

```rust
impl<M: Memory> ThreadManager<M> {
    pub fn lock_create(&mut self) -> LockId {
        let id = self.locks.len();

        self.locks.push(LockState::Unlocked);

        id
    }

    pub fn lock_acquire(&mut self, lock_id: LockId) -> Result {
        let active = self.active_thread.unwrap();

        let Some(lock) = self.locks.get(lock_id) else {
            throw_ub!("acquiring non existing lock");
        };

        // If the lock is not taken, the lock gets acquired by the current (active) thread.
        // Otherwise, the active thread gets blocked.
        match lock {
            LockState::Unlocked => {
                self.locks.mutate_at(lock_id, |lock_state| {
                    *lock_state = LockState::LockedBy(active);
                });
            },
            LockState::LockedBy(_) => {
                self.threads.mutate_at(active, |thread| {
                    thread.state = ThreadState::BlockedOnLock(lock_id);
                });
            },
        }

        ret(())
    }

    pub fn lock_release(&mut self, lock_id: LockId) -> NdResult {
        let active = self.active_thread.unwrap();

        let Some(lock) = self.locks.get(lock_id) else {
            throw_ub!("release non existing lock");
        };

        match lock {
            LockState::LockedBy(thread_id) if thread_id == active => {
                // If any thread is blocked waiting for this lock, we want to unblock one of those.
                if self.threads.any(|thread| thread.state == ThreadState::BlockedOnLock(lock_id)) {
                    // We pick the thread that gets the lock.
                    let distr = libspecr::IntDistribution {
                        start: Int::ZERO,
                        end: Int::from(self.threads.len()),
                        divisor: Int::ONE,
                    };

                    let acquirer_id: ThreadId = pick(distr, |id: ThreadId| {
                        let Some(thread) = self.threads.get(id) else {
                            return false;
                        };

                        thread.state == ThreadState::BlockedOnLock(lock_id)
                    })?;

                    // We unblock the selected thread.
                    self.threads.mutate_at(acquirer_id, |thread| {
                        thread.state = ThreadState::Enabled;
                    });

                    // Rather than unlock and lock again we just change the lock owner.
                    self.locks.mutate_at(lock_id, |lock| {
                        *lock = LockState::LockedBy(thread_id);
                    });
                }
                
                else {
                    self.locks.mutate_at(lock_id, |lock| {
                        *lock = LockState::Unlocked;
                    });
                }


                ret(())
            },
            _ => throw_ub!("releasing non owned lock")
        }
    }
}
```

## The Intrinsics for Locks

This exposes the ThreadManager operations to the language as intrinsics.

The `Create` intrinsic. Used to create locks.

```rust
impl<M: Memory> Machine<M> {
    fn eval_intrinsic(
        &mut self,
        Intrinsic::Lock(LockIntrinsic::Create): Intrinsic,
        arguments: List<Value<M>>,
    ) -> NdResult<(Value<M>, Type)> {
        if arguments.len() > 0 {
            throw_ub!("invalid number of arguments for `LockIntrinsic::Create`");
        }

        let lock_id = self.thread_manager.lock_create();

        // FIXME: What if the id does not fit into a u32.
        // Currently the id is just a u32. This is hardcoded but no real thought is behind this.
        let id_size = Size::from_bits(Int::from(32)).unwrap();
        let id_type = Type::Int(IntType{
            signed: Signedness::Unsigned,
            size: id_size,
        });

        ret((Value::Int(lock_id), id_type))
    }
}
```

The `Acquire` intrinsic.

```rust
impl<M: Memory> Machine<M> {
    fn eval_intrinsic(
        &mut self,
        Intrinsic::Lock(LockIntrinsic::Acquire): Intrinsic,
        arguments: List<Value<M>>,
    ) -> NdResult<(Value<M>, Type)> {
        if arguments.len() != 1 {
            throw_ub!("invalid number of arguments for `LockIntrinsic::Acquire`");
        }

        let Value::Int(lock_id) = arguments[0] else {
            throw_ub!("invalid first argument to `LockIntrinsic::Acquire`");
        };

        self.thread_manager.lock_acquire(lock_id)?;

        ret(unit())
    }
}
```

The `Release` intrinsic.

```rust
impl<M: Memory> Machine<M> {
    fn eval_intrinsic(
        &mut self,
        Intrinsic::Lock(LockIntrinsic::Release): Intrinsic,
        arguments: List<Value<M>>,
    ) -> NdResult<(Value<M>, Type)> {
        if arguments.len() != 1 {
            throw_ub!("invalid number of arguments for `LockIntrinsic::Release`");
        }

        let Value::Int(lock_id) = arguments[0] else {
            throw_ub!("invalid first argument to `LockIntrinsic::Release`");
        };

        self.thread_manager.lock_release(lock_id)?;

        ret(unit())
    }
}
```