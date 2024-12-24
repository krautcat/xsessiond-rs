use std::clone::Clone;
use std::env;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use libc;
use nix::errno::Errno;
use nix::fcntl::{open, OFlag as FileOFlag};
use nix::sys::stat::{umask, Mode as FileMode};
use nix::unistd::{dup2, fork, setsid, ForkResult};
use signal_hook::consts::signal::*;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;
use signal_hook::iterator::exfiltrator::origin::WithOrigin;
use signal_hook::iterator::SignalsInfo;

use sessiond::application::{Application, ApplicationError};
use sessiond::x11_client::X11Client;

enum DaemonizeErrorType {
    Fork,
    NewSession,
    ChangeWorkingDirectory,
    OpenDevNull,
    RedirectToFile,
    RedirectStream,
}

struct DaemonizeError {
    error: DaemonizeErrorType,
    retcode: i32,
}

enum StdioImpl {
    DevNull,
    RedirectToFile(File),
    Keep,
}

pub struct Stdio {
    inner: StdioImpl,
}

impl Stdio {
    pub fn devnull() -> Self {
        Self {
            inner: StdioImpl::DevNull,
        }
    }

    pub fn keep() -> Self {
        Self {
            inner: StdioImpl::Keep,
        }
    }
}

impl From<File> for Stdio {
    fn from(file: File) -> Self {
        Self {
            inner: StdioImpl::RedirectToFile(file),
        }
    }
}

struct Daemon {
    stdin: Stdio,
    stdout: Stdio,
    stderr: Stdio,

    is_running: Arc<Mutex<bool>>,
}

impl Daemon {
    fn new(is_running: Arc<Mutex<bool>>) -> Self {
        return Daemon {
            stdin: Stdio::devnull(),
            stdout: Stdio::devnull(),
            stderr: Stdio::devnull(),

            is_running,
        };
    }

    fn daemonize(self: &Self) -> Result<ForkResult, DaemonizeError> {
        let pid = unsafe { fork() };
        match pid {
            Ok(ForkResult::Child) => {
                ();
            }

            Ok(ForkResult::Parent { .. }) => {
                return Ok(pid.unwrap());
            }

            Err(_) => {
                return Err(DaemonizeError {
                    error: DaemonizeErrorType::Fork,
                    retcode: Errno::last_raw(),
                });
            }
        }

        umask(FileMode::empty());

        match setsid() {
            Ok(_) => (),
            Err(_) => {
                return Err(DaemonizeError {
                    error: DaemonizeErrorType::NewSession,
                    retcode: Errno::last_raw(),
                })
            }
        }

        match env::set_current_dir(Path::new("/")) {
            Ok(_) => (),
            Err(_) => {
                return Err(DaemonizeError {
                    error: DaemonizeErrorType::ChangeWorkingDirectory,
                    retcode: Errno::last_raw(),
                })
            }
        }

        /*match self.redirect_standard_streams() {
            Ok(_) => (),
            Err(err) => return Err(err),
        }*/

        return Ok(pid.unwrap());
    }

    fn redirect_standard_streams(self: &Self) -> Result<(), DaemonizeError> {
        let devnull_fd = open(
            Path::new("/dev/null"),
            FileOFlag::O_RDWR,
            FileMode::S_IRUSR | FileMode::S_IWUSR,
        );
        match devnull_fd {
            Ok(_) => (),
            Err(_) => {
                return Err(DaemonizeError {
                    error: DaemonizeErrorType::OpenDevNull,
                    retcode: Errno::last_raw(),
                })
            }
        }

        let process_stdio = |fd, stdio: &Stdio| -> Result<i32, DaemonizeError> {
            match &stdio.inner {
                StdioImpl::DevNull => match dup2(devnull_fd.unwrap(), fd) {
                    Ok(res) => return Ok(res),
                    Err(_) => {
                        return Err(DaemonizeError {
                            error: DaemonizeErrorType::RedirectStream,
                            retcode: Errno::last_raw(),
                        })
                    }
                },
                StdioImpl::RedirectToFile(file) => {
                    let raw_fd = file.as_raw_fd();
                    match dup2(raw_fd, fd) {
                        Ok(res) => return Ok(res),
                        Err(_) => {
                            return Err(DaemonizeError {
                                error: DaemonizeErrorType::RedirectStream,
                                retcode: Errno::last_raw(),
                            })
                        }
                    }
                }
                StdioImpl::Keep => return Ok(-1),
            }
        };

        process_stdio(libc::STDIN_FILENO, &self.stdin)?;
        //process_stdio(libc::STDOUT_FILENO, &self.stdout)?;
        process_stdio(libc::STDERR_FILENO, &self.stderr)?;

        return Ok(());
    }

    fn run(self: &Self, application: &mut Application) -> Result<i32, ApplicationError> {
        return application.run();
    }
}

fn main() -> process::ExitCode {
    let daemon_is_running = Arc::new(Mutex::new(true));
    let daemon = Daemon::new(daemon_is_running.clone());

    match daemon.daemonize() {
        Ok(ForkResult::Child) => {
            // Make sure double CTRL+C and similar kills
            let term_now = Arc::new(AtomicBool::new(false));
            for sig in TERM_SIGNALS {
                // When terminated by a second term signal, exit with exit code 1.
                // This will do nothing the first time (because term_now is false).
                flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now)).unwrap();
                // But this will "arm" the above for the second time, by setting it to true.
                // The order of registering these is important, if you put this one first, it will
                // first arm and then terminate ‒ all in the first round.
                flag::register(*sig, Arc::clone(&term_now)).unwrap();
            }

            // Subscribe to all these signals with information about where they come from. We use the
            // extra info only for logging in this example (it is not available on all the OSes or at
            // all the occasions anyway, it may return `Unknown`).
            let mut sigs = vec![
                // Some terminal handling
                SIGTSTP, SIGCONT, SIGWINCH,
                // Reload of configuration for daemons ‒ um, is this example for a TUI app or a daemon
                // O:-)? You choose...
                SIGHUP, // Application-specific action, to print some statistics.
                SIGUSR1,
            ];
            sigs.extend(TERM_SIGNALS);
            let mut signals = SignalsInfo::<WithOrigin>::new(&sigs).unwrap();

            let daemon_thread = std::thread::spawn(move || {
                let mut x11_client = X11Client::new();
                
                let mut app = Application::new(daemon.is_running.clone(), &mut x11_client);
                
                return daemon.run(&mut app);
            });
            // This is the actual application that'll start in its own thread. We'll control it from
            // this thread based on the signals, but it keeps running.
            // This is called after all the signals got registered, to avoid the short race condition
            // in the first registration of each signal in multi-threaded programs.

            // Consume all the incoming signals. This happens in "normal" Rust thread, not in the
            // signal handlers. This means that we are allowed to do whatever we like in here, without
            // restrictions, but it also means the kernel believes the signal already got delivered, we
            // handle them in delayed manner. This is in contrast with eg the above
            // `register_conditional_shutdown` where the shutdown happens *inside* the handler.
            for info in &mut signals {
                // Will print info about signal + where it comes from.
                match info.signal {
                    term_sig => {
                        // These are all the ones left
                        *daemon_is_running.lock().unwrap() = false;
                        assert!(TERM_SIGNALS.contains(&term_sig));
                        break;
                    }
                }
            }

            match daemon_thread.join().unwrap() {
                Ok(_) => process::exit(0),
                Err(err) => process::exit(err.retcode),
            }
        }

        Ok(ForkResult::Parent { .. }) => {
            process::exit(0);
        }

        Err(err) => {
            process::exit(-err.retcode);
        }
    }
}
