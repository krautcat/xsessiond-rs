use std::string::String;

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

