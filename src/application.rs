use std::sync::{Arc, Mutex};

use xcb;

use crate::info::ProcessesWindowsInfo;
use crate::x11_client::X11Client;

pub enum ApplicationErrorType {
    X11Error,
    ConnectionError,
    ProtocolError,
}

pub struct ApplicationError {
    pub kind: ApplicationErrorType,
    pub retcode: i32,
}
pub struct Application<'a> {
    x11_client: &'a X11Client<'a>,

    proc_win_info: ProcessesWindowsInfo,

    is_running: Arc<Mutex<bool>>,
}

impl<'a> Application<'a> {
    pub fn new(running: Arc<Mutex<bool>>, x11_client: &'a mut X11Client<'a>) -> Self {
        return Application {
            x11_client: x11_client.connect(),
            proc_win_info: ProcessesWindowsInfo::new(),
            is_running: running,
        };
    }

    pub fn run(self: &mut Self) -> Result<i32, ApplicationError> {
        let x11_conn = &self.x11_client.x11_connection;
        
        let wm_client_list = self
            .x11_client
            .x11_connection
            .send_request(&xcb::x::InternAtom {
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

        /*let root_window = x11_conn
            .get_setup()
            .roots()
            .nth(self.x11_client.x11_screen as usize)
            .unwrap();

        let tree_reply = x11_conn.wait_for_reply(x11_conn.send_request(&xcb::x::QueryTree {
            window: root_window.root(),
        }));

        for c in tree_reply.unwrap().children() {
            let window_information = self.x11_client.get_window_information(c);
            self.proc_win_info.insert(&window_information.unwrap());
        }*/

        for w in self.x11_client.get_wm_clients() {
            let window_info = self.x11_client.get_window_information(&w).unwrap();
            self.proc_win_info.insert(&window_info);
        }


        for proc_windows_info_iter in self.proc_win_info.procinfo.iter() {
            println!(
                "Process '{}' with pid '{}' has windows with xids '{}'",
                proc_windows_info_iter.0.cmdline,
                proc_windows_info_iter.0.process_id,
                proc_windows_info_iter
                    .1
                    .into_iter()
                    .map(|w_i| w_i.window_xid.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            );
        }

        while *self.is_running.lock().unwrap() {
            let event = match x11_conn.wait_for_event() {
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
                    self.proc_win_info
                        .insert(&self.x11_client.get_window_information(&window).unwrap());
                }
                xcb::Event::X(xcb::x::Event::DestroyNotify(ev)) => {
                    let window = ev.window();
                    self.proc_win_info
                        .remove(&self.x11_client.get_window_information(&window).unwrap());
                }
                xcb::Event::X(xcb::x::Event::PropertyNotify(_ev)) => {}
                _ => {}
            }
        }
        return Ok(0);
    }
}
