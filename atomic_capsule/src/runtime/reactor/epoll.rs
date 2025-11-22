//! # EpollBackend - Linux epoll(7) implementation
//!
//! **Platform-specific backend for Linux I/O multiplexing.**
//!
//! Uses Linux epoll for efficient event notification.
//! Edge-triggered mode (EPOLLET) for reduced false positives.

use super::{Event, Interest, ReactorBackend, ReactorError, ReactorResult};
use std::os::unix::io::RawFd;
use std::time::Duration;

extern "C" {
    fn epoll_create1(flags: i32) -> RawFd;
    fn epoll_ctl(epfd: RawFd, op: i32, fd: RawFd, event: *mut libc::epoll_event) -> i32;
    fn epoll_wait(
        epfd: RawFd,
        events: *mut libc::epoll_event,
        maxevents: i32,
        timeout: i32,
    ) -> i32;
}

// epoll constants
const EPOLL_CLOEXEC: i32 = 0o2000000;
const EPOLL_CTL_ADD: i32 = 1;
const EPOLL_CTL_MOD: i32 = 2;
const EPOLL_CTL_DEL: i32 = 3;
const EPOLLIN: u32 = 0x001;
const EPOLLOUT: u32 = 0x004;
const EPOLLET: u32 = 0x80000000;
const EPOLL_BATCH_SIZE: usize = 64;

/// Linux epoll backend
pub struct EpollBackend {
    epfd: RawFd,
}

impl EpollBackend {
    /// Create new epoll reactor
    ///
    /// # ASSUM Tags
    /// - `#ASSUME_EPOLL_CREATE_SAFE`: epoll_create1 returns valid FD or -1
    pub fn new() -> ReactorResult<Self> {
        unsafe {
            let epfd = epoll_create1(EPOLL_CLOEXEC);
            if epfd < 0 {
                // #VERIFY_EPOLL_ERROR: errno is set on -1
                return Err(ReactorError::OsError);
            }
            Ok(Self { epfd })
        }
    }

    /// Convert Interest to epoll event mask
    fn interests_to_mask(interests: Interest) -> u32 {
        let mut mask = EPOLLET; // Edge-triggered by default
        if interests.readable {
            mask |= EPOLLIN;
        }
        if interests.writable {
            mask |= EPOLLOUT;
        }
        mask
    }

    /// Convert epoll event mask to readable/writable flags
    fn mask_to_events(mask: u32) -> (bool, bool) {
        let readable = (mask & EPOLLIN) != 0;
        let writable = (mask & EPOLLOUT) != 0;
        (readable, writable)
    }
}

impl ReactorBackend for EpollBackend {
    /// Register FD with epoll
    fn register_fd(&mut self, fd: RawFd, interests: Interest) -> ReactorResult<()> {
        if fd < 0 {
            return Err(ReactorError::InvalidFd);
        }

        let mut event = libc::epoll_event {
            events: Self::interests_to_mask(interests),
            u64: fd as u64,
        };

        unsafe {
            let ret = epoll_ctl(self.epfd, EPOLL_CTL_ADD, fd, &mut event);
            if ret != 0 {
                // #VERIFY_EPOLL_ERROR: errno indicates error type
                return Err(ReactorError::OsError);
            }
        }

        Ok(())
    }

    /// Unregister FD from epoll
    fn unregister_fd(&mut self, fd: RawFd) -> ReactorResult<()> {
        if fd < 0 {
            return Err(ReactorError::InvalidFd);
        }

        let mut event = libc::epoll_event {
            events: 0,
            u64: fd as u64,
        };

        unsafe {
            let ret = epoll_ctl(self.epfd, EPOLL_CTL_DEL, fd, &mut event);
            if ret != 0 {
                return Err(ReactorError::OsError);
            }
        }

        Ok(())
    }

    /// Modify FD interest flags
    fn modify_fd(&mut self, fd: RawFd, interests: Interest) -> ReactorResult<()> {
        if fd < 0 {
            return Err(ReactorError::InvalidFd);
        }

        let mut event = libc::epoll_event {
            events: Self::interests_to_mask(interests),
            u64: fd as u64,
        };

        unsafe {
            let ret = epoll_ctl(self.epfd, EPOLL_CTL_MOD, fd, &mut event);
            if ret != 0 {
                return Err(ReactorError::OsError);
            }
        }

        Ok(())
    }

    /// Poll for events
    ///
    /// Performance target: <1μs per operation (amortized over batch)
    /// Collects up to EPOLL_BATCH_SIZE events
    ///
    /// # ASSUM Tags
    /// - `#ASSUME_EPOLL_WAIT_SAFE`: epoll_wait returns count or -1
    /// - `#ASSUME_EVENT_READY_BITS`: Events returned accurately reflect FD state
    fn poll_events(&mut self, timeout: Duration) -> ReactorResult<Vec<Event>> {
        let timeout_ms = timeout
            .as_millis()
            .try_into()
            .unwrap_or(i32::MAX)
            .min(i32::MAX);

        let mut events = vec![
            libc::epoll_event { events: 0, u64: 0 };
            EPOLL_BATCH_SIZE
        ];
        let mut result = Vec::new();

        unsafe {
            let nfds = epoll_wait(
                self.epfd,
                events.as_mut_ptr(),
                EPOLL_BATCH_SIZE as i32,
                timeout_ms,
            );

            if nfds < 0 {
                // #VERIFY_EPOLL_WAIT_ERROR: errno indicates error
                return Err(ReactorError::OsError);
            }

            // #VERIFY_EVENT_COUNT: nfds is valid count
            for i in 0..nfds as usize {
                let event = events[i];
                let fd = event.u64 as RawFd;
                let (readable, writable) = Self::mask_to_events(event.events);

                result.push(Event {
                    fd,
                    readable,
                    writable,
                });
            }
        }

        Ok(result)
    }
}

impl Drop for EpollBackend {
    fn drop(&mut self) {
        // #VERIFY_EPOLL_CLOSE: Close epoll FD on drop
        unsafe {
            if self.epfd >= 0 {
                libc::close(self.epfd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoll_backend_creation() {
        let backend = EpollBackend::new();
        assert!(backend.is_ok());
    }

    #[test]
    fn test_interests_to_mask() {
        let interests = Interest::read();
        let mask = EpollBackend::interests_to_mask(interests);
        assert!(mask & EPOLLIN != 0);
        assert!(mask & EPOLLET != 0);
    }

    #[test]
    fn test_mask_to_events() {
        let mask = EPOLLIN | EPOLLOUT | EPOLLET;
        let (readable, writable) = EpollBackend::mask_to_events(mask);
        assert!(readable);
        assert!(writable);
    }
}
