fn main() {
    let mode = 0o660;
    let path = "test_sock";
    std::fs::write(path, "test").unwrap();
    let gid = unsafe { libc::getgid() }; 
    let path_cstr = std::ffi::CString::new(path).unwrap();
    unsafe {
        let res = libc::chown(path_cstr.as_ptr(), u32::MAX, gid);
        println!("chown res: {}", res);
        if res < 0 {
            println!("errno: {}", std::io::Error::last_os_error());
        }
    }
}
