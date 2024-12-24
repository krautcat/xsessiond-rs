use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::string::String;

use sysinfo::{ProcessesToUpdate, System};

use crate::x11_client::X11WindowInformation;

#[derive(PartialEq, Eq, Hash)]
pub struct WindowInfo {
    pub window_name: String,
    pub window_xid: u32,
    pub desktop_name: String,
    pub desktop_number: u32,
}

impl WindowInfo {
    pub fn new(name: &String, xid: u32, dname: &String, dnum: u32) -> Self {
        return WindowInfo {
            window_name: name.clone(),
            window_xid: xid,
            desktop_name: dname.clone(),
            desktop_number: dnum,
        };
    }
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct ProcessInfo {
    pub cmdline: String,
    pub process_id: usize,
}

impl ProcessInfo {
    pub fn new(cmdline: String, process_id: usize) -> Self {
        return ProcessInfo {
            cmdline,
            process_id,
        };
    }
}

pub struct ProcessesWindowsInfo {
    pub procinfo: HashMap<ProcessInfo, HashSet<WindowInfo>>,
    sysinfo: System,
}

impl ProcessesWindowsInfo {
    pub fn new() -> Self {
        return ProcessesWindowsInfo {
            procinfo: HashMap::new(),
            sysinfo: System::new_all(),
        };
    }

    pub fn insert(self: &mut Self, x11_window_info: &X11WindowInformation) -> () {
        self.sysinfo.refresh_processes(ProcessesToUpdate::All);
        let process = self.sysinfo.process(x11_window_info.process_id);
        match process {
            Some(p) => {
                let cmdline = p.cmd().join(OsStr::new(" ")).into_string().unwrap();

                let window_info = WindowInfo::new(
                    &x11_window_info.x11_window_name,
                    x11_window_info.x11_resource_id,
                    &x11_window_info.x11_desktop_name,
                    x11_window_info.x11_desktop_number,
                );
                let proc_info = ProcessInfo::new(
                    cmdline,
                    usize::try_from(x11_window_info.process_id.as_u32()).unwrap(),
                );
                match self.procinfo.get_mut(&proc_info) {
                    Some(w_infos) => {
                        w_infos.insert(window_info);
                    }
                    None => {
                        let mut hash_set = HashSet::new();
                        hash_set.insert(window_info);
                        self.procinfo.insert(proc_info, hash_set);
                    }
                }
            }
            None => {}
        }
    }

    pub fn remove(self: &mut Self, x11_window_info: &X11WindowInformation) -> () {
        self.sysinfo.refresh_processes(ProcessesToUpdate::All);
        let process = self.sysinfo.process(x11_window_info.process_id);

        match process {
            Some(p) => {
                let cmdline = p.cmd().join(OsStr::new(" ")).into_string().unwrap();
                let pid = usize::try_from(x11_window_info.process_id.as_u32()).unwrap();
                let proc_info = ProcessInfo::new(cmdline, pid);

                match self.procinfo.get_mut(&proc_info) {
                    Some(windows_of_process) => {
                        windows_of_process.retain(|window_info| {
                            window_info.window_xid != x11_window_info.x11_resource_id
                        });

                        if windows_of_process.is_empty() {
                            self.procinfo.remove(&proc_info);
                        }
                    }
                    None => {}
                }
            }
            None => {}
        }
    }
}
