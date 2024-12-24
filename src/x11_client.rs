use std::convert::From;

use sysinfo::Pid;
use xcb::x::Window as X11Window;
use xcb::Connection as X11Connection;
use xcb::Xid;
use xcb_wm::ewmh::Connection as EWMHConnection;
use xcb_wm::icccm::Connection as ICCCMConnection;

enum ClientErrorType {
    Connection,
}

pub struct ClientError {
    err_type: ClientErrorType,
}

impl ClientError {
    pub fn new(err: ClientErrorType) -> Self {
        return ClientError { err_type: err };
    }
}

#[derive(Debug)]
pub enum GatherInfoErrorType {
    WindowDesktopNumber,
}

#[derive(Debug)]
pub struct GatherInfoError {
    err: GatherInfoErrorType,
}

impl GatherInfoError {
    pub fn new(err: GatherInfoErrorType) -> Self {
        return GatherInfoError { err };
    }
}
pub struct X11WindowInformation<'a> {
    pub x11_window: &'a X11Window,
    pub x11_resource_id: u32,
    pub x11_window_name: String,
    pub x11_desktop_number: u32,
    pub x11_desktop_name: String,
    pub process_id: Pid,
}

impl<'a> X11WindowInformation<'a> {
    pub fn new(
        window: &'a X11Window,
        resource_id: u32,
        name: String,
        desktop_number: u32,
        desktop_name: String,
        pid: Pid,
    ) -> Self {
        return X11WindowInformation {
            x11_window: window,
            x11_resource_id: resource_id,
            x11_window_name: name,
            x11_desktop_number: desktop_number,
            x11_desktop_name: desktop_name,
            process_id: pid,
        };
    }
}

pub struct X11Client<'a> {
    pub x11_connection: X11Connection,
    ewmh_connection: Option<EWMHConnection<'a>>,
    icccm_connection: Option<ICCCMConnection<'a>>,

    pub x11_screen: i32,
}

impl<'a> X11Client<'a> {
    pub fn new() -> Self {
        let (x11_con, x11_screen) = X11Connection::connect(None).unwrap();
        return X11Client {
            x11_connection: x11_con,
            ewmh_connection: None,
            icccm_connection: None,

            x11_screen,
        };
    }

    pub fn connect(self: &'a mut Self) -> &Self {
        self.ewmh_connection = Some(EWMHConnection::connect(&self.x11_connection));
        self.icccm_connection = Some(ICCCMConnection::connect(&self.x11_connection));

        return self;
    }

    pub fn get_wm_clients(self: &Self) -> Vec<X11Window> {
        let request_cookie = self.ewmh_connection.as_ref().unwrap().send_request(&xcb_wm::ewmh::proto::GetClientList);
        return self.ewmh_connection.as_ref().unwrap().wait_for_reply(request_cookie).unwrap().clients;
    }

    pub fn get_window_information(
        self: &'a Self,
        window: &'a X11Window,
    ) -> Result<X11WindowInformation, GatherInfoError> {
        let desktop_number = match self.get_desktop_number_of_window(window) {
            Ok(r) => r,
            Err(e) => return Err(e),
        };

        return Ok(X11WindowInformation::new(
            window,
            window.resource_id(),
            self.get_window_name(window),
            desktop_number,
            self.get_desktop_name_of_window(desktop_number),
            self.get_process_id_of_local_client(window),
        ));
    }

    fn get_window_name(self: &Self, window: &X11Window) -> String {
        let ewmh_con = self.ewmh_connection.as_ref().unwrap();
        return ewmh_con
            .wait_for_reply(ewmh_con.send_request(&xcb_wm::ewmh::proto::GetWmName(*window)))
            .unwrap()
            .name
            .clone();
    }

    fn get_desktop_number_of_window(
        self: &Self,
        window: &X11Window,
    ) -> Result<u32, GatherInfoError> {
        let ewmh_con = self.ewmh_connection.as_ref().unwrap();

        match ewmh_con
            .wait_for_reply(ewmh_con.send_request(&xcb_wm::ewmh::proto::GetWmDesktop(*window)))
        {
            Ok(reply) => return Ok(reply.desktop),
            Err(_) => {
                return Err(GatherInfoError::new(
                    GatherInfoErrorType::WindowDesktopNumber,
                ))
            }
        }
    }

    fn get_desktop_name_of_window(self: &Self, number: u32) -> String {
        let ewmh_con = self.ewmh_connection.as_ref().unwrap();
        let desktop_names = ewmh_con
            .wait_for_reply(ewmh_con.send_request(&xcb_wm::ewmh::proto::GetDesktopNames))
            .unwrap();
        return desktop_names.names[usize::try_from(number).unwrap()].clone();
    }

    fn get_process_id_of_local_client(self: &Self, window: &X11Window) -> Pid {
        let bits: u32 = 0x02;
        let window_id_spec = xcb::res::ClientIdSpec {
            client: window.resource_id(),
            mask: xcb::res::ClientIdMask::from_bits(bits).unwrap(),
        };

        let pid_reply = self
            .x11_connection
            .wait_for_reply(self.x11_connection.send_request(&xcb::res::QueryClientIds {
                specs: &[window_id_spec],
            }));

        return Pid::from(
            usize::try_from(pid_reply.unwrap().ids().next().unwrap().value()[0]).unwrap(),
        );
    }
}

struct X11ClientExtensions<'a> {
    ewmh: EWMHConnection<'a>,
    icccm: ICCCMConnection<'a>,
}

impl<'a> X11ClientExtensions<'a> {
    pub fn new(x11_con: &'a X11Connection) -> X11ClientExtensions {
        return X11ClientExtensions {
            ewmh: EWMHConnection::connect(x11_con),
            icccm: ICCCMConnection::connect(x11_con),
        };
    }
}
