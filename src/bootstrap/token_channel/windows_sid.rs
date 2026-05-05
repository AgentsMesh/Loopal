//! Windows SID lookup helpers — pulled out of `windows.rs` to keep
//! that file under the 200-line cap. Intentionally raw FFI to avoid
//! pulling in `windows-sys` as a dependency.

type Handle = *mut std::ffi::c_void;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const TOKEN_QUERY: u32 = 0x0008;
const TOKEN_USER_CLASS: i32 = 1;

unsafe extern "system" {
    fn OpenProcess(desired: u32, inherit: i32, pid: u32) -> Handle;
    fn GetCurrentProcess() -> Handle;
    fn CloseHandle(h: Handle) -> i32;
    fn OpenProcessToken(process: Handle, desired: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(
        token: Handle,
        class: i32,
        buf: *mut std::ffi::c_void,
        buf_len: u32,
        ret_len: *mut u32,
    ) -> i32;
    fn GetLengthSid(sid: *const std::ffi::c_void) -> u32;
}

#[repr(C)]
struct SidAndAttributes {
    sid: *const std::ffi::c_void,
    attributes: u32,
}
#[repr(C)]
struct TokenUser {
    user: SidAndAttributes,
}

/// Returns the user SID of `pid`, or current process when `pid` is None.
pub fn query_user_sid(pid: Option<u32>) -> Option<Vec<u8>> {
    let process = match pid {
        Some(p) => unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, p) },
        None => unsafe { GetCurrentProcess() },
    };
    if process.is_null() {
        return None;
    }
    let owns_handle = pid.is_some();

    let mut token: Handle = std::ptr::null_mut();
    let opened = unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) };
    if owns_handle {
        unsafe { CloseHandle(process) };
    }
    if opened == 0 || token.is_null() {
        return None;
    }

    let mut needed: u32 = 0;
    unsafe {
        GetTokenInformation(
            token,
            TOKEN_USER_CLASS,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    }
    if needed == 0 {
        unsafe { CloseHandle(token) };
        return None;
    }

    let mut buf = vec![0u8; needed as usize];
    let ok = unsafe {
        GetTokenInformation(
            token,
            TOKEN_USER_CLASS,
            buf.as_mut_ptr() as *mut _,
            needed,
            &mut needed,
        )
    };
    unsafe { CloseHandle(token) };
    if ok == 0 {
        return None;
    }

    let user = unsafe { &*(buf.as_ptr() as *const TokenUser) };
    let sid_len = unsafe { GetLengthSid(user.user.sid) };
    if sid_len == 0 {
        return None;
    }
    let sid_bytes =
        unsafe { std::slice::from_raw_parts(user.user.sid as *const u8, sid_len as usize) };
    Some(sid_bytes.to_vec())
}
