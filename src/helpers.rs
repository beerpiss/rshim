use std::{ffi::OsString, os::windows::ffi::OsStringExt, ptr};
use winapi::{
    shared::{
        minwindef::{DWORD, MAX_PATH},
        winerror::ERROR_INSUFFICIENT_BUFFER,
        ntdef::{MAKELANGID, LANG_NEUTRAL, SUBLANG_DEFAULT},
    },
    um::{
        errhandlingapi::GetLastError, 
        libloaderapi::GetModuleFileNameW,
        winbase::{FormatMessageW, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS}
    },
};

macro_rules! wide_string {
    ($string:expr) => {{
        use std::os::windows::ffi::OsStrExt;
        let input = std::ffi::OsStr::new($string);
        let vec: Vec<u16> = input.encode_wide().chain(Some(0)).collect();
        vec
    }};
}
pub(crate) use wide_string;

pub fn get_executable_path() -> Option<String> {
    fn get_executable_path(len: usize) -> Option<String> {
        let mut buf = Vec::with_capacity(len);
        unsafe {
            let ret = GetModuleFileNameW(ptr::null_mut(), buf.as_mut_ptr(), len as DWORD) as usize;
            if ret == 0 {
                None
            } else if ret < len {
                // Success, we need to trim trailing null bytes from the vec.
                buf.set_len(ret);
                let s = OsString::from_wide(&buf);
                Some(s.into_string().unwrap())
            } else {
                // The buffer might not be big enough so we need to check errno.
                let errno = GetLastError();
                if errno == ERROR_INSUFFICIENT_BUFFER {
                    get_executable_path(len * 2)
                } else {
                    None
                }
            }
        }
    }

    get_executable_path(MAX_PATH)
}

pub fn error_code_to_message(code: u32) -> Option<String> {
    let mut message_buf: [u16; 512] = [0; 512];

    // Get the error string by the code
    let buf_len = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            ptr::null_mut(),
            code,
            MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT) as u32,
            message_buf.as_mut_ptr(),
            512,
            ptr::null_mut(),
        )
    };

    // there is no message for the error
    if buf_len == 0 {
        return None;
    }

    let mut error_string = String::from_utf16_lossy(&message_buf);

    // Remove \n from end of string
    error_string.pop();
    // Remove \r from end of string
    error_string.pop();

    Some(error_string)
}
