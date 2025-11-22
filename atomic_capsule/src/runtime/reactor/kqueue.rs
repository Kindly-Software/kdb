//! # KqueueBackend - BSD/macOS kqueue(2) implementation
//!
//! **Platform-specific backend for BSD/macOS I/O multiplexing.**
//!
//! Uses BSD kqueue for efficient event notification.
//! Supports both read and write events via kevent struct.

use super::{Event, Interest, ReactorBackend, ReactorError, ReactorResult};
use std::os::unix::io::RawFd;
use std::time::Duration;

extern "C" {
    fn kqueue() -> RawFd;
    fn kevent(
        kq: RawFd,
        changelist: *const libc::kevent,
        nchanges: i32,
        eventlist: *mut libc::kevent,
        nevents: i32,
        timeout: *const libc::timespec,
    ) -> i32;
}

// kqueue constants
const KQUEUE_BATCH_SIZE: usize = 64;

/// BSD/macOS kqueue backend
pub struct KqueueBackend {
    kq: RawFd,
}

impl KqueueBackend {
    /// Create new kqueue reactor
    ///
    /// # ASSUM Tags
    /// - `#ASSUME_KQUEUE_SAFE`: kqueue() returns valid FD or -1
    pub fn new() -> ReactorResult<Self> {
        unsafe {
            let kq = kqueue();
            if kq < 0 {
                // #VERIFY_KQUEUE_ERROR: errno is set on -1
                return Err(ReactorError::OsError);
            }
            Ok(Self { kq })
        }
    }
}

impl ReactorBackend for KqueueBackend {
    /// Register FD with kqueue
    fn register_fd(&mut self, fd: RawFd, interests: Interest) -> ReactorResult<()> {
        if fd < 0 {
            return Err(ReactorError::InvalidFd);
        }

        let mut events = Vec::new();

        // Add read event if interested
        if interests.readable {
            events.push(libc::kevent {
                ident: fd as libc::uintptr_t,
                filter: libc::EVFILT_READ,
                flags: libc::EV_ADD | libc::EV_ENABLE,
                fflags: 0,
                data: 0,
                udata: fd as *mut libc::c_void,
            });
        }

        // Add write event if interested
        if interests.writable {
            events.push(libc::kevent {
                ident: fd as libc::uintptr_t,
                filter: libc::EVFILT_WRITE,
                flags: libc::EV_ADD | libc::EV_ENABLE,
                fflags: 0,
                data: 0,
                udata: fd as *mut libc::c_void,
            });
        }

        if events.is_empty() {
            return Ok(());
        }

        unsafe {
            let ret = kevent(
                self.kq,
                events.as_ptr(),
                events.len() as i32,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            );

            if ret < 0 {
                // #VERIFY_KEVENT_ERROR: errno indicates error
                return Err(ReactorError::OsError);
            }
        }

        Ok(())
    }

    /// Unregister FD from kqueue
    fn unregister_fd(&mut self, fd: RawFd) -> ReactorResult<()> {
        if fd < 0 {
            return Err(ReactorError::InvalidFd);
        }

        let events = [
            libc::kevent {
                ident: fd as libc::uintptr_t,
                filter: libc::EVFILT_READ,
                flags: libc::EV_DELETE,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            },
            libc::kevent {
                ident: fd as libc::uintptr_t,
                filter: libc::EVFILT_WRITE,
                flags: libc::EV_DELETE,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            },
        ];

        unsafe {
            let ret = kevent(
                self.kq,
                events.as_ptr(),
                2,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            );

            if ret < 0 {
                return Err(ReactorError::OsError);
            }
        }

        Ok(())
    }

    /// Modify FD interest flags
    fn modify_fd(&mut self, fd: RawFd, interests: Interest) -> ReactorResult<()> {
        // In kqueue, we need to delete and re-add with new flags
        self.unregister_fd(fd)?;
        self.register_fd(fd, interests)?;
        Ok(())
    }

    /// Poll for events
    ///
    /// Performance target: <1μs per operation (amortized over batch)
    /// Collects up to KQUEUE_BATCH_SIZE events
    ///
    /// # ASSUM Tags
    /// - `#ASSUME_KEVENT_SAFE`: kevent returns count or -1
    /// - `#ASSUME_EVENT_READY_BITS`: Events returned accurately reflect FD state
    fn poll_events(&mut self, timeout: Duration) -> ReactorResult<Vec<Event>> {
        let mut timespec = libc::timespec {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_nsec: timeout.subsec_nanos() as libc::c_long,
        };

        let mut events = vec![
            libc::kevent {
                ident: 0,
                filter: 0,
                flags: 0,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            };
            KQUEUE_BATCH_SIZE
        ];
        let mut result = Vec::new();

        unsafe {
            let nfds = kevent(
                self.kq,
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                KQUEUE_BATCH_SIZE as i32,
                &mut timespec,
            );

            if nfds < 0 {
                // #VERIFY_KEVENT_WAIT_ERROR: errno indicates error
                return Err(ReactorError::OsError);
            }

            // #VERIFY_EVENT_COUNT: nfds is valid count
            // Track which FDs we've already added to result to avoid duplicates
            // (kqueue sends separate events for read and write)
            let mut processed_fds = std::collections::HashSet::new();

            for i in 0..nfds as usize {
                let event = events[i];
                let fd = event.udata as RawFd;

                // Skip if we've already processed this FD (only add once with both flags)
                if processed_fds.contains(&fd) {
                    continue;
                }

                let mut readable = false;
                let mut writable = false;

                // Check current event
                if event.filter == libc::EVFILT_READ {
                    readable = true;
                }
                if event.filter == libc::EVFILT_WRITE {
                    writable = true;
                }

                // Look ahead for paired event
                for j in (i + 1)..nfds as usize {
                    let next_event = events[j];
                    if next_event.udata as RawFd == fd {
                        if next_event.filter == libc::EVFILT_READ {
                            readable = true;
                        }
                        if next_event.filter == libc::EVFILT_WRITE {
                            writable = true;
                        }
                    }
                }

                result.push(Event {
                    fd,
                    readable,
                    writable,
                });

                processed_fds.insert(fd);
            }
        }

        Ok(result)
    }
}

impl Drop for KqueueBackend {
    fn drop(&mut self) {
        // #VERIFY_KQUEUE_CLOSE: Close kqueue FD on drop
        unsafe {
            if self.kq >= 0 {
                libc::close(self.kq);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kqueue_backend_creation() {
        let backend = KqueueBackend::new();
        assert!(backend.is_ok());
    }
}
