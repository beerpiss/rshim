#![feature(control_flow_enum)]
#![feature(try_blocks)]
#![no_main]

mod shims;

use shims::Shim;

use std::{
    env,
    ffi::{CStr, CString},
    io::{stderr, Write},
    mem::size_of,
    path::Path,
    process::{exit, Command},
    ptr::null_mut,
};
use winapi::{
    shared::minwindef::{BOOL, DWORD, FALSE, TRUE},
    um::{
        combaseapi::CoInitializeEx,
        consoleapi,
        objbase::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE},
        processthreadsapi::GetExitCodeProcess,
        shellapi::{ShellExecuteExA, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOA},
        synchapi::WaitForSingleObject,
        winbase::INFINITE,
        wincon,
        winuser::SW_NORMAL,
    },
};

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

const EXIT_FAILED_LOAD_SHIM: i32 = 1;
const EXIT_FAILED_SPAWN_PROG: i32 = 2;
const EXIT_FAILED_WAIT_PROG: i32 = 3;
const EXIT_PROG_TERMINATED: i32 = 4;

const ERROR_ELEVATION_REQUIRED: i32 = 740;

#[no_mangle]
pub fn main(argc: isize, argv: *const *const i8) {
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
    let shim = match Shim::init(env::current_exe().unwrap().to_string_lossy().into()) {
        Ok(v) => v,
        Err(e) => {
            let mut err = String::from("Error while loading shim: ");
            err.push_str(&e.to_string());
            err.push('\n');
            stderr.write_all(err.as_bytes()).unwrap();
            exit(EXIT_FAILED_LOAD_SHIM);
        }
    };
    let args = if let Some(mut shim_args) = shim.args {
        shim_args.extend_from_slice(argv.as_slice());
        shim_args
    } else {
        argv
    };
    let mut cmd = match Command::new(&shim.target_path).args(&args).spawn() {
        Ok(v) => v,
        Err(e) if e.raw_os_error() == Some(ERROR_ELEVATION_REQUIRED) => {
            exit(execute(&shim.target_path, "runas", &args))
        }
        Err(e) => {
            let mut err = String::from("Error while spawning target program `");
            err.push_str(&shim.target_path.to_string_lossy());
            err.push_str("`: ");
            err.push_str(&e.to_string());
            err.push('\n');
            stderr.write_all(err.as_bytes()).unwrap();
            exit(EXIT_FAILED_SPAWN_PROG);
        }
    };
    let status = match cmd.wait() {
        Ok(v) => v,
        Err(e) => {
            let mut err = String::from("Error while waiting target program `");
            err.push_str(&shim.target_path.to_string_lossy());
            err.push_str("`: ");
            err.push_str(&e.to_string());
            err.push('\n');
            stderr.write_all(err.as_bytes()).unwrap();
            exit(EXIT_FAILED_WAIT_PROG);
        }
    };
    exit(status.code().unwrap_or(EXIT_PROG_TERMINATED))
}

fn execute(program: &Path, verb: &str, args: &[String]) -> i32 {
    let runas = CString::new(verb).unwrap();
    let program = CString::new(program.to_str().unwrap()).unwrap();
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
    code as i32
}
