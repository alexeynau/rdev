use crate::{
    rdev::{Event, ListenError},
    windows::common::{convert, get_scan_code, set_key_hook, set_mouse_hook, HookError},
};
use std::{os::raw::c_int, ptr::null_mut, time::SystemTime};
use winapi::{
    shared::{
        basetsd::ULONG_PTR,
        minwindef::{LPARAM, LRESULT, WPARAM},
    },
    um::winuser::{CallNextHookEx, GetMessageA, UnhookWindowsHookEx, HC_ACTION, PKBDLLHOOKSTRUCT, PMOUSEHOOKSTRUCT},
};

use super::{KEYBOARD_HOOK, MOUSE_HOOK};

static mut GLOBAL_CALLBACK: Option<Box<dyn FnMut(Event)>> = None;

impl From<HookError> for ListenError {
    fn from(error: HookError) -> Self {
        match error {
            HookError::Mouse(code) => ListenError::MouseHookError(code),
            HookError::Key(code) => ListenError::KeyHookError(code),
        }
    }
}

unsafe fn raw_callback(
    code: c_int,
    param: WPARAM,
    lpdata: LPARAM,
    f_get_extra_data: impl FnOnce(isize) -> ULONG_PTR,
) -> LRESULT {
    if code == HC_ACTION {
        let (opt, code) = convert(param, lpdata);
        if let Some(event_type) = opt {
            let event = Event {
                event_type,
                time: SystemTime::now(),
                unicode: None,
                platform_code: code as _,
                position_code: get_scan_code(lpdata),
                extra_data: f_get_extra_data(lpdata),
            };
            if let Some(callback) = &mut GLOBAL_CALLBACK {
                callback(event);
            }
        }
    }
    #[cfg(feature="debug_print")]
    println!("rdev: raw callback");
    CallNextHookEx(null_mut(), code, param, lpdata)
}

unsafe extern "system" fn raw_callback_mouse(code: i32, param: usize, lpdata: isize) -> isize {
    raw_callback(code, param, lpdata, |data: isize| unsafe {
        (*(data as PMOUSEHOOKSTRUCT)).dwExtraInfo
    })
}

unsafe extern "system" fn raw_callback_keyboard(code: i32, param: usize, lpdata: isize) -> isize {
    raw_callback(code, param, lpdata, |data: isize| unsafe {
        (*(data as PKBDLLHOOKSTRUCT)).dwExtraInfo
    })
}

pub fn listen<T>(callback: T) -> Result<(), ListenError>
where
    T: FnMut(Event) + 'static,
{
    #[cfg(feature="debug_print")]
    println!("rdev: listen");
    unsafe {
        GLOBAL_CALLBACK = Some(Box::new(callback));
        set_key_hook(raw_callback_keyboard)?;
        if !crate::keyboard_only() {
            set_mouse_hook(raw_callback_mouse)?;
        }

        GetMessageA(null_mut(), null_mut(), 0, 0);
    }
    Ok(())
}

pub fn unhook() -> bool {  
    let mut status = 0;
    unsafe {
        status = UnhookWindowsHookEx(KEYBOARD_HOOK);

        if !crate::keyboard_only() {
            status &=  UnhookWindowsHookEx(MOUSE_HOOK);
        }

    }
    return status != 0;
}
