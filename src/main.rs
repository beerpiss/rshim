#![no_main]

mod helpers;
mod shims;

use helpers::{get_executable_path, wide_string, error_code_to_message};
use shims::Shim;

use std::{
    ffi::{CStr, CString},
    io::{stderr, Write},
    mem::size_of,
    ptr::null_mut,
};
use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, FALSE, LPVOID, TRUE},
        winerror::ERROR_ELEVATION_REQUIRED,
    },
    um::{
        combaseapi::CoInitializeEx,
        consoleapi,
        jobapi2::{AssignProcessToJobObject, SetInformationJobObject},
        objbase::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE},
        processthreadsapi::{
            CreateProcessW, GetExitCodeProcess, ResumeThread, PROCESS_INFORMATION, STARTUPINFOW,
        },
        shellapi::{ShellExecuteExA, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOA},
        synchapi::WaitForSingleObject,
        winbase::{CreateJobObjectA, CREATE_SUSPENDED, INFINITE},
        wincon,
        winnt::{
            JobObjectExtendedLimitInformation, JOBOBJECT_BASIC_LIMIT_INFORMATION,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
        },
        winuser::SW_NORMAL,
        errhandlingapi::GetLastError,
    },
};

const EXIT_FAILED_LOAD_SHIM: u32 = 1;
const EXIT_FAILED_SPAWN_PROG: u32 = 2;
const EXIT_FAILED_WAIT_PROG: u32 = 3;

unsafe extern "system" fn routine_handler(evt: DWORD) -> BOOL {
    match evt {
        wincon::CTRL_C_EVENT => TRUE,
        wincon::CTRL_BREAK_EVENT => TRUE,
        wincon::CTRL_CLOSE_EVENT => TRUE,
        wincon::CTRL_LOGOFF_EVENT => TRUE,
        wincon::CTRL_SHUTDOWN_EVENT => TRUE,
        other => {
            let mut stderr = stderr();
            let mut err = String::from("unknown event number: ");
            err.push_str(&other.to_string());
            err.push_str(", unhandled!");
            err.push('\n');
            stderr.write_all(err.as_bytes()).unwrap();
            FALSE
        }
    }
}

fn execute_elevated(program: &str, args: &[String]) -> u32 {
    let runas = CString::new("runas").unwrap();
    let program = CString::new(program).unwrap();
    let mut params = String::new();
    for arg in args.iter() {
        params.push(' ');
        if arg.is_empty() {
            params.push_str("\"\"");
        } else if arg.find(&[' ', '\t', '"'][..]).is_none() {
            params.push_str(arg);
        } else {
            params.push('"');
            for c in arg.chars() {
                match c {
                    '\\' => params.push_str("\\\\"),
                    '"' => params.push_str("\\\""),
                    c => params.push(c),
                }
            }
            params.push('"');
        }
    }

    let params = CString::new(&params[..]).unwrap();
    let mut info = SHELLEXECUTEINFOA {
        cbSize: size_of::<SHELLEXECUTEINFOA>() as DWORD,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS,
        lpVerb: runas.as_ptr(),
        lpFile: program.as_ptr(),
        lpParameters: params.as_ptr(),
        nShow: SW_NORMAL,
        ..Default::default()
    };
    let res = unsafe {
        CoInitializeEx(
            null_mut(),
            COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE,
        );
        ShellExecuteExA(&mut info as *mut _)
    };
    if res == FALSE || info.hProcess.is_null() {
        return EXIT_FAILED_SPAWN_PROG;
    }
    let mut code: DWORD = 0;
    unsafe {
        WaitForSingleObject(info.hProcess, INFINITE);
        if GetExitCodeProcess(info.hProcess, &mut code as *mut _) == FALSE {
            return EXIT_FAILED_WAIT_PROG;
        }
    }
    code
}

fn execute(program: &str, args: &[String]) -> u32 {
    let mut params = String::new();
    for arg in args.iter() {
        params.push(' ');
        if arg.is_empty() {
            params.push_str("\"\"");
        } else if arg.find(&[' ', '\t', '"'][..]).is_none() {
            params.push_str(arg);
        } else {
            params.push('"');
            for c in arg.chars() {
                match c {
                    '\\' => params.push_str("\\\\"),
                    '"' => params.push_str("\\\""),
                    c => params.push(c),
                }
            }
            params.push('"');
        }
    }
    let mut command = String::from(program);
    command.push(' ');
    command.push_str(&params);

    let mut startup_info = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process_info = PROCESS_INFORMATION::default();
    let handle = unsafe { CreateJobObjectA(null_mut(), null_mut()) };
    let mut jeli = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            &mut jeli as *mut _ as LPVOID,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
    }

    let res = unsafe {
        CreateProcessW(
            null_mut(),
            wide_string!(&command).as_mut_ptr(),
            null_mut(),
            null_mut(),
            TRUE,
            CREATE_SUSPENDED,
            null_mut(),
            null_mut(),
            &mut startup_info,
            &mut process_info,
        )
    };
    if res == FALSE || process_info.hProcess.is_null() {
        return EXIT_FAILED_SPAWN_PROG;
    }
    let mut code: DWORD = 0;
    unsafe {
        AssignProcessToJobObject(handle, process_info.hProcess);
        ResumeThread(process_info.hThread);
        WaitForSingleObject(process_info.hProcess, INFINITE);
        if GetExitCodeProcess(process_info.hProcess, &mut code as *mut _) == FALSE {
            return EXIT_FAILED_WAIT_PROG;
        }
    }
    code as u32
}

#[no_mangle]
pub extern "C" fn main(argc: isize, argv: *const *const i8) -> isize {
    let mut stderr = stderr();
    let res: BOOL = unsafe { consoleapi::SetConsoleCtrlHandler(Some(routine_handler), TRUE) };
    if res == FALSE {
        stderr
            .write_all(b"shim: register Ctrl handler failed.\n")
            .unwrap();
    }

    let mut argv: Vec<String> = (0..argc)
        .map(|i| {
            let str_slice = unsafe { CStr::from_ptr(*argv.offset(i)).to_str().unwrap() };
            str_slice.to_owned()
        })
        .collect();
    argv.remove(0);
    let shim = match Shim::init(get_executable_path().unwrap()) {
        Ok(v) => v,
        Err(e) => {
            let mut err = String::from("Error while loading shim: ");
            err.push_str(&e.to_string());
            err.push('\n');
            stderr.write_all(err.as_bytes()).unwrap();
            return EXIT_FAILED_LOAD_SHIM as isize;
        }
    };
    let args = if let Some(mut shim_args) = shim.args {
        shim_args.extend_from_slice(argv.as_slice());
        shim_args
    } else {
        argv
    };
    let status = execute(&shim.target_path, &args);
    match status {
        ERROR_ELEVATION_REQUIRED => execute_elevated(&shim.target_path, &args) as isize,
        EXIT_FAILED_SPAWN_PROG => {
            let mut err = String::from("Error while spawning target program `");
            err.push_str(&shim.target_path);
            err.push_str("`: ");
            // err.push_str(&e.to_string());
            err.push_str(unsafe { &error_code_to_message(GetLastError()).unwrap_or(String::new()) });
            err.push('\n');
            stderr.write_all(err.as_bytes()).unwrap();
            EXIT_FAILED_SPAWN_PROG as isize
        }
        EXIT_FAILED_WAIT_PROG => {
            let mut err = String::from("Error while waiting target program `");
            err.push_str(&shim.target_path);
            err.push_str("`: ");
            // err.push_str(&e.to_string());
            err.push_str(unsafe { &error_code_to_message(GetLastError()).unwrap_or(String::new()) });
            err.push('\n');
            stderr.write_all(err.as_bytes()).unwrap();
            EXIT_FAILED_WAIT_PROG as isize
        }
        _ => status as isize
    }
}
