#![allow(improper_ctypes_definitions)]
use crate::macos::common::*;
use crate::rdev::{Event, ListenError};
use cocoa::base::nil;
use cocoa::foundation::NSAutoreleasePool;
use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoopRunInMode};
use core_graphics::event::{CGEventTapLocation, CGEventType};
use lazy_static::lazy_static;
use std::os::raw::c_void;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

static mut GLOBAL_CALLBACK: Option<Box<dyn FnMut(Event)>> = None;
static STOP_LISTENING: AtomicBool = AtomicBool::new(false);

// lazy_static!{
//     static ref GLOABAL_RUN_LOOP: Arc<Mutex<Option<CFRunLoopRef>>> = Arc::new(Mutex::new(None));
// }

unsafe extern "C" fn raw_callback(
    _proxy: CGEventTapProxy,
    _type: CGEventType,
    cg_event: CGEventRef,
    _user_info: *mut c_void,
) -> CGEventRef {
    // println!("Event ref {:?}", cg_event_ptr);
    // let cg_event: CGEvent = transmute_copy::<*mut c_void, CGEvent>(&cg_event_ptr);
    if let Ok(mut state) = KEYBOARD_STATE.lock() {
        if let Some(keyboard) = state.as_mut() {
            if let Some(event) = convert(_type, &cg_event, keyboard) {
                if let Some(callback) = &mut GLOBAL_CALLBACK {
                    callback(event);
                }
            }
        }
    }
    // println!("Event ref END {:?}", cg_event_ptr);
    // cg_event_ptr
    cg_event
}

pub fn listen<T>(callback: T) -> Result<(), ListenError>
where
    T: FnMut(Event) + 'static,
{
    let mut types = kCGEventMaskForAllEvents;
    if crate::keyboard_only() {
        types = (1 << CGEventType::KeyDown as u64)
            + (1 << CGEventType::KeyUp as u64)
            + (1 << CGEventType::FlagsChanged as u64);
    }
    unsafe {
        GLOBAL_CALLBACK = Some(Box::new(callback));
        let _pool = NSAutoreleasePool::new(nil);
        let tap = CGEventTapCreate(
            CGEventTapLocation::HID, // HID, Session, AnnotatedSession,
            kCGHeadInsertEventTap,
            CGEventTapOption::ListenOnly,
            types,
            raw_callback,
            nil,
        );
        if tap.is_null() {
            return Err(ListenError::EventTapError);
        }
        let _loop = CFMachPortCreateRunLoopSource(nil, tap, 0);
        if _loop.is_null() {
            return Err(ListenError::LoopSourceError);
        }

        let current_loop = CFRunLoopGetMain();

        // *GLOABAL_RUN_LOOP.lock().unwrap() = Some(current_loop);
        CFRunLoopAddSource(current_loop, _loop, kCFRunLoopCommonModes);

        CGEventTapEnable(tap, true);
        
        while !STOP_LISTENING.load(std::sync::atomic::Ordering::SeqCst) {
             CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, 1);
        }

        CFRunLoopStop(current_loop);

        STOP_LISTENING.store(false, std::sync::atomic::Ordering::SeqCst);

        //CFRunLoopRun();
    }
    Ok(())
}

pub fn unhook() -> bool {
    STOP_LISTENING.store(true, std::sync::atomic::Ordering::SeqCst);

    true
}
