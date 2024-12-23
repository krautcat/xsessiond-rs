use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::sync::{Arc, Mutex};

use sysinfo::{ProcessesToUpdate, System};
use xcb;
use xcb::Xid;
use xcb::x::Window;

use crate::info::{ProcessInfo, WindowInfo};
use crate::x11_client::X11Client;

enum ApplicationErrorType {
    X11Error,
    ConnectionError,
    ProtocolError,
}

struct ApplicationError {
    kind: ApplicationErrorType,
    retcode: i32,
}

pub struct Application<'a> {
    x11_client: X11Client<'a>,

    sysinfo: System,
    processes: HashMap<ProcessInfo, HashSet<WindowInfo>>,

    is_running: Arc<Mutex<bool>>,
}

impl<'a> Application<'a> {
    pub fn new(
        running: Arc<Mutex<bool>>,
        x11_client: X11Client<'a>,
    ) -> Self {
        return Application {
            x11_client: x11_client,
            sysinfo: System::new_all(),
            processes: HashMap::new(), 
            is_running: running,
        };
    }

    fn run(self: &'_ mut Self) -> Result<i32, ApplicationError> {
        println!("HERE");
        let wm_client_list = self.x11_client.x11_connection.send_request(&xcb::x::InternAtom {
            only_if_exists: true,
            name: "_NET_CLIENT_LIST".as_bytes(),
        });
        let wm_client_list = self
            .x11_client
            .x11_connection
            .wait_for_reply(wm_client_list)
            .unwrap()
            .atom();
        assert!(wm_client_list != xcb::x::ATOM_NONE, "EWMH not supported");

        for screen in self.x11_client.x11_connection.get_setup().roots() {
            let window = screen.root();

            let pointer = self.x11_client.x11_connection
                .wait_for_reply(
                    self.x11_client.x11_connection.send_request(&xcb::x::QueryPointer { window }),
                )
                .unwrap();

            if pointer.same_screen() {
                let list = self.x11_client.x11_connection
                    .wait_for_reply(self.x11_client.x11_connection.send_request(&xcb::x::GetProperty {
                        delete: false,
                        window,
                        property: wm_client_list,
                        r#type: xcb::x::ATOM_NONE,
                        long_offset: 0,
                        long_length: 100,
                    }))
                    .unwrap();

                for client in list.value::<xcb::x::Window>() {
                    self.set_window_information(client);
                }
            }
        }

        while *self.is_running.lock().unwrap() {
            let event = match self.x11_client.x11_connection.wait_for_event() {
                Err(xcb::Error::Connection(_)) => {
                    return Err(ApplicationError {
                        kind: ApplicationErrorType::ConnectionError,
                        retcode: 10,
                    });
                }
                Err(xcb::Error::Protocol(_)) => {
                    return Err(ApplicationError {
                        kind: ApplicationErrorType::ProtocolError,
                        retcode: 11,
                    });
                }
                Ok(event) => event,
            };

            match event {
                xcb::Event::X(xcb::x::Event::CreateNotify(ev)) => {
                    let window = ev.window();
                    self.set_window_information(&window);
                }
                xcb::Event::X(xcb::x::Event::DestroyNotify(ev)) => {
                    let window = ev.window();
                    self.delete_window_information(&window);
                }
                xcb::Event::X(xcb::x::Event::PropertyNotify(_ev)) => {}
                _ => {}
            }
        }
        return Ok(0);
    }


    fn set_window_information(self: &mut Self, window: &Window) -> () {
        let x11_wininfo = self.x11_client.get_window_information(window);

        self.sysinfo.refresh_processes(ProcessesToUpdate::All);
        let process = self.sysinfo.process(x11_wininfo.process_id);
        match process {
            Some(p) => {
                let cmdline = p.cmd().join(OsStr::new(" ")).into_string().unwrap();

                let window_info = WindowInfo::new(
                    &x11_wininfo.x11_window_name,
                    x11_wininfo.x11_resource_id,
                    &x11_wininfo.x11_desktop_name,
                    x11_wininfo.x11_desktop_number,
                );
                let proc_info = ProcessInfo::new(cmdline, usize::try_from(x11_wininfo.process_id.as_u32()).unwrap());
                match self.processes.get_mut(&proc_info) {
                    Some(w_infos) => {
                        w_infos.insert(window_info);
                    }
                    None => {
                        let mut hash_set = HashSet::new();
                        hash_set.insert(window_info);
                        self.processes.insert(proc_info, hash_set);
                    }
                }
            }
            None => {}
        }
    }

    fn delete_window_information(self: &mut Self, window: &Window) -> () {
        let x11_wininfo = self.x11_client.get_window_information(window);

        self.sysinfo.refresh_processes(ProcessesToUpdate::All);
        let process = self.sysinfo.process(x11_wininfo.process_id);
        match process {
            Some(p) => {
                let cmdline = p.cmd().join(OsStr::new(" ")).into_string().unwrap();

                let proc_info = ProcessInfo::new(cmdline, usize::try_from(x11_wininfo.process_id.as_u32()).unwrap());
                let mut w_infos_for_proc = self.processes.get_mut(&proc_info);
                match w_infos_for_proc {
                    Some(w_infos) => {
                        for w_i in w_infos.iter() {
                            if w_i.window_xid == window.resource_id() {
                                w_infos_for_proc.unwrap().remove(w_i);
                            }
                        }

                        if w_infos.is_empty() {
                            self.processes.remove(&proc_info);
                        }
                    }
                    None => {}
                }
            }
            None => {}
        }
    }
}
